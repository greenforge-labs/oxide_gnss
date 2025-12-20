//! Standalone NTRIP client test program.
//!
//! Tests the NTRIP client against public domain casters without requiring
//! ROS2 or a connected GNSS device.
//!
//! Usage:
//!   cargo run --example ntrip_test -- [command] [options]
//!
//! Commands:
//!   sourcetable <host> [port]     - Fetch and display sourcetable
//!   nearest <host> <lat> <lon>    - Find nearest RTCM mountpoint
//!   connect <host> <mountpoint>   - Test connection to a mountpoint
//!   test-casters                  - Test against known public casters

use std::env;
use std::time::Duration;

use oxide_gnss::config::{NtripConfig, NtripConnectionConfig, NtripVersion};
use oxide_gnss::error::NtripError;
use oxide_gnss::ntrip::{NtripClient, Sourcetable};

/// Well-known public NTRIP casters for testing
/// Sources:
/// - http://www.rtcm-ntrip.org/home
/// - http://rtk2go.com/real-time-status/
/// - https://www.euref-ip.be/
/// - https://euref-ip.net/home
/// - https://products.igs-ip.net/home
/// - https://gnss.ga.gov.au/stream
const PUBLIC_CASTERS: &[(&str, u16, &str, &str)] = &[
    // RTK2go - Large free community caster
    ("rtk2go.com", 2101, "RTK2go Community", "Global"),
    
    // EUREF - European Reference Frame
    ("euref-ip.net", 2101, "EUREF-IP", "Europe"),
    ("www.euref-ip.net", 2101, "EUREF-IP (www)", "Europe"),
    ("euref-ip.be", 2101, "EUREF-IP Belgium", "Europe"),
    
    // IGS - International GNSS Service
    ("products.igs-ip.net", 2101, "IGS Products", "Global"),
    ("igs-ip.net", 2101, "IGS-IP", "Global"),
    
    // Geoscience Australia
    ("auscors.ga.gov.au", 2101, "AUSCORS (Geoscience AU)", "Australia"),
    
    // BKG - German Federal Agency
    ("igs.bkg.bund.de", 2101, "BKG Germany", "Germany"),
    ("ntrip.bkg.bund.de", 2101, "BKG NTRIP", "Germany"),
    
    // Other regional casters
    ("caster.centipede.fr", 2101, "Centipede RTK", "France"),
    ("ntrip.emlid.com", 2101, "Emlid", "Global"),
    
    // HTTPS casters (port 443 typically)
    ("euref-ip.net", 443, "EUREF-IP (HTTPS)", "Europe"),
    ("products.igs-ip.net", 443, "IGS Products (HTTPS)", "Global"),
];

