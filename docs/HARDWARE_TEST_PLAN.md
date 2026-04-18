# Hardware Test Plan: oxide_gnss v0.1.0 Release Validation

## Context

oxide_gnss has 6 operating modes and 4 optional feature flags. All code prep is done on `ros2-release-prep`. Hardware testing across every mode is the sole remaining blocker before tagging v0.1.0. This plan covers the complete test matrix using available hardware.

---

## Available Hardware

| Item | Role |
|------|------|
| SparkFun ZED-F9P SMA #1 (labeled **BASE**) | Base station in paired modes |
| SparkFun ZED-F9P SMA #2 (labeled **ROVER**) | Rover in all modes |
| Adafruit FT232H (3.3V logic) | UART2 monitor / debug tap |
| 2x USB-C cables | F9P → PC (primary UBX link) |
| SMA GNSS antennas + cables | Satellite reception |
| PC running Ubuntu 24.04 / ROS2 Jazzy | Driver host |

**Required but not hardware:** NTRIP account (e.g., Geoscience Australia `ntrip.data.gnss.ga.gov.au`)

> **Warning:** The Hyperion HP-TI-PRGUSB adapter outputs 5V logic — DO NOT connect it to the F9P (will damage the 3.3V I/O). Use the Adafruit FT232H instead.

---

## Prerequisites (Do Once)

### 1. Build the driver
```bash
source /opt/ros/jazzy/setup.bash
cd ~/ros2_ws
colcon build --packages-up-to oxide_gnss_msgs oxide_gnss \
    --allow-overriding builtin_interfaces std_msgs geometry_msgs sensor_msgs diagnostic_msgs action_msgs
source install/setup.bash
```

### 2. Identify USB serial ports
Plug in both F9Ps via USB-C. Run:
```bash
ls /dev/ttyACM*
```
Note which port maps to which board. To create stable symlinks (recommended):
```bash
# Find serial numbers
udevadm info -a /dev/ttyACM0 | grep serial
udevadm info -a /dev/ttyACM1 | grep serial
```
Create `/etc/udev/rules.d/99-gnss.rules`:
```
SUBSYSTEM=="tty", ATTRS{idVendor}=="1546", ATTRS{idProduct}=="01a9", ATTRS{serial}=="<BASE_SERIAL>", SYMLINK+="gnss_f9p_base"
SUBSYSTEM=="tty", ATTRS{idVendor}=="1546", ATTRS{idProduct}=="01a9", ATTRS{serial}=="<ROVER_SERIAL>", SYMLINK+="gnss_f9p_rover"
```
Then `sudo udevadm control --reload-rules && sudo udevadm trigger`.

### 3. Set NTRIP credentials
```bash
export NTRIP_USERNAME="your_username"
export NTRIP_PASSWORD="your_password"
```

### 4. Antenna placement
- Place both antennas with clear sky view (outdoors or windowsill)
- For moving_base/moving_base_rover: separate antennas by at least 20 cm (further = better heading accuracy)
- Ensure both antennas can see the same constellation of satellites

### 5. UART2 wiring for paired modes
SparkFun ZED-F9P SMA boards expose UART2 on the 4-pin header (labeled on the PCB silkscreen). For base→rover RTCM:

```
F9P BASE UART2 TX  ──→  F9P ROVER UART2 RX
F9P BASE GND       ──→  F9P ROVER GND
```
(One-way is sufficient — RTCM flows base→rover only. Both boards are separately powered via USB-C.)

**FT232H debug tap** (optional, for verifying UART2 traffic):
```
F9P BASE UART2 TX  ──→  FT232H RX  (tap to monitor RTCM output)
FT232H GND         ──→  F9P BASE GND
```
Monitor with: `picocom -b 460800 /dev/ttyUSB0` (or 115200 depending on mode config)

---

## Test Execution Order

Tests are ordered from simplest (fewest dependencies) to most complex (paired receivers + NTRIP). Each test builds confidence for the next.

---

## TEST 1: Standalone Mode

**Hardware:** 1x F9P (ROVER), USB only, antenna with sky view
**Config file:** `config/standalone.yaml`
**Wiring:** USB-C only, no UART2

### Setup
Edit `config/standalone.yaml` — set `device.port` to your actual device (e.g., `/dev/gnss_f9p_rover` or `/dev/ttyACM0`).

### Launch
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/standalone.yaml \
    namespace:=gnss_rover \
    log_level:=info
