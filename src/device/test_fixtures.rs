//! Test fixtures for UBX protocol testing.
//!
//! This module provides:
//! - Real UBX packet byte sequences for parsing tests
//! - Helper functions to create test data structures
//! - Invalid/malformed packets for error path testing
//!
//! # Packet Format Reference
//!
//! UBX packets have the following structure:
//! - Sync chars: 0xB5 0x62
//! - Class: 1 byte
//! - ID: 1 byte
//! - Length: 2 bytes (little-endian, payload only)
//! - Payload: variable
//! - Checksum: 2 bytes (Fletcher-8 over class..payload)

#[cfg(test)]
pub mod ubx_packets {
    //! Raw UBX packet byte sequences for protocol testing.

    /// Calculate UBX checksum (Fletcher-8) over class, id, length, and payload.
    pub fn ubx_checksum(data: &[u8]) -> (u8, u8) {
        let mut ck_a: u8 = 0;
        let mut ck_b: u8 = 0;
        for byte in data {
            ck_a = ck_a.wrapping_add(*byte);
            ck_b = ck_b.wrapping_add(ck_a);
        }
        (ck_a, ck_b)
    }

    /// Build a complete UBX packet with proper checksum.
    pub fn build_ubx_packet(class: u8, id: u8, payload: &[u8]) -> Vec<u8> {
        let len = payload.len() as u16;
        let mut packet = vec![0xB5, 0x62, class, id];
        packet.extend_from_slice(&len.to_le_bytes());
        packet.extend_from_slice(payload);

        // Checksum over class, id, length, payload
        let (ck_a, ck_b) = ubx_checksum(&packet[2..]);
        packet.push(ck_a);
        packet.push(ck_b);

        packet
    }

    /// Valid NAV-PVT packet (class 0x01, id 0x07, 92 bytes payload).
    ///
    /// This represents a 3D fix with RTK Float carrier solution.
    /// - Position: ~37.7749°N, 122.4194°W (San Francisco area)
    /// - 12 satellites, RTK Float
    /// - Horizontal accuracy: 0.5m
    pub fn nav_pvt_rtk_float() -> Vec<u8> {
        // NAV-PVT payload structure (92 bytes):
        // iTOW(4), year(2), month(1), day(1), hour(1), min(1), sec(1), valid(1),
        // tAcc(4), nano(4), fixType(1), flags(1), flags2(1), numSV(1),
        // lon(4), lat(4), height(4), hMSL(4), hAcc(4), vAcc(4),
        // velN(4), velE(4), velD(4), gSpeed(4), headMot(4),
        // sAcc(4), headAcc(4), pDOP(2), flags3(2), reserved(4), headVeh(4), magDec(2), magAcc(2)
        #[rustfmt::skip]
        let payload: [u8; 92] = [
            // iTOW: 123456000 ms (0x075BCD00)
            0x00, 0xCD, 0x5B, 0x07,
            // year: 2024 (0x07E8)
            0xE8, 0x07,
            // month: 6, day: 15, hour: 14, min: 30, sec: 45
            0x06, 0x0F, 0x0E, 0x1E, 0x2D,
            // valid: 0x37 (validDate, validTime, fullyResolved, validMag)
            0x37,
            // tAcc: 50ns (0x32)
            0x32, 0x00, 0x00, 0x00,
            // nano: 123456789 (0x075BCD15)
            0x15, 0xCD, 0x5B, 0x07,
            // fixType: 3 (3D fix)
            0x03,
            // flags: 0x41 (gnssFixOK=1, carrSoln=1 RTK Float in bits 6-7)
            0x41,
            // flags2: 0x20 (confirmedAvail)
            0x20,
            // numSV: 12
            0x0C,
            // lon: -122.4194° = -1224194000 (0xB7108E30 as i32)
            0x30, 0x8E, 0x10, 0xB7,
            // lat: 37.7749° = 377749000 (0x168435C8)
            0xC8, 0x35, 0x84, 0x16,
            // height: 50000mm = 50m (0x0000C350)
            0x50, 0xC3, 0x00, 0x00,
            // hMSL: 30000mm = 30m (0x00007530)
            0x30, 0x75, 0x00, 0x00,
            // hAcc: 500mm = 0.5m (0x000001F4)
            0xF4, 0x01, 0x00, 0x00,
            // vAcc: 800mm = 0.8m (0x00000320)
            0x20, 0x03, 0x00, 0x00,
            // velN: 100 mm/s = 0.1 m/s (0x00000064)
            0x64, 0x00, 0x00, 0x00,
            // velE: 50 mm/s = 0.05 m/s (0x00000032)
            0x32, 0x00, 0x00, 0x00,
            // velD: -20 mm/s = -0.02 m/s (0xFFFFFFEC)
            0xEC, 0xFF, 0xFF, 0xFF,
            // gSpeed: 112 mm/s (0x00000070)
            0x70, 0x00, 0x00, 0x00,
            // headMot: 45.0° = 4500000 (0x0044AA20) in 1e-5 deg
            0x20, 0xAA, 0x44, 0x00,
            // sAcc: 50 mm/s (0x00000032)
            0x32, 0x00, 0x00, 0x00,
            // headAcc: 5.0° = 500000 (0x0007A120) in 1e-5 deg
            0x20, 0xA1, 0x07, 0x00,
            // pDOP: 1.5 = 150 (0x0096)
            0x96, 0x00,
            // flags3: 0x02 (lastCorrectionAge = 1 = 0-1s)
            0x02, 0x00,
            // reserved: 0
            0x00, 0x00, 0x00, 0x00,
            // headVeh: 0
            0x00, 0x00, 0x00, 0x00,
            // magDec: 0, magAcc: 0
            0x00, 0x00, 0x00, 0x00,
        ];

        build_ubx_packet(0x01, 0x07, &payload)
    }