/// Test locations around the world for distance testing
const TEST_LOCATIONS: &[(&str, f64, f64)] = &[
    ("Sydney, Australia", -33.8688, 151.2093),
    ("Brisbane, Australia", -27.4698, 153.0251),
    ("Alice Springs, Australia", -23.6980, 133.8807),
    ("London, UK", 51.5074, -0.1278),
    ("Paris, France", 48.8566, 2.3522),
    ("Berlin, Germany", 52.5200, 13.4050),
    ("New York, USA", 40.7128, -74.0060),
    ("San Francisco, USA", 37.7749, -122.4194),
    ("Tokyo, Japan", 35.6762, 139.6503),
];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("oxide_gnss=debug".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    match args[1].as_str() {
        "sourcetable" => {
            if args.len() < 3 {
                eprintln!("Usage: ntrip_test sourcetable <host> [port] [--https] [--v1|--v2]");
                return Ok(());
            }
            let host = &args[2];
            let port: u16 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(2101);
            let use_https = args.iter().any(|a| a == "--https");
            let version = parse_version(&args);
            cmd_sourcetable(host, port, use_https, version).await?;
        }
        "nearest" => {
            if args.len() < 5 {
                eprintln!("Usage: ntrip_test nearest <host> <lat> <lon> [port]");
                return Ok(());
            }
            let host = &args[2];
            let lat: f64 = args[3].parse()?;
            let lon: f64 = args[4].parse()?;
            let port: u16 = args.get(5).and_then(|p| p.parse().ok()).unwrap_or(2101);
            cmd_nearest(host, port, lat, lon).await?;
        }
        "connect" => {
            if args.len() < 4 {
                eprintln!("Usage: ntrip_test connect <host> <mountpoint> [port] [--user=X] [--pass=X] [--https] [--v1|--v2]");
                eprintln!();
                eprintln!("Note: Most public casters require registration for stream access.");
                eprintln!("      RTK2go accepts email as username, no password needed.");
                return Ok(());
            }
            let host = &args[2];
            let mountpoint = &args[3];
            let port: u16 = args.get(4).and_then(|p| p.parse().ok()).unwrap_or(2101);
            let use_https = args.iter().any(|a| a == "--https");
            let version = parse_version(&args);
            let user = parse_arg(&args, "--user=");
            let pass = parse_arg(&args, "--pass=");
            cmd_connect(host, port, mountpoint, user.as_deref(), pass.as_deref(), use_https, version).await?;
        }
        "test-casters" => {
            cmd_test_casters().await?;
        }
        "test-versions" => {
            if args.len() < 3 {
                eprintln!("Usage: ntrip_test test-versions <host> [port]");
                return Ok(());
            }
            let host = &args[2];
            let port: u16 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(2101);
            cmd_test_versions(host, port).await?;
        }
        "test-locations" => {
            if args.len() < 3 {
                eprintln!("Usage: ntrip_test test-locations <host> [port]");
                return Ok(());
            }
            let host = &args[2];
            let port: u16 = args.get(3).and_then(|p| p.parse().ok()).unwrap_or(2101);
            cmd_test_locations(host, port).await?;
        }
        "list-casters" => {
            cmd_list_casters();
        }
        "list-locations" => {
            cmd_list_locations();
        }
        _ => {
            print_usage();
        }
    }

    Ok(())
}

fn print_usage() {
    println!("NTRIP Client Test Tool");
    println!();
    println!("Usage: cargo run --example ntrip_test -- <command> [options]");
    println!();
    println!("Commands:");
    println!("  sourcetable <host> [port] [--https] [--v1|--v2]");
    println!("      Fetch and display sourcetable from a caster");
    println!();
    println!("  nearest <host> <lat> <lon> [port]");
    println!("      Find nearest RTCM mountpoint to given coordinates");
    println!();
    println!("  connect <host> <mountpoint> [port] [--user=X] [--pass=X] [--https] [--v1|--v2]");
    println!("      Test connection to a specific mountpoint (may require credentials)");
    println!();
    println!("  test-casters");
    println!("      Test sourcetable retrieval against all known public casters");
    println!();
    println!("  test-versions <host> [port]");
    println!("      Test NTRIP v1 vs v2 protocol against a caster");
    println!();
    println!("  test-locations <host> [port]");
    println!("      Find nearest mountpoints from various world locations");
    println!();
    println!("  list-casters");
    println!("      List all known public casters");
    println!();
    println!("  list-locations");
    println!("      List all test locations");
    println!();
    println!("Options:");
    println!("  --https      Use HTTPS/TLS connection");
    println!("  --v1         Force NTRIP v1 protocol");
    println!("  --v2         Force NTRIP v2 protocol");
    println!("  --user=EMAIL Username for authentication");
    println!("  --pass=PASS  Password for authentication");
    println!();
    println!("Examples:");
    println!("  cargo run --example ntrip_test -- sourcetable rtk2go.com");
    println!("  cargo run --example ntrip_test -- sourcetable euref-ip.net 443 --https");
    println!("  cargo run --example ntrip_test -- nearest auscors.ga.gov.au -27.5 153.0");
    println!("  cargo run --example ntrip_test -- test-casters");
    println!("  cargo run --example ntrip_test -- test-versions rtk2go.com");
    println!("  cargo run --example ntrip_test -- test-locations rtk2go.com");
    println!();
    println!("  # Connect to RTK2go stream (use email as username):");
    println!("  cargo run --example ntrip_test -- connect rtk2go.com MOUNTPOINT --user=you@email.com");
    println!();
    println!("Authentication Notes:");
    println!("  - Sourcetable access is typically open (no auth needed)");
    println!("  - Stream connections usually require registration");
    println!("  - RTK2go: Use email as username, password can be blank or email");
    println!("  - EUREF/IGS/AUSCORS: Require formal registration");
    println!("  - Centipede: Open community network (France)");
}

