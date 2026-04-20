# Individual satellite status from NAV-SAT
# Provides per-satellite signal quality and usage information

# GNSS constellation identifier
uint8 gnss_id
uint8 GNSS_GPS = 0
uint8 GNSS_SBAS = 1
uint8 GNSS_GALILEO = 2
uint8 GNSS_BEIDOU = 3
uint8 GNSS_IMES = 4
uint8 GNSS_QZSS = 5
uint8 GNSS_GLONASS = 6
uint8 GNSS_NAVIC = 7

# Satellite vehicle ID (constellation-specific)
uint8 sv_id

# Signal strength (carrier-to-noise ratio, dB-Hz)
uint8 cno

# Elevation angle (degrees, -90 to +90)
int8 elevation

# Azimuth angle (degrees, 0 to 360)
int16 azimuth

# Pseudorange residual (0.1m resolution)
int16 pr_residual

# Satellite is used in current navigation solution
bool used_in_solution

# Signal quality indicator (0=none, 1-4=searching, 5-7=acquired/locked)
uint8 signal_quality

# Satellite health (0=unknown, 1=healthy, 2=unhealthy)
uint8 health
uint8 HEALTH_UNKNOWN = 0
uint8 HEALTH_HEALTHY = 1
uint8 HEALTH_UNHEALTHY = 2

# Orbit/ephemeris source (0=none, 1=ephemeris, 2=almanac, 3=other)
uint8 orbit_source
