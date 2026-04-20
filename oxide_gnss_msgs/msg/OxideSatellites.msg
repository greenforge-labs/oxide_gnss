# Satellite constellation status from NAV-SAT
# Provides per-satellite data and aggregated signal quality metrics

std_msgs/Header header

# Total number of satellites tracked
uint8 num_satellites

# Number of satellites used in navigation solution
uint8 num_used

# Per-satellite status (variable length array)
OxideSatellite[] satellites

# Aggregated signal quality metrics (computed from used satellites only)
# Mean C/N0 of satellites used in solution (dB-Hz)
float32 mean_cno

# Minimum C/N0 among used satellites (dB-Hz) - weakest link indicator
uint8 min_cno

# Number of used satellites with C/N0 >= 30 dB-Hz (usable signal threshold)
uint8 sats_above_threshold

# Per-constellation satellite counts (used in solution)
uint8 num_gps
uint8 num_glonass
uint8 num_galileo
uint8 num_beidou
uint8 num_sbas
uint8 num_qzss