fn parse_version(args: &[String]) -> NtripVersion {
    if args.iter().any(|a| a == "--v1") {
        NtripVersion::V1
    } else if args.iter().any(|a| a == "--v2") {
        NtripVersion::V2
    } else {
        NtripVersion::Auto
    }
}

fn parse_arg(args: &[String], prefix: &str) -> Option<String> {
    args.iter()
        .find(|a| a.starts_with(prefix))
        .map(|a| a[prefix.len()..].to_string())
}

fn make_config(
    host: &str,
    port: u16,
    mountpoint: &str,
    use_https: bool,
    version: NtripVersion,
) -> NtripConfig {
    NtripConfig {
        host: host.to_string(),
        port,
        mountpoint: mountpoint.to_string(),
        username: None,
        password: None,
        use_https,
        tls_skip_verify: false,
        ntrip_version: version,
        send_gga: false,
        gga_interval_secs: 10,
        connection: NtripConnectionConfig {
            timeout_secs: 15,
            read_timeout_secs: 30,
            reconnect: false,
            initial_delay_secs: 1,
            max_delay_secs: 60,
            backoff_reset_secs: 3600,
        },
    }
}

/// Fetch and display sourcetable
async fn cmd_sourcetable(
    host: &str,
    port: u16,
    use_https: bool,
    version: NtripVersion,
) -> Result<(), NtripError> {
    let protocol = if use_https { "https" } else { "http" };
    println!(
        "Fetching sourcetable from {}://{}:{} (NTRIP {:?})...\n",
        protocol, host, port, version
    );

    let config = make_config(host, port, "", use_https, version);
    let table = NtripClient::get_sourcetable(&config).await?;

    print_sourcetable(&table);
    Ok(())
}

/// Find nearest RTCM mountpoint
async fn cmd_nearest(host: &str, port: u16, lat: f64, lon: f64) -> Result<(), NtripError> {
    println!(
        "Finding nearest RTCM mountpoint to ({}, {}) from {}:{}...\n",
        lat, lon, host, port
    );

    let config = make_config(host, port, "", false, NtripVersion::Auto);
    let table = NtripClient::get_sourcetable(&config).await?;

    println!("Top 10 nearest RTCM streams:");
    println!("{:-<80}", "");

    let streams = table.streams_by_distance(lat, lon);
    for (i, (stream, dist)) in streams.iter().take(10).enumerate() {
        if stream.is_rtcm() {
            println!(
                "{:2}. {:20} {:>8.1} km  {} ({})",
                i + 1,
                stream.mountpoint,
                dist,
                stream.format,
                stream.nav_system
            );
        }
    }

    if let Some((nearest, dist)) = table.nearest_rtcm_stream(lat, lon) {
        println!();
        println!("Recommended: {} at {:.1} km", nearest.mountpoint, dist);
    }

    Ok(())
}