```

### Verification Checklist

| # | Check | Command | Pass Criteria |
|---|-------|---------|---------------|
| 1.1 | Node starts without error | Observe terminal output | No panics, no config validation errors |
| 1.2 | ~/fix publishes | `ros2 topic hz /gnss_rover/fix` | ~10 Hz |
| 1.3 | Position is valid | `ros2 topic echo /gnss_rover/fix --once` | `latitude`/`longitude` non-zero, near your location |
| 1.4 | ~/velocity publishes | `ros2 topic hz /gnss_rover/velocity` | ~10 Hz |
| 1.5 | ~/time_reference publishes | `ros2 topic echo /gnss_rover/time_reference --once` | GPS timestamp populated |
| 1.6 | ~/satellites publishes | `ros2 topic echo /gnss_rover/satellites --once` | `num_satellites` > 0, `num_used` >= 4 |
| 1.7 | /diagnostics healthy | `ros2 topic echo /diagnostics --once` | Fix type = "3D", no error statuses |
| 1.8 | Fix type is 3D (not RTK) | `ros2 topic echo /gnss_rover/fix --once` | `status.status` = 0 (STATUS_FIX), not 2 (SBAS) |
| 1.9 | Covariance populated | `ros2 topic echo /gnss_rover/fix --once` | `position_covariance` has non-zero values |
| 1.10 | Excluded topics absent | `ros2 topic list \| grep gnss_rover` | No `~/integrity`, `~/operational`, `~/baseline_pose` |
| 1.11 | Clean shutdown | Ctrl+C the launch | No panics, tasks shut down gracefully |

---

## TEST 2: Rover NTRIP Mode

**Hardware:** 1x F9P (ROVER), USB only, antenna with sky view, internet connection
**Config file:** `config/rover_ntrip.yaml`
**Wiring:** USB-C only, no UART2

### Setup
Edit `config/rover_ntrip.yaml`:
- Set `device.port` to your device
- Set NTRIP `host`, `port`, `mountpoint` for a caster near your location
- Ensure `NTRIP_USERNAME` and `NTRIP_PASSWORD` env vars are set

### Launch
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/rover_ntrip.yaml \
    namespace:=gnss_rover \
    log_level:=info
```

### Verification Checklist

| # | Check | Command | Pass Criteria |
|---|-------|---------|---------------|
| 2.1 | Node starts, NTRIP connects | Observe terminal | "NTRIP connected" log message, no auth errors |
| 2.2 | ~/fix publishes at rate | `ros2 topic hz /gnss_rover/fix` | ~10 Hz |
| 2.3 | RTK Float achieved | `ros2 topic echo /gnss_rover/fix` | `status.status` = 2 (SBAS/DGPS) within ~30s |
| 2.4 | RTK Fixed achieved | `ros2 topic echo /gnss_rover/fix` | `status.status` = 2, covariance drops to cm-level (< 0.01 m^2) within ~60s |
| 2.5 | High-precision position | `ros2 topic echo /gnss_rover/fix --once` | Position accuracy visibly better than standalone (smaller covariance) |
| 2.6 | ~/integrity publishes | `ros2 topic echo /gnss_rover/integrity --once` | `level` = 0 (OK) when RTK fixed with good signal |
| 2.7 | ~/operational publishes | `ros2 topic echo /gnss_rover/operational --once` | `data` = true |
| 2.8 | Integrity checks pass | `ros2 topic echo /gnss_rover/integrity --once` | Individual `check_*_ok` fields are `true` |
| 2.9 | ~/velocity publishes | `ros2 topic hz /gnss_rover/velocity` | ~10 Hz |
| 2.10 | ~/time_reference publishes | `ros2 topic echo /gnss_rover/time_reference --once` | Timestamp populated |
| 2.11 | /diagnostics NTRIP status | `ros2 topic echo /diagnostics` | NTRIP status shows connected, correction age < 10s |
| 2.12 | Correction age reasonable | Check `/diagnostics` or `~/integrity` | `correction_age_s` updating, < 10s |
| 2.13 | NTRIP reconnection | Temporarily disable network, re-enable | Driver reconnects and resumes RTK within backoff period |
| 2.14 | Excluded topics absent | `ros2 topic list \| grep gnss_rover` | No `~/satellites` (disabled in config), no `~/baseline_pose` |
| 2.15 | Clean shutdown | Ctrl+C | NTRIP disconnects cleanly, no panics |

---

## TEST 3: Static Base Mode

**Hardware:** 1x F9P (BASE), USB only, antenna with sky view
**Config file:** `config/static_base.yaml`
**Wiring:** USB-C only (UART2 RTCM output tested passively with FT232H)

### Setup
Edit `config/static_base.yaml`:
- Set `device.port` to your BASE device

### Launch
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/static_base.yaml \
    namespace:=gnss_base \
    log_level:=info
