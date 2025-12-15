# Detailed signal security (UBX-SEC-SIG) information
# Provides per-center-frequency jamming state in addition to high-level flags.

std_msgs/Header header

# High-level detector enable flags
bool jam_det_enabled
bool spf_det_enabled

# High-level states
uint8 jamming_state         # 0=unknown, 1=ok, 2=warning, 3=critical
uint8 spoofing_state        # 0=unknown, 1=ok, 2=indicated, 3=multiple

# Number of center frequencies provided in this message
uint8 jam_num_cent_freqs

# Per-center-frequency info (parallel arrays)
uint32[] cent_freq_khz
bool[] jammed