/// Test connection to a mountpoint
async fn cmd_connect(
    host: &str,
    port: u16,
    mountpoint: &str,
    user: Option<&str>,
    pass: Option<&str>,
    use_https: bool,
    version: NtripVersion,
) -> Result<(), NtripError> {
    let protocol = if use_https { "https" } else { "http" };
    println!(
        "Connecting to {}://{}:{}/{} (NTRIP {:?})...\n",
        protocol, host, port, mountpoint, version
    );

    let mut config = make_config(host, port, mountpoint, use_https, version);
    config.username = user.map(String::from);
    config.password = pass.map(String::from);

    let mut client = NtripClient::new(config)?;
    client.connect().await?;

    println!("Connected! Reading data for 5 seconds...\n");

    let mut total_bytes = 0usize;
    let mut buf = [0u8; 4096];
    let start = std::time::Instant::now();

    while start.elapsed() < Duration::from_secs(5) {
        match tokio::time::timeout(Duration::from_secs(2), client.read_chunk(&mut buf)).await {
            Ok(Ok(n)) => {
                total_bytes += n;
                // Show first few bytes as hex for debugging
                if total_bytes <= 100 {
                    print!("  [{} bytes] ", n);
                    for b in &buf[..n.min(16)] {
                        print!("{:02X} ", b);
                    }
                    if n > 16 {
                        print!("...");
                    }
                    println!();
                }
            }
            Ok(Err(e)) => {
                println!("Read error: {}", e);
                break;
            }
            Err(_) => {
                println!("Read timeout");
            }
        }
    }

    println!("\nReceived {} bytes in 5 seconds", total_bytes);
    println!(
        "Average rate: {:.1} bytes/sec",
        total_bytes as f64 / 5.0
    );

    Ok(())
}

/// Test against known public casters
async fn cmd_test_casters() -> Result<(), NtripError> {
    println!("Testing NTRIP client against public casters...\n");
    println!("Testing {} casters...\n", PUBLIC_CASTERS.len());

    let mut success = 0;
    let mut failed = 0;

    for (host, port, description, region) in PUBLIC_CASTERS {
        println!("{:-<70}", "");
        println!("{} [{}] ({}:{})", description, region, host, port);
        println!("{:-<70}", "");

        let use_https = *port == 443;
        let config = make_config(host, *port, "", use_https, NtripVersion::Auto);

        match NtripClient::get_sourcetable(&config).await {
            Ok(table) => {
                success += 1;
                println!("  ✓ Sourcetable retrieved successfully");
                println!(
                    "    {} streams, {} casters, {} networks",
                    table.streams.len(),
                    table.casters.len(),
                    table.networks.len()
                );

                let rtcm_count = table.rtcm_streams().len();
                println!("    {} RTCM streams available", rtcm_count);

                // Show a few example mountpoints
                if !table.streams.is_empty() {
                    println!("    Sample mountpoints:");
                    for stream in table.streams.iter().take(3) {
                        println!(
                            "      - {} ({}) @ ({:.2}, {:.2})",
                            stream.mountpoint, stream.format, stream.latitude, stream.longitude
                        );
                    }
                }
            }
            Err(e) => {
                failed += 1;
                println!("  ✗ Failed: {}", e);
            }
        }
        println!();
    }

    println!("=== Summary ===");
    println!("Success: {}/{}", success, PUBLIC_CASTERS.len());
    println!("Failed:  {}/{}", failed, PUBLIC_CASTERS.len());
    Ok(())
}

/// Test NTRIP v1 vs v2 protocol against a caster
async fn cmd_test_versions(host: &str, port: u16) -> Result<(), NtripError> {
    println!("Testing NTRIP protocol versions against {}:{}...\n", host, port);

    for (version, name) in [
        (NtripVersion::V1, "NTRIP v1 (HTTP/1.0)"),
        (NtripVersion::V2, "NTRIP v2 (HTTP/1.1)"),
        (NtripVersion::Auto, "Auto-detect"),
    ] {
        println!("{:-<50}", "");
        println!("Testing: {}", name);
        println!("{:-<50}", "");

        let config = make_config(host, port, "", false, version);

        match NtripClient::get_sourcetable(&config).await {
            Ok(table) => {
                println!("  ✓ Success - {} streams found", table.streams.len());
            }
            Err(e) => {
                println!("  ✗ Failed: {}", e);
            }
        }
        println!();
    }

    Ok(())
}

