//! One-shot admin tool to assign a USB serial string to a u-blox ZED-F9P.
//!
//! A factory-reset F9P has a blank USB serial, which leaves udev with
//! nothing to key its `ID_SERIAL_SHORT`-based rules on. This tool reads the
//! current string via CFG-VALGET, and — if it's blank (or `--force` is set)
//! — writes the chosen value to all three layers (RAM + BBR + FLASH) so the
//! assignment survives a power cycle.
//!
//! Scope: **one device at a time**, run manually during bring-up. NOT part
//! of normal driver startup. To avoid silently creating a udev collision,
//! the tool scans other tty serial numbers on the host and refuses to
//! proceed if the chosen serial is already taken by another device
//! (override with `--force`).

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use tokio::time::timeout;

use oxide_gnss::device::{
    assemble_usb_serial_string, build_cfg_valset_all_layers, chunk_usb_serial_to_cfg_vals,
    query_cfg_valget, AckResult, SerialPortBuilder, UbxHandler, USB_SERIAL_KEY_IDS,
};

const MAX_SERIAL_LEN: usize = 32;
const ACK_TIMEOUT: Duration = Duration::from_millis(500);
const VALGET_TIMEOUT: Duration = Duration::from_millis(500);
const CFG_VALSET_CLASS: u8 = 0x06;
const CFG_VALSET_MSG_ID: u8 = 0x8A;
const LAYER_RAM: u8 = 0;

struct Args {
    port: PathBuf,
    serial: String,
    baud: u32,
    force: bool,
    dry_run: bool,
}

fn print_usage() {
    eprintln!(
        "Usage: oxide_gnss_assign_serial --port <path> --serial <string> \
[--baud <rate>] [--force] [--dry-run]

Assign a USB serial string to a u-blox ZED-F9P. The value is written to
RAM + BBR + FLASH and takes effect on the next USB re-enumeration
(unplug/replug).

Options:
  --port     Serial port path (e.g. /dev/ttyACM0)
  --serial   USB serial string to assign (ASCII, 1..=32 bytes)
  --baud     Baud rate (default: 460800)
  --force    Write even if the current serial is non-blank, or if another
             device on the host already uses the same serial.
  --dry-run  Probe and print what would be written, but don't write.
  -h, --help Print this help."
    );
}

fn parse_args() -> Result<Args, ExitCode> {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut port: Option<PathBuf> = None;
    let mut serial: Option<String> = None;
    let mut baud: u32 = 460_800;
    let mut force = false;
    let mut dry_run = false;

    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--port" => {
                i += 1;
                let Some(v) = raw.get(i) else {
                    eprintln!("error: --port requires a value");
                    return Err(ExitCode::from(2));
                };
                port = Some(PathBuf::from(v));
            }
            "--serial" => {
                i += 1;
                let Some(v) = raw.get(i) else {
                    eprintln!("error: --serial requires a value");
                    return Err(ExitCode::from(2));
                };
                serial = Some(v.clone());
            }
            "--baud" => {
                i += 1;
                let Some(v) = raw.get(i) else {
                    eprintln!("error: --baud requires a value");
                    return Err(ExitCode::from(2));
                };
                baud = v.parse().map_err(|_| {
                    eprintln!("error: --baud must be a non-negative integer");
                    ExitCode::from(2)
                })?;
            }
            "--force" => force = true,
            "--dry-run" => dry_run = true,
            "-h" | "--help" => {
                print_usage();
                return Err(ExitCode::SUCCESS);
            }
            other => {
                eprintln!("error: unknown argument '{other}'");
                return Err(ExitCode::from(2));
            }
        }
        i += 1;
    }

    let Some(port) = port else {
        eprintln!("error: --port is required");
        print_usage();
        return Err(ExitCode::from(2));
    };
    let Some(serial) = serial else {
        eprintln!("error: --serial is required");
        print_usage();
        return Err(ExitCode::from(2));
    };

    if serial.is_empty() || serial.len() > MAX_SERIAL_LEN || !serial.is_ascii() {
        eprintln!(
            "error: --serial must be 1..={MAX_SERIAL_LEN} ASCII bytes (got {} bytes)",
            serial.len()
        );
        return Err(ExitCode::from(2));
    }

    Ok(Args {
        port,
        serial,
        baud,
        force,
        dry_run,
    })
}