    /// Valid NAV-PVT packet with RTK Fixed carrier solution.
    pub fn nav_pvt_rtk_fixed() -> Vec<u8> {
        let mut packet = nav_pvt_rtk_float();
        // Modify flags byte to indicate RTK Fixed (carrSoln=2 in bits 6-7)
        // flags is at offset 6 (header) + 20 (payload offset) = 26
        // Actually: sync(2) + class(1) + id(1) + len(2) + payload_offset(20) = 26
        packet[26] = 0x81; // gnssFixOK=1, carrSoln=2 (Fixed)

        // Recalculate checksum
        let (ck_a, ck_b) = ubx_checksum(&packet[2..packet.len() - 2]);
        let len = packet.len();
        packet[len - 2] = ck_a;
        packet[len - 1] = ck_b;

        packet
    }

    /// Valid NAV-PVT packet with no fix.
    pub fn nav_pvt_no_fix() -> Vec<u8> {
        #[rustfmt::skip]
        let payload: [u8; 92] = [
            // iTOW
            0x00, 0xCD, 0x5B, 0x07,
            // year, month, day, hour, min, sec
            0xE8, 0x07, 0x06, 0x0F, 0x0E, 0x1E, 0x2D,
            // valid: 0x00 (no valid time/date)
            0x00,
            // tAcc
            0x00, 0x00, 0x00, 0x00,
            // nano
            0x00, 0x00, 0x00, 0x00,
            // fixType: 0 (no fix)
            0x00,
            // flags: 0x00 (no fix)
            0x00,
            // flags2: 0x00
            0x00,
            // numSV: 2 (too few satellites)
            0x02,
            // lon, lat, height, hMSL (all zeros - no valid position)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            // hAcc, vAcc (very large = unknown)
            0xFF, 0xFF, 0xFF, 0x7F,
            0xFF, 0xFF, 0xFF, 0x7F,
            // velocities (zeros)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            // sAcc, headAcc
            0xFF, 0xFF, 0xFF, 0x7F,
            0xFF, 0xFF, 0xFF, 0x7F,
            // pDOP: 99.99
            0x0F, 0x27,
            // flags3, reserved, headVeh, magDec, magAcc
            0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        build_ubx_packet(0x01, 0x07, &payload)
    }

    /// Valid NAV-HPPOSLLH packet (class 0x01, id 0x14).
    ///
    /// High-precision position with cm-level accuracy.
    pub fn nav_hpposllh_valid() -> Vec<u8> {
        // NAV-HPPOSLLH payload (36 bytes):
        // version(1), reserved(2), flags(1), iTOW(4),
        // lon(4), lat(4), height(4), hMSL(4),
        // lonHp(1), latHp(1), heightHp(1), hMSLHp(1),
        // hAcc(4), vAcc(4)
        #[rustfmt::skip]
        let payload: [u8; 36] = [
            // version: 0
            0x00,
            // reserved
            0x00, 0x00,
            // flags: 0x03 (invalidLlh=0, invalidHeight=0)
            0x03,
            // iTOW
            0x00, 0xCD, 0x5B, 0x07,
            // lon: -122.4194° = -1224194000
            0x30, 0x8E, 0x10, 0xB7,
            // lat: 37.7749° = 377749000
            0xC8, 0x35, 0x84, 0x16,
            // height: 50000 mm
            0x50, 0xC3, 0x00, 0x00,
            // hMSL: 30000 mm
            0x30, 0x75, 0x00, 0x00,
            // lonHp: 50 (0.5mm)
            0x32,
            // latHp: 25 (0.25mm)
            0x19,
            // heightHp: 10 (0.1mm)
            0x0A,
            // hMSLHp: 5
            0x05,
            // hAcc: 15 mm (0.1mm units) = 1.5mm accuracy
            0x96, 0x00, 0x00, 0x00,
            // vAcc: 25 mm (0.1mm units) = 2.5mm accuracy
            0xFA, 0x00, 0x00, 0x00,
        ];

        build_ubx_packet(0x01, 0x14, &payload)
    }

    /// Valid NAV-SAT packet with 4 satellites.
    pub fn nav_sat_4_sats() -> Vec<u8> {
        // NAV-SAT payload:
        // iTOW(4), version(1), numSvs(1), reserved(2),
        // then numSvs * 12 bytes per satellite
        let num_svs: u8 = 4;
        let mut payload = vec![
            // iTOW
            0x00, 0xCD, 0x5B, 0x07,    // version: 1
            0x01,    // numSvs
            num_svs, // reserved
            0x00, 0x00,
        ];

        // Add satellite entries (12 bytes each)
        // Format: gnssId(1), svId(1), cno(1), elev(1), azim(2), prRes(2), flags(4)
        let satellites: [(u8, u8, u8, i8, i16, i16, u32); 4] = [
            // GPS G01: strong signal, used
            (0, 1, 45, 75, 120, 10, 0x0000001F), // svUsed=1
            // GPS G05: good signal, used
            (0, 5, 38, 45, 240, -5, 0x0000001F),
            // GLONASS R07: moderate signal, used
            (6, 7, 32, 30, 60, 15, 0x0000001F),
            // Galileo E11: weak signal, not used
            (2, 11, 22, 15, 300, -20, 0x00000017), // svUsed=0
        ];

        for (gnss_id, sv_id, cno, elev, azim, pr_res, flags) in satellites {
            payload.push(gnss_id);
            payload.push(sv_id);
            payload.push(cno);
            payload.push(elev as u8);
            payload.extend_from_slice(&azim.to_le_bytes());
            payload.extend_from_slice(&pr_res.to_le_bytes());
            payload.extend_from_slice(&flags.to_le_bytes());
        }

        build_ubx_packet(0x01, 0x35, &payload)
    }

    /// Valid NAV-COV packet (position/velocity covariance).
    pub fn nav_cov_valid() -> Vec<u8> {
        // NAV-COV payload (64 bytes):
        // iTOW(4), version(1), posCovValid(1), velCovValid(1), reserved(9),
        // posCovNN(4), posCovNE(4), posCovND(4), posCovEE(4), posCovED(4), posCovDD(4),
        // velCovNN(4), velCovNE(4), velCovND(4), velCovEE(4), velCovED(4), velCovDD(4)
        #[rustfmt::skip]
        let payload: [u8; 64] = [
            // iTOW
            0x00, 0xCD, 0x5B, 0x07,
            // version
            0x00,
            // posCovValid: 1
            0x01,
            // velCovValid: 1
            0x01,
            // reserved (9 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Position covariance (m^2) - diagonal dominant
            // posCovNN: 0.25 (as f32 LE)
            0x00, 0x00, 0x80, 0x3E,
            // posCovNE: 0.01
            0x0A, 0xD7, 0x23, 0x3C,
            // posCovND: 0.02
            0x0A, 0xD7, 0xA3, 0x3C,
            // posCovEE: 0.25
            0x00, 0x00, 0x80, 0x3E,
            // posCovED: 0.015
            0x9A, 0x99, 0x79, 0x3C,
            // posCovDD: 0.64
            0x29, 0x5C, 0x24, 0x3F,
            // Velocity covariance (m^2/s^2)
            // velCovNN: 0.01
            0x0A, 0xD7, 0x23, 0x3C,
            // velCovNE: 0.001
            0x6F, 0x12, 0x83, 0x3A,
            // velCovND: 0.001
            0x6F, 0x12, 0x83, 0x3A,
            // velCovEE: 0.01
            0x0A, 0xD7, 0x23, 0x3C,
            // velCovED: 0.001
            0x6F, 0x12, 0x83, 0x3A,
            // velCovDD: 0.04
            0x0A, 0xD7, 0x23, 0x3D,
        ];

        build_ubx_packet(0x01, 0x36, &payload)
    }

    /// Valid ACK-ACK packet for CFG-VALSET.
    pub fn ack_ack_cfg_valset() -> Vec<u8> {
        // ACK-ACK: class 0x05, id 0x01
        // Payload: clsID(1), msgID(1) of acknowledged message
        let payload = [0x06, 0x8A]; // CFG-VALSET
        build_ubx_packet(0x05, 0x01, &payload)
    }

    /// Valid ACK-NAK packet for CFG-VALSET.
    pub fn ack_nak_cfg_valset() -> Vec<u8> {
        // ACK-NAK: class 0x05, id 0x00
        let payload = [0x06, 0x8A]; // CFG-VALSET
        build_ubx_packet(0x05, 0x00, &payload)
    }

    /// Valid SEC-SIG packet (jamming/spoofing status).
    pub fn sec_sig_ok() -> Vec<u8> {
        // SEC-SIG payload (8 + variable bytes for center frequencies)
        // sigSecFlags(4), reserved(2), jamNumCentFreqs(1), reserved(1),
        // then jamNumCentFreqs * centFreq entries
        #[rustfmt::skip]
        let payload: [u8; 8] = [
            // sigSecFlags: jamDetEnabled=1, spfDetEnabled=1, jammingState=OK(1), spoofingState=OK(1)
            // Bit layout: [0]=jamDetEnabled, [1]=spfDetEnabled, [4:2]=jammingState, [6:5]=spoofingState
            0x07, 0x00, 0x00, 0x00,
            // reserved
            0x00, 0x00,
            // jamNumCentFreqs: 0
            0x00,
            // reserved
            0x00,
        ];

        build_ubx_packet(0x27, 0x09, &payload)
    }

    /// SEC-SIG packet indicating jamming warning.
    pub fn sec_sig_jamming_warning() -> Vec<u8> {
        #[rustfmt::skip]
        let payload: [u8; 8] = [
            // sigSecFlags: jamDetEnabled=1, spfDetEnabled=1, jammingState=Warning(2), spoofingState=OK(1)
            // jammingState in bits [4:2] = 2, spoofingState in bits [6:5] = 1
            0x0B, 0x00, 0x00, 0x00, // 0x0B = 0b00001011 (jamDet=1, spfDet=1, jamState=2)
            0x00, 0x00,
            0x00,
            0x00,
        ];

        build_ubx_packet(0x27, 0x09, &payload)
    }

    /// Valid MON-HW packet (hardware status).
    pub fn mon_hw_ok() -> Vec<u8> {
        // MON-HW payload (60 bytes)
        #[rustfmt::skip]
        let payload: [u8; 60] = [
            // pinSel(4), pinBank(4), pinDir(4), pinVal(4)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
            // noisePerMS(2): 100
            0x64, 0x00,
            // agcCnt(2): 5000
            0x88, 0x13,
            // aStatus: 2 (OK)
            0x02,
            // aPower: 1 (On)
            0x01,
            // flags: 0x01 (rtcCalib=1, safeBoot=0, jammingState=OK)
            0x01,
            // reserved
            0x00,
            // usedMask(4)
            0xFF, 0xFF, 0xFF, 0xFF,
            // VP(17 bytes)
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00,
            // jamInd: 20 (low jamming)
            0x14,
            // reserved(2)
            0x00, 0x00,
            // pinIrq(4)
            0x00, 0x00, 0x00, 0x00,
            // pullH(4), pullL(4)
            0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00,
        ];

        build_ubx_packet(0x0A, 0x09, &payload)
    }

    // =========================================================================
    // Error case packets
    // =========================================================================

    /// NAV-PVT packet with invalid checksum.
    pub fn nav_pvt_bad_checksum() -> Vec<u8> {
        let mut packet = nav_pvt_rtk_float();
        // Corrupt the checksum
        let len = packet.len();
        packet[len - 2] = 0xFF;
        packet[len - 1] = 0xFF;
        packet
    }

    /// Truncated NAV-PVT packet (missing last 10 bytes).
    pub fn nav_pvt_truncated() -> Vec<u8> {
        let packet = nav_pvt_rtk_float();
        packet[..packet.len() - 12].to_vec()
    }

    /// Unknown message class/ID.
    pub fn unknown_message() -> Vec<u8> {
        // Use reserved class 0xFF
        let payload = [0x00, 0x01, 0x02, 0x03];
        build_ubx_packet(0xFF, 0xFF, &payload)
    }

    /// Empty payload message (valid structure but no data).
    pub fn empty_payload() -> Vec<u8> {
        build_ubx_packet(0x01, 0x07, &[])
    }

    /// Multiple concatenated packets (for stream testing).
    pub fn multiple_packets() -> Vec<u8> {
        let mut data = nav_pvt_rtk_float();
        data.extend(nav_hpposllh_valid());
        data.extend(ack_ack_cfg_valset());
        data
    }

    /// Garbage data followed by valid packet.
    pub fn garbage_then_valid() -> Vec<u8> {
        let mut data = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x01, 0x02, 0x03];
        data.extend(nav_pvt_rtk_float());
        data
    }
}

