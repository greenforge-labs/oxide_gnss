# GNSS Integrity Status Message
# Provides safety-critical integrity monitoring data for GNSS solutions

# Header with timestamp
std_msgs/Header header

# Overall integrity level (0=OK, 1=DEGRADED, 2=CRITICAL, 3=FAILED)
uint8 level
uint8 LEVEL_OK = 0
uint8 LEVEL_DEGRADED = 1
uint8 LEVEL_CRITICAL = 2
uint8 LEVEL_FAILED = 3

# Human-readable status message
string status_message

# Operational status (true if level is OK or DEGRADED)
bool operational

# Position quality
uint8 fix_type              # 0=none, 1=dead-reck, 2=2D, 3=3D, 4=GNSS+DR, 5=time-only, 6=RTK-float, 7=RTK-fixed
uint8 carrier_solution      # 0=none, 1=float, 2=fixed
bool differential_applied   # Differential corrections applied
uint8 num_satellites        # Number of satellites used
float32 h_accuracy_m        # Horizontal accuracy (m)
float32 v_accuracy_m        # Vertical accuracy (m)
float32 pdop                # Position dilution of precision

# Covariance data
bool covariance_valid       # True if covariance data is valid
float64[9] position_covariance   # ENU position covariance (row-major 3x3)
float64[9] velocity_covariance   # ENU velocity covariance (row-major 3x3)

# RTK/Correction status
float32 correction_age_s    # Age of differential corrections (-1 if no RTK)
bool correction_received    # RTCM/correction data received
bool correction_used        # Correction data was used by receiver

# Heading quality (dual-antenna)
bool heading_valid          # Heading solution valid
uint8 heading_carrier_solution  # 0=none, 1=float, 2=fixed
float32 baseline_m          # Measured baseline length (m)
float32 baseline_expected_m # Expected baseline length (m)
float32 baseline_accuracy_m # Baseline accuracy (m)
float32 heading_accuracy_deg # Heading accuracy (deg)

# Security status
uint8 jamming_state         # 0=unknown, 1=ok, 2=warning, 3=critical
uint8 JAMMING_UNKNOWN = 0
uint8 JAMMING_OK = 1
uint8 JAMMING_WARNING = 2
uint8 JAMMING_CRITICAL = 3

uint8 spoofing_state        # 0=unknown, 1=ok, 2=indicated, 3=multiple
uint8 SPOOFING_UNKNOWN = 0
uint8 SPOOFING_OK = 1
uint8 SPOOFING_INDICATED = 2
uint8 SPOOFING_MULTIPLE = 3

uint32 security_events      # Number of security events logged
uint8 jamming_indicator     # CW jamming indicator (0-255 scale from MON-HW)

# Hardware status
uint8 antenna_status        # 0=unknown, 1=ok, 2=open, 3=short
uint8 ANTENNA_UNKNOWN = 0
uint8 ANTENNA_OK = 1
uint8 ANTENNA_OPEN = 2
uint8 ANTENNA_SHORT = 3

# Communication status
uint8 comm_ports            # Number of communication ports
uint8 comm_tx_errors        # TX errors detected