```

### Verification Checklist

| # | Check | Command | Pass Criteria |
|---|-------|---------|---------------|
| 3.1 | Node starts without error | Observe terminal | Config validates, device connects |
| 3.2 | Survey-in starts | Observe logs | "Survey-in started" or NAV_SVIN messages in logs |
| 3.3 | ~/fix publishes | `ros2 topic hz /gnss_base/fix` | ~1 Hz (configured rate) |
| 3.4 | Position stabilizes | `ros2 topic echo /gnss_base/fix` | Position converges over survey-in period |
| 3.5 | Survey-in completes | Observe logs/diagnostics | Survey-in valid after min_duration_s (60s) and accuracy < 2.0m |
| 3.6 | ~/satellites publishes | `ros2 topic echo /gnss_base/satellites --once` | Satellite data present |
| 3.7 | /diagnostics healthy | `ros2 topic echo /diagnostics` | Fix type = "3D" or "Time" |
| 3.8 | RTCM on UART2 (optional) | Connect FT232H RX to BASE UART2 TX, monitor with `picocom -b 115200 /dev/ttyUSB0` | Binary RTCM data visible (non-printable bytes flowing) |
| 3.9 | Excluded topics absent | `ros2 topic list \| grep gnss_base` | No `~/integrity`, `~/operational`, `~/baseline_pose` |
| 3.10 | Clean shutdown | Ctrl+C | Graceful shutdown |

---

## TEST 4: Static Base + Rover Radio (Paired)

**Hardware:** 2x F9P (BASE + ROVER), UART2 crossover wire, both antennas with sky view
**Config files:** `config/static_base.yaml` (base) + `config/rover_radio.yaml` (rover)
**Wiring:**
```
F9P BASE UART2 TX  ──→  F9P ROVER UART2 RX
F9P BASE GND       ──→  F9P ROVER GND
Both F9Ps connected to PC via USB-C (two cables)
```

### Setup
- Edit `config/static_base.yaml`: `device.port` → BASE device, `uart2.baudrate` → **115200**
- Edit `config/rover_radio.yaml`: `device.port` → ROVER device, `uart2.baudrate` → **115200** (must match base)

### Launch (two terminals)

**Terminal 1 — Base:**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/static_base.yaml \
    namespace:=gnss_base \
    log_level:=info
```

**Terminal 2 — Rover (start after base survey-in completes):**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/rover_radio.yaml \
    namespace:=gnss_rover \
    log_level:=info
```

### Verification Checklist

| # | Check | Command | Pass Criteria |
|---|-------|---------|---------------|
| 4.1 | Base survey-in completes | Watch gnss_base logs | Survey-in valid |
| 4.2 | Base RTCM flowing on UART2 | (Optional) FT232H tap on UART2 TX | Binary data flowing |
| 4.3 | Rover receives corrections | Watch gnss_rover logs | RTCM messages being processed |
| 4.4 | Rover achieves RTK Float | `ros2 topic echo /gnss_rover/fix` | Fix improves beyond standalone |
| 4.5 | Rover achieves RTK Fixed | `ros2 topic echo /gnss_rover/fix` | cm-level covariance, RTK Fixed status |
| 4.6 | Rover ~/integrity OK | `ros2 topic echo /gnss_rover/integrity --once` | `level` = 0, `check_correction_age_ok` = true |
| 4.7 | Rover position matches base vicinity | Compare base and rover positions | Within expected distance for antenna separation |
| 4.8 | Both nodes run concurrently | `ros2 node list` | Both `/gnss_base/gnss_node` and `/gnss_rover/gnss_node` present |
| 4.9 | Topic namespacing correct | `ros2 topic list` | Base topics under `/gnss_base/`, rover under `/gnss_rover/` |
| 4.10 | Clean dual shutdown | Ctrl+C both terminals | Both shut down gracefully |

---

## TEST 5: Moving Base + Moving Base Rover (Paired)

**Hardware:** 2x F9P (BASE + ROVER), UART2 crossover wire, both antennas with sky view, separated by known distance (measure it)
**Config files:** `config/moving_base.yaml` (base) + `config/moving_base_rover.yaml` (rover)
**Wiring:**
```
F9P BASE UART2 TX  ──→  F9P ROVER UART2 RX
F9P BASE GND       ──→  F9P ROVER GND
Both F9Ps connected to PC via USB-C
```

### Setup
- Edit `config/moving_base.yaml`: `device.port` → BASE device, `uart2.baudrate` → **460800**
- Edit `config/moving_base_rover.yaml`: `device.port` → ROVER device, `uart2.baudrate` → **460800** (must match)
- Comment out the `ntrip:` section in `moving_base.yaml` for initial test (add NTRIP back in step 5.11)
- Measure and record the physical distance between the two antennas

### Launch (two terminals)

**Terminal 1 — Moving Base:**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/moving_base.yaml \
    namespace:=gnss_base \
    log_level:=info
```