#[cfg(test)]
pub mod helpers {
    //! Helper functions to create test data structures.

    use std::time::Instant;

    use crate::device::ubx::{
        AntennaPowerData, AntennaStatusData, CarrierSolution, CovData, HpPosData, JammingStateData,
        MonHwData, PvtData, SatInfo, SatStatus, SecSigData, SpoofingStateData,
    };
    use crate::state::FixType;

    /// Create a sample PvtData with default values (3D fix).
    pub fn make_pvt() -> PvtData {
        make_pvt_with_fix(FixType::Fix3D)
    }

    /// Create a PvtData with specified fix type.
    pub fn make_pvt_with_fix(fix_type: FixType) -> PvtData {
        let carr_soln = match fix_type {
            FixType::RtkFixed => CarrierSolution::Fixed,
            FixType::RtkFloat => CarrierSolution::Float,
            _ => CarrierSolution::None,
        };

        PvtData {
            itow: 123456000,
            year: 2024,
            month: 6,
            day: 15,
            hour: 14,
            min: 30,
            sec: 45,
            nano: 123456789,
            fix_type,
            num_sv: 12,
            lon: -122.4194,
            lat: 37.7749,
            height: 50.0,
            height_msl: 30.0,
            h_acc: 0.5,
            v_acc: 0.8,
            vel_n: 0.1,
            vel_e: 0.05,
            vel_d: -0.02,
            g_speed: 0.112,
            head_mot: 45.0,
            s_acc: 0.05,
            head_acc: 5.0,
            p_dop: 1.5,
            carr_soln,
            diff_corr_age_s: Some(1),
            received_at: Instant::now(),
        }
    }

