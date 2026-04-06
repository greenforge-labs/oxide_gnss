//! Coordinate frame transformations for GNSS data.
//!
//! GNSS receivers typically output velocities in NED (North-East-Down) frame,
//! while ROS conventionally uses ENU (East-North-Up) frame.
//!
//! This module provides conversions between these frames.

use crate::config::CoordinateFrame;

/// A 3D velocity vector.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Velocity3D {
    /// X component (East in ENU, North in NED).
    pub x: f64,
    /// Y component (North in ENU, East in NED).
    pub y: f64,
    /// Z component (Up in ENU, Down in NED).
    pub z: f64,
}

impl Velocity3D {
    /// Create a new velocity vector.
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    /// Create velocity from NED components (North, East, Down).
    pub fn from_ned(north: f64, east: f64, down: f64) -> Self {
        Self {
            x: north,
            y: east,
            z: down,
        }
    }

    /// Create velocity from ENU components (East, North, Up).
    pub fn from_enu(east: f64, north: f64, up: f64) -> Self {
        Self {
            x: east,
            y: north,
            z: up,
        }
    }
}

/// Convert NED velocity to ENU velocity.
///
/// NED (North, East, Down) -> ENU (East, North, Up)
/// - NED.North -> ENU.North (y)
/// - NED.East  -> ENU.East (x)  
/// - NED.Down  -> ENU.Up (z, negated)
///
/// # Example
/// ```
/// use oxide_gnss::transform::ned_to_enu;
///
/// let (east, north, up) = ned_to_enu(1.0, 2.0, -3.0);
/// assert_eq!(east, 2.0);   // NED.East -> ENU.East
/// assert_eq!(north, 1.0);  // NED.North -> ENU.North
/// assert_eq!(up, 3.0);     // -NED.Down -> ENU.Up
/// ```
pub fn ned_to_enu(north: f64, east: f64, down: f64) -> (f64, f64, f64) {
    (east, north, -down)
}

/// Convert ENU velocity to NED velocity.
///
/// ENU (East, North, Up) -> NED (North, East, Down)
/// - ENU.East  -> NED.East (y)
/// - ENU.North -> NED.North (x)
/// - ENU.Up    -> NED.Down (z, negated)
///
/// # Example
/// ```
/// use oxide_gnss::transform::enu_to_ned;
///
/// let (north, east, down) = enu_to_ned(2.0, 1.0, 3.0);
/// assert_eq!(north, 1.0);  // ENU.North -> NED.North
/// assert_eq!(east, 2.0);   // ENU.East -> NED.East
/// assert_eq!(down, -3.0);  // -ENU.Up -> NED.Down
/// ```
pub fn enu_to_ned(east: f64, north: f64, up: f64) -> (f64, f64, f64) {
    (north, east, -up)
}

/// Transform NED velocity to the target coordinate frame.
///
/// If target is ENU, converts NED -> ENU.
/// If target is NED, returns the values unchanged.
pub fn ned_to_frame(north: f64, east: f64, down: f64, target: CoordinateFrame) -> (f64, f64, f64) {
    match target {
        CoordinateFrame::ENU => ned_to_enu(north, east, down),
        CoordinateFrame::NED => (north, east, down),
    }
}

/// Transform velocity from one frame to another.
///
/// This is the general-purpose transform function.
pub fn transform_velocity(
    vel: Velocity3D,
    from: CoordinateFrame,
    to: CoordinateFrame,
) -> Velocity3D {
    if from == to {
        return vel;
    }

    match (from, to) {
        (CoordinateFrame::NED, CoordinateFrame::ENU) => {
            let (x, y, z) = ned_to_enu(vel.x, vel.y, vel.z);
            Velocity3D::new(x, y, z)
        }
        (CoordinateFrame::ENU, CoordinateFrame::NED) => {
            let (x, y, z) = enu_to_ned(vel.x, vel.y, vel.z);
            Velocity3D::new(x, y, z)
        }
        _ => vel, // Same frame, already handled above
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ned_to_enu() {
        // Vehicle moving North at 1 m/s, East at 2 m/s, descending at 0.5 m/s
        let (east, north, up) = ned_to_enu(1.0, 2.0, 0.5);

        assert_eq!(east, 2.0); // East velocity unchanged
        assert_eq!(north, 1.0); // North velocity unchanged
        assert_eq!(up, -0.5); // Down becomes negative Up
    }

    #[test]
    fn test_enu_to_ned() {
        // Vehicle moving East at 2 m/s, North at 1 m/s, ascending at 0.5 m/s
        let (north, east, down) = enu_to_ned(2.0, 1.0, 0.5);

        assert_eq!(north, 1.0); // North velocity unchanged
        assert_eq!(east, 2.0); // East velocity unchanged
        assert_eq!(down, -0.5); // Up becomes negative Down
    }

    #[test]
    fn test_round_trip_ned_enu_ned() {
        let original = (1.5, -2.3, 0.7);
        let (e, n, u) = ned_to_enu(original.0, original.1, original.2);
        let (n2, e2, d2) = enu_to_ned(e, n, u);

        assert!((n2 - original.0).abs() < 1e-10);
        assert!((e2 - original.1).abs() < 1e-10);
        assert!((d2 - original.2).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip_enu_ned_enu() {
        let original = (1.5, -2.3, 0.7);
        let (n, e, d) = enu_to_ned(original.0, original.1, original.2);
        let (e2, n2, u2) = ned_to_enu(n, e, d);

        assert!((e2 - original.0).abs() < 1e-10);
        assert!((n2 - original.1).abs() < 1e-10);
        assert!((u2 - original.2).abs() < 1e-10);
    }

    #[test]
    fn test_ned_to_frame_enu() {
        let (x, y, z) = ned_to_frame(1.0, 2.0, 0.5, CoordinateFrame::ENU);
        assert_eq!(x, 2.0); // East
        assert_eq!(y, 1.0); // North
        assert_eq!(z, -0.5); // Up
    }

    #[test]
    fn test_ned_to_frame_ned() {
        // When target is NED, values pass through unchanged
        let (x, y, z) = ned_to_frame(1.0, 2.0, 0.5, CoordinateFrame::NED);
        assert_eq!(x, 1.0); // North
        assert_eq!(y, 2.0); // East
        assert_eq!(z, 0.5); // Down
    }

    #[test]
    fn test_transform_velocity_same_frame() {
        let vel = Velocity3D::new(1.0, 2.0, 3.0);
        let result = transform_velocity(vel, CoordinateFrame::ENU, CoordinateFrame::ENU);
        assert_eq!(result, vel);
    }

    #[test]
    fn test_transform_velocity_ned_to_enu() {
        let vel = Velocity3D::from_ned(1.0, 2.0, 0.5); // N=1, E=2, D=0.5
        let result = transform_velocity(vel, CoordinateFrame::NED, CoordinateFrame::ENU);

        assert_eq!(result.x, 2.0); // East
        assert_eq!(result.y, 1.0); // North
        assert_eq!(result.z, -0.5); // Up
    }

    #[test]
    fn test_zero_velocity() {
        let (e, n, u) = ned_to_enu(0.0, 0.0, 0.0);
        assert_eq!(e, 0.0);
        assert_eq!(n, 0.0);
        assert_eq!(u, 0.0);
    }

    #[test]
    fn test_negative_velocities() {
        // Moving South (-North), West (-East), ascending (-Down)
        let (e, n, u) = ned_to_enu(-1.0, -2.0, -0.5);
        assert_eq!(e, -2.0); // West
        assert_eq!(n, -1.0); // South
        assert_eq!(u, 0.5); // Up (ascending)
    }
}