/// Test nearest mountpoints from various world locations
async fn cmd_test_locations(host: &str, port: u16) -> Result<(), NtripError> {
    println!("Testing nearest mountpoints from various locations...\n");
    println!("Fetching sourcetable from {}:{}...", host, port);

    let config = make_config(host, port, "", false, NtripVersion::Auto);
    let table = NtripClient::get_sourcetable(&config).await?;

    println!("Found {} streams ({} RTCM)\n", table.streams.len(), table.rtcm_streams().len());

    if table.streams.is_empty() {
        println!("No streams available for distance testing.");
        return Ok(());
    }

    println!("{:<25} {:>10} {:>10}  {:<20} {:>10}", 
        "Location", "Lat", "Lon", "Nearest Mountpoint", "Distance");
    println!("{:-<85}", "");

    for (name, lat, lon) in TEST_LOCATIONS {
        if let Some((stream, dist)) = table.nearest_rtcm_stream(*lat, *lon) {
            println!(
                "{:<25} {:>10.4} {:>10.4}  {:<20} {:>8.1} km",
                name, lat, lon, 
                truncate(&stream.mountpoint, 20),
                dist
            );
        } else {
            println!(
                "{:<25} {:>10.4} {:>10.4}  {:<20}",
                name, lat, lon, "(no RTCM streams)"
            );
        }
    }

    Ok(())
}

/// List all known public casters
fn cmd_list_casters() {
    println!("Known Public NTRIP Casters:\n");
    println!("{:<30} {:>6}  {:<25} {:<15}", "Host", "Port", "Description", "Region");
    println!("{:-<80}", "");

    for (host, port, description, region) in PUBLIC_CASTERS {
        println!("{:<30} {:>6}  {:<25} {:<15}", host, port, description, region);
    }

    println!("\nTotal: {} casters", PUBLIC_CASTERS.len());
}

/// List all test locations
fn cmd_list_locations() {
    println!("Test Locations:\n");
    println!("{:<25} {:>12} {:>12}", "Name", "Latitude", "Longitude");
    println!("{:-<55}", "");

    for (name, lat, lon) in TEST_LOCATIONS {
        println!("{:<25} {:>12.4} {:>12.4}", name, lat, lon);
    }

    println!("\nTotal: {} locations", TEST_LOCATIONS.len());
}

fn print_sourcetable(table: &Sourcetable) {
    println!("=== Sourcetable Summary ===");
    println!(
        "Streams: {}  Casters: {}  Networks: {}",
        table.streams.len(),
        table.casters.len(),
        table.networks.len()
    );
    println!();

    if !table.casters.is_empty() {
        println!("--- Casters ---");
        for caster in &table.casters {
            println!("  {}:{} - {}", caster.host, caster.port, caster.identifier);
        }
        println!();
    }

    if !table.networks.is_empty() {
        println!("--- Networks ---");
        for network in &table.networks {
            println!("  {} - {}", network.identifier, network.operator);
        }
        println!();
    }

    let rtcm_streams = table.rtcm_streams();
    let rtcm_count = rtcm_streams.len();
    
    println!("--- Streams (RTCM): {} total ---", rtcm_count);
    
    // Only show first 5 streams to reduce verbosity
    if rtcm_count > 0 {
        println!(
            "{:<20} {:<15} {:<20} {:>10} {:>10}",
            "Mountpoint", "Format", "NavSys", "Lat", "Lon"
        );
        println!("{:-<80}", "");

        for stream in rtcm_streams.iter().take(5) {
            println!(
                "{:<20} {:<15} {:<20} {:>10.4} {:>10.4}",
                truncate(&stream.mountpoint, 20),
                truncate(&stream.format, 15),
                truncate(&stream.nav_system, 20),
                stream.latitude,
                stream.longitude
            );
        }
        
        if rtcm_count > 5 {
            println!("  ... and {} more streams", rtcm_count - 5);
        }
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() > max_len {
        format!("{}…", &s[..max_len - 1])
    } else {
        s.to_string()
    }
}