    /// Create a PvtData with RTK Fixed solution.
    pub fn make_pvt_rtk_fixed() -> PvtData {
        make_pvt_with_fix(FixType::RtkFixed)
    }

    /// Create a PvtData with RTK Float solution.
    pub fn make_pvt_rtk_float() -> PvtData {
        make_pvt_with_fix(FixType::RtkFloat)
    }

    /// Create a PvtData with no fix.
    pub fn make_pvt_no_fix() -> PvtData {
        let mut pvt = make_pvt_with_fix(FixType::NoFix);
        pvt.num_sv = 2;
        pvt.h_acc = f32::MAX;
        pvt.v_acc = f32::MAX;
        pvt.p_dop = 99.99;
        pvt.diff_corr_age_s = None;
        pvt
    }

    /// Create a sample HpPosData.
    pub fn make_hp_pos() -> HpPosData {
        HpPosData {
            lat: 37.7749005,
            lon: -122.4194002,
            height: 50.001,
            h_acc: 0.015, // 15mm
            v_acc: 0.025, // 25mm
        }
    }

    /// Create a sample HpPosData with specified accuracy.
    pub fn make_hp_pos_with_accuracy(h_acc: f32, v_acc: f32) -> HpPosData {
        HpPosData {
            lat: 37.7749005,
            lon: -122.4194002,
            height: 50.001,
            h_acc,
            v_acc,
        }
    }

