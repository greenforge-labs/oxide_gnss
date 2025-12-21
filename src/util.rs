//! Shared utility functions for oxide_gnss.

/// Calculate exponential backoff delay.
///
/// Returns the delay in seconds for a given reconnection attempt using
/// exponential backoff: `initial * 2^(attempt-1)`, capped at `max_delay`.
///
/// # Arguments
/// * `attempt` - The current attempt number (0-indexed)
/// * `initial_delay` - Initial delay in seconds
/// * `max_delay` - Maximum delay in seconds
///
/// # Returns
/// The delay in seconds for this attempt
pub fn calculate_backoff(attempt: u32, initial_delay: u32, max_delay: u32) -> u32 {
    if attempt == 0 {
        return initial_delay;
    }

    // Exponential backoff: initial * 2^(attempt-1), capped at max
    let delay = initial_delay.saturating_mul(1 << (attempt - 1).min(10));
    delay.min(max_delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_calculation() {
        assert_eq!(calculate_backoff(0, 1, 60), 1);
        assert_eq!(calculate_backoff(1, 1, 60), 1);
        assert_eq!(calculate_backoff(2, 1, 60), 2);
        assert_eq!(calculate_backoff(3, 1, 60), 4);
        assert_eq!(calculate_backoff(4, 1, 60), 8);
        assert_eq!(calculate_backoff(10, 1, 60), 60); // Capped at max
    }

    #[test]
    fn test_backoff_with_larger_initial() {
        assert_eq!(calculate_backoff(1, 5, 120), 5);
        assert_eq!(calculate_backoff(2, 5, 120), 10);
        assert_eq!(calculate_backoff(3, 5, 120), 20);
    }
}