**Terminal 2 — Moving Base Rover:**
```bash
ros2 launch oxide_gnss oxide_gnss.launch.py \
    config_file:=$(pwd)/config/moving_base_rover.yaml \
    namespace:=gnss_rover \
    log_level:=info
```

### Verification Checklist

| # | Check | Command | Pass Criteria |
|---|-------|---------|---------------|
| 5.1 | Both nodes start | Observe both terminals | No config errors, devices connect |
| 5.2 | Base ~/fix publishes | `ros2 topic hz /gnss_base/fix` | ~5 Hz |
| 5.3 | Rover ~/fix publishes | `ros2 topic hz /gnss_rover/fix` | ~5 Hz |
| 5.4 | Rover achieves RTK Fixed | `ros2 topic echo /gnss_rover/fix` | RTK Fixed status, cm-level covariance |
| 5.5 | ~/baseline_pose publishes | `ros2 topic hz /gnss_rover/baseline_pose` | ~5 Hz |
| 5.6 | Baseline distance correct | `ros2 topic echo /gnss_rover/baseline_pose --once` | `pose.position` x/y/z magnitude matches measured antenna separation |
| 5.7 | Heading quaternion valid | `ros2 topic echo /gnss_rover/baseline_pose --once` | Quaternion norm = 1.0, heading stable |
| 5.8 | Heading direction plausible | Compute yaw from quaternion | Matches physical antenna arrangement direction |
| 5.9 | Base ~/integrity OK | `ros2 topic echo /gnss_base/integrity --once` | `level` = 0 or 1 |
| 5.10 | Rover ~/integrity OK | `ros2 topic echo /gnss_rover/integrity --once` | `level` = 0, all checks pass |
| 5.11 | (Optional) Add NTRIP to base | Uncomment `ntrip:` in moving_base.yaml, restart base | Base gets absolute RTK fix, rover inherits improved accuracy |
| 5.12 | Heading rotates with physical rotation | Carefully rotate antenna arrangement | Heading quaternion yaw changes to match |
| 5.13 | Clean dual shutdown | Ctrl+C both terminals | Graceful shutdown |

---

## TEST 6: Feature Flag Edge Cases

Run these as variations of the above modes to verify feature gating works correctly.

| # | Test | Mode | Config Change | Pass Criteria |
|---|------|------|--------------|---------------|
| 6.1 | All features off | `standalone` | `satellites: false` | Only ~/fix, ~/velocity, ~/time_reference publish |
| 6.2 | Satellites on rover_ntrip | `rover_ntrip` | `satellites: true` | ~/satellites topic appears and publishes |
| 6.3 | Integrity off on rover_ntrip | `rover_ntrip` | `integrity: false` | No ~/integrity or ~/operational topics |
| 6.4 | Heading off on MB rover | `moving_base_rover` | `heading: false` | No ~/baseline_pose topic |
| 6.5 | Invalid feature rejected | `standalone` | `integrity: true` | Driver rejects config at startup (integrity not allowed in standalone) |

---

## TEST 7: Error Handling & Reconnection

| # | Test | Procedure | Pass Criteria |
|---|------|-----------|---------------|
| 7.1 | Device disconnect/reconnect | Unplug USB-C from F9P while running, wait 5s, replug | Driver detects disconnect, reconnects with backoff, resumes publishing |
| 7.2 | NTRIP network loss | Disable WiFi/ethernet during rover_ntrip test, re-enable | NTRIP reconnects with exponential backoff, RTK resumes |
| 7.3 | Invalid serial port | Set `device.port` to `/dev/ttyNONEXIST` | Clean error message, no panic |
| 7.4 | Invalid NTRIP credentials | Set wrong password | Clean error message in logs, node stays running (device still works without corrections) |
| 7.5 | UART2 baud mismatch | Set base UART2 to 460800, rover to 115200 | Rover doesn't crash; just fails to achieve RTK (no valid RTCM received) |
| 7.6 | Watchdog timeout | Cover antenna completely (no satellites) | Fix degrades gracefully, diagnostics reflect "No Fix" |

---

## General Notes

- **Test outdoors or near a window.** Indoor testing with poor sky view will make RTK modes fail to converge, which is expected but unhelpful for validation.
- **Wait for convergence.** RTK Fixed can take 30-120s after corrections start flowing. Don't declare failure too early.
- **UART2 baud rates must match** between paired devices. The config files use 115200 for static_base/rover_radio and 460800 for moving_base pairs.
- **Namespace separation** (`gnss_base` vs `gnss_rover`) is critical when running two driver instances on the same machine.
- **FT232H as debug tool:** Connect FT232H RX to a UART2 TX line to passively sniff RTCM traffic without affecting the link. Useful for confirming data is actually flowing on the wire.