    /// Create a SatInfo with specified satellite count.
    pub fn make_sat_info(count: usize) -> SatInfo {
        let mut sats = Vec::with_capacity(count);
        for i in 0..count {
            sats.push(SatStatus {
                gnss_id: (i % 4) as u8, // Cycle through GPS(0), SBAS(1), Galileo(2), BeiDou(3)
                sv_id: (i + 1) as u8,
                cno: 35 + (i % 15) as u8, // 35-49 dB-Hz
                elev: (30 + (i * 5) % 60) as i8,
                azim: ((i * 30) % 360) as i16,
                pr_res: ((i % 20) as i16) - 10,
                flags: 0x0000001F,      // All quality flags set, svUsed=1
                sv_used: i < count - 1, // Last satellite not used
            });
        }

        SatInfo {
            num_sats: count as u8,
            sats,
        }
    }

    /// Create a SatInfo with weak signals.
    pub fn make_sat_info_weak() -> SatInfo {
        let sats = vec![
            SatStatus {
                gnss_id: 0,
                sv_id: 1,
                cno: 22, // Below 30 dB-Hz threshold
                elev: 15,
                azim: 120,
                pr_res: 5,
                flags: 0x0000001F,
                sv_used: true,
            },
            SatStatus {
                gnss_id: 0,
                sv_id: 5,
                cno: 18, // Very weak
                elev: 10,
                azim: 240,
                pr_res: -8,
                flags: 0x0000001F,
                sv_used: true,
            },
        ];

        SatInfo { num_sats: 2, sats }
    }