/// Scan `/sys/class/tty/ttyACM*` and `ttyUSB*` for existing USB serial
/// strings reported by the kernel. Returns `(tty_path, serial)` pairs where
/// serial is present and non-empty.
///
/// Silent on errors: a missing or unreadable `/sys/class/tty` just means we
/// skip the host-wide check.
fn scan_host_usb_serials() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(Path::new("/sys/class/tty")) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if !name_str.starts_with("ttyACM") && !name_str.starts_with("ttyUSB") {
            continue;
        }

        // Traverse to the USB device that owns this tty. For ttyACM/ttyUSB
        // this is the parent of the `device` symlink.
        let serial_path = entry.path().join("device").join("..").join("serial");
        if let Ok(s) = std::fs::read_to_string(&serial_path) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                out.push((format!("/dev/{name_str}"), trimmed.to_string()));
            }
        }
    }
    out
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(code) => return code,
    };

    // Host-wide collision check.
    let existing = scan_host_usb_serials();
    let duplicates: Vec<_> = existing
        .iter()
        .filter(|(tty, s)| {
            // An existing tty whose own path matches our --port is expected
            // to be the target device itself; ignore it in the duplicate check.
            s == &args.serial && Path::new(tty) != args.port
        })
        .collect();
    if !duplicates.is_empty() {
        eprintln!(
            "WARN: serial '{}' is already in use by another device on this host:",
            args.serial
        );
        for (tty, s) in &duplicates {
            eprintln!("      {tty} -> serial={s}");
        }
        if !args.force {
            eprintln!(
                "error: refusing to proceed — would create a udev collision. \
                 Re-run with --force if you really mean to duplicate the serial."
            );
            return ExitCode::from(3);
        }
        eprintln!("--force set; proceeding anyway.");
    }

    // Open the target port.
    let mut serial = match SerialPortBuilder::new(args.port.to_string_lossy().as_ref())
        .baud_rate(args.baud)
        .open()
        .await
    {
        Ok(p) => p,
        Err(e) => {
            eprintln!("error: opening {}: {e}", args.port.display());
            return ExitCode::from(1);
        }
    };
    let mut ubx = UbxHandler::new();

    // Probe current serial (from RAM layer).
    let resp = match query_cfg_valget(
        &mut serial,
        &mut ubx,
        &USB_SERIAL_KEY_IDS,
        LAYER_RAM,
        VALGET_TIMEOUT,
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: CFG-VALGET probe failed: {e}");
            return ExitCode::from(1);
        }
    };
    let current = assemble_usb_serial_string(&resp);
    match current.as_deref() {
        Some("") | None => {
            println!("current USB serial: <blank>");
        }
        Some(s) => {
            println!("current USB serial: '{s}'");
            if !args.force {
                eprintln!("error: refusing to overwrite non-blank serial without --force.");
                return ExitCode::from(3);
            }
            eprintln!("--force set; will overwrite.");
        }
    }
    println!("assigning USB serial: '{}'", args.serial);

    if args.dry_run {
        println!("--dry-run set; not writing.");
        return ExitCode::SUCCESS;
    }

    // Write RAM + BBR + FLASH.
    let Some(cfg_vals) = chunk_usb_serial_to_cfg_vals(&args.serial) else {
        // Should be unreachable — parse_args already validated length/ASCII.
        eprintln!("error: internal: --serial failed late validation.");
        return ExitCode::from(2);
    };

    let packet = build_cfg_valset_all_layers(&cfg_vals);
    ubx.clear_pending_ack();
    ubx.expect_ack(CFG_VALSET_CLASS, CFG_VALSET_MSG_ID);
    if let Err(e) = serial.write(&packet).await {
        eprintln!("error: writing CFG-VALSET: {e}");
        return ExitCode::from(1);
    }

    let mut buf = [0u8; 256];
    let ack = timeout(ACK_TIMEOUT, async {
        loop {
            let n = serial.read(&mut buf).await?;
            if n > 0 {
                let result = ubx.process(&buf[..n]);
                if let Some(ack) = result.ack {
                    return Ok::<AckResult, oxide_gnss::error::DeviceError>(ack);
                }
            }
        }
    })
    .await;

    match ack {
        Ok(Ok(AckResult::Ack)) => {
            println!("OK: USB serial assigned (RAM + BBR + FLASH).");
            println!("Unplug and replug the F9P for USB re-enumeration to apply the change.");
            ExitCode::SUCCESS
        }
        Ok(Ok(AckResult::Nak)) => {
            eprintln!("error: F9P NAK'd CFG-VALSET — serial NOT assigned.");
            ExitCode::from(1)
        }
        Ok(Err(e)) => {
            eprintln!("error: serial read failed while awaiting ACK: {e}");
            ExitCode::from(1)
        }
        Err(_) => {
            eprintln!("error: timed out waiting for CFG-VALSET ACK.");
            ExitCode::from(1)
        }
    }
}