    /// Create a sample CovData with valid covariances.
    pub fn make_cov() -> CovData {
        CovData {
            itow: 123456000,
            pos_cov_valid: true,
            vel_cov_valid: true,
            pos_cov: [0.25, 0.01, 0.02, 0.25, 0.015, 0.64], // ~0.5m, ~0.5m, ~0.8m
            vel_cov: [0.01, 0.001, 0.001, 0.01, 0.001, 0.04],
        }
    }

    /// Create a CovData with invalid position covariance.
    pub fn make_cov_invalid() -> CovData {
        CovData {
            itow: 123456000,
            pos_cov_valid: false,
            vel_cov_valid: false,
            pos_cov: [0.0; 6],
            vel_cov: [0.0; 6],
        }
    }

    /// Create a sample SecSigData with OK status.
    pub fn make_sec_sig_ok() -> SecSigData {
        SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Ok,
            spoofing_state: SpoofingStateData::Ok,
            jam_num_cent_freqs: 0,
            cent_freq_khz: vec![],
            jammed: vec![],
        }
    }

    /// Create a SecSigData indicating jamming.
    pub fn make_sec_sig_jamming() -> SecSigData {
        SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Warning,
            spoofing_state: SpoofingStateData::Ok,
            jam_num_cent_freqs: 1,
            cent_freq_khz: vec![1575420], // L1 frequency
            jammed: vec![true],
        }
    }

    /// Create a SecSigData indicating spoofing.
    pub fn make_sec_sig_spoofing() -> SecSigData {
        SecSigData {
            jam_det_enabled: true,
            spf_det_enabled: true,
            jamming_state: JammingStateData::Ok,
            spoofing_state: SpoofingStateData::Indicated,
            jam_num_cent_freqs: 0,
            cent_freq_khz: vec![],
            jammed: vec![],
        }
    }

    /// Create a sample MonHwData with OK status.
    pub fn make_mon_hw_ok() -> MonHwData {
        MonHwData {
            antenna_status: AntennaStatusData::Ok,
            antenna_power: AntennaPowerData::On,
            jam_ind: 20, // Low jamming indicator
            noise_per_ms: 100,
            agc_cnt: 5000,
            jamming_state: JammingStateData::Ok,
            rtc_calib: true,
            safe_boot: false,
        }
    }

    /// Create a MonHwData with antenna problem.
    pub fn make_mon_hw_antenna_short() -> MonHwData {
        MonHwData {
            antenna_status: AntennaStatusData::Short,
            antenna_power: AntennaPowerData::Off,
            jam_ind: 0,
            noise_per_ms: 0,
            agc_cnt: 0,
            jamming_state: JammingStateData::Unknown,
            rtc_calib: false,
            safe_boot: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::helpers::*;
    use super::ubx_packets::*;

    #[test]
    fn test_packet_checksums_are_valid() {
        // Verify all packet builders produce valid checksums
        let packets = vec![
            ("nav_pvt_rtk_float", nav_pvt_rtk_float()),
            ("nav_pvt_rtk_fixed", nav_pvt_rtk_fixed()),
            ("nav_pvt_no_fix", nav_pvt_no_fix()),
            ("nav_hpposllh", nav_hpposllh_valid()),
            ("nav_sat", nav_sat_4_sats()),
            ("nav_cov", nav_cov_valid()),
            ("ack_ack", ack_ack_cfg_valset()),
            ("ack_nak", ack_nak_cfg_valset()),
            ("sec_sig_ok", sec_sig_ok()),
            ("mon_hw_ok", mon_hw_ok()),
        ];

        for (name, packet) in packets {
            // Verify sync chars
            assert_eq!(packet[0], 0xB5, "{}: bad sync char 1", name);
            assert_eq!(packet[1], 0x62, "{}: bad sync char 2", name);

            // Verify checksum
            let len = packet.len();
            let (expected_a, expected_b) = ubx_checksum(&packet[2..len - 2]);
            assert_eq!(packet[len - 2], expected_a, "{}: checksum A mismatch", name);
            assert_eq!(packet[len - 1], expected_b, "{}: checksum B mismatch", name);
        }
    }

    #[test]
    fn test_bad_checksum_packet() {
        let packet = nav_pvt_bad_checksum();
        let len = packet.len();
        let (expected_a, expected_b) = ubx_checksum(&packet[2..len - 2]);

        // Checksum should NOT match
        assert!(
            packet[len - 2] != expected_a || packet[len - 1] != expected_b,
            "bad_checksum packet should have invalid checksum"
        );
    }

    #[test]
    fn test_helper_make_pvt() {
        let pvt = make_pvt();
        assert_eq!(pvt.fix_type, crate::state::FixType::Fix3D);
        assert_eq!(pvt.num_sv, 12);
        assert!(pvt.lat > 37.0 && pvt.lat < 38.0);
        assert!(pvt.lon < -122.0 && pvt.lon > -123.0);
    }

    #[test]
    fn test_helper_make_sat_info() {
        let sat_info = make_sat_info(8);
        assert_eq!(sat_info.num_sats, 8);
        assert_eq!(sat_info.sats.len(), 8);

        // Last satellite should not be used
        assert!(!sat_info.sats.last().unwrap().sv_used);
        // First should be used
        assert!(sat_info.sats.first().unwrap().sv_used);
    }

    #[test]
    fn test_helper_make_sec_sig_variants() {
        let ok = make_sec_sig_ok();
        assert_eq!(ok.jamming_state, crate::device::ubx::JammingStateData::Ok);

        let jamming = make_sec_sig_jamming();
        assert_eq!(
            jamming.jamming_state,
            crate::device::ubx::JammingStateData::Warning
        );

        let spoofing = make_sec_sig_spoofing();
        assert_eq!(
            spoofing.spoofing_state,
            crate::device::ubx::SpoofingStateData::Indicated
        );
    }

    #[test]
    fn test_multiple_packets_concatenation() {
        let data = multiple_packets();
        // Should contain 3 packets concatenated
        // Count sync chars
        let sync_count = data.windows(2).filter(|w| w == &[0xB5, 0x62]).count();
        assert_eq!(sync_count, 3, "should have 3 UBX packets");
    }
}
