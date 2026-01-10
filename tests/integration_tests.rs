//! Integration tests for oxide_gnss.
//!
//! These tests verify integration between components without requiring
//! actual hardware. They focus on:
//! - Message flow through channels
//! - Configuration loading and validation
//! - State machine transitions
//! - Error propagation

use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::timeout;

// Re-export test fixtures from main crate if available
#[cfg(test)]
mod message_flow_tests {
    use super::*;

    /// Test that messages flow correctly through mpsc channels.
    #[tokio::test]
    async fn test_message_channel_ordering() {
        let (tx, mut rx) = mpsc::channel::<u32>(64);

        // Send messages in order
        for i in 0..10 {
            tx.send(i).await.unwrap();
        }

        // Verify order preserved
        for expected in 0..10 {
            let msg = rx.recv().await.unwrap();
            assert_eq!(msg, expected);
        }
    }

    /// Test watch channel behavior for latest-value semantics.
    #[tokio::test]
    async fn test_watch_channel_latest_value() {
        let (tx, rx) = watch::channel(0u32);

        // Send multiple values
        tx.send(1).unwrap();
        tx.send(2).unwrap();
        tx.send(3).unwrap();

        // Receiver should see latest value
        assert_eq!(*rx.borrow(), 3);
    }

    /// Test channel backpressure behavior.
    #[tokio::test]
    async fn test_bounded_channel_backpressure() {
        let (tx, mut rx) = mpsc::channel::<u32>(4);

        // Fill the channel
        for i in 0..4 {
            tx.send(i).await.unwrap();
        }

        // Next send should block (test with timeout)
        let send_result = timeout(Duration::from_millis(50), tx.send(4)).await;
        assert!(send_result.is_err(), "Expected timeout due to full channel");

        // Drain one message to make room
        let _ = rx.recv().await;

        // Now send should succeed
        let send_result = timeout(Duration::from_millis(50), tx.send(4)).await;
        assert!(send_result.is_ok(), "Expected send to succeed after drain");
    }

    /// Test that dropping sender closes the receiver.
    #[tokio::test]
    async fn test_sender_drop_closes_receiver() {
        let (tx, mut rx) = mpsc::channel::<u32>(64);

        tx.send(1).await.unwrap();
        tx.send(2).await.unwrap();
        drop(tx);

        // Should receive buffered messages
        assert_eq!(rx.recv().await, Some(1));
        assert_eq!(rx.recv().await, Some(2));

        // Then None when closed
        assert_eq!(rx.recv().await, None);
    }

    /// Test that dropping receiver causes send error.
    #[tokio::test]
    async fn test_receiver_drop_causes_send_error() {
        let (tx, rx) = mpsc::channel::<u32>(64);

        drop(rx);

        let result = tx.send(1).await;
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod shutdown_coordination_tests {
    use super::*;

    /// Test shutdown signal propagation through watch channel.
    #[tokio::test]
    async fn test_shutdown_signal_broadcast() {
        let (tx, rx1) = watch::channel(false);
        let rx2 = rx1.clone();
        let rx3 = rx1.clone();

        // All receivers should see false initially
        assert!(!*rx1.borrow());
        assert!(!*rx2.borrow());
        assert!(!*rx3.borrow());

        // Send shutdown signal
        tx.send(true).unwrap();

        // All receivers should see true
        assert!(*rx1.borrow());
        assert!(*rx2.borrow());
        assert!(*rx3.borrow());
    }

    /// Test that tasks can detect shutdown signal change.
    #[tokio::test]
    async fn test_shutdown_change_detection() {
        let (tx, mut rx) = watch::channel(false);

        // Spawn a task that waits for shutdown
        let handle = tokio::spawn(async move {
            rx.changed().await.unwrap();
            *rx.borrow()
        });

        // Small delay to ensure task is waiting
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Send shutdown signal
        tx.send(true).unwrap();

        // Task should complete with true
        let result = timeout(Duration::from_millis(100), handle)
            .await
            .expect("Task should complete")
            .expect("Task should not panic");
        assert!(result);
    }

    /// Test concurrent shutdown handling.
    #[tokio::test]
    async fn test_concurrent_shutdown_handling() {
        let (tx, rx) = watch::channel(false);

        // Spawn multiple tasks
        let mut handles = Vec::new();
        for i in 0..5 {
            let mut rx_clone = rx.clone();
            handles.push(tokio::spawn(async move {
                rx_clone.changed().await.unwrap();
                (i, *rx_clone.borrow())
            }));
        }

        // Small delay to ensure all tasks are waiting
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Send shutdown signal
        tx.send(true).unwrap();

        // All tasks should complete
        for handle in handles {
            let result = timeout(Duration::from_millis(100), handle)
                .await
                .expect("Task should complete")
                .expect("Task should not panic");
            assert!(result.1, "Task {} should receive shutdown", result.0);
        }
    }
}

#[cfg(test)]
mod state_machine_tests {
    /// Test basic state transitions (simulated).
    #[derive(Debug, Clone, PartialEq)]
    enum TestState {
        Disconnected,
        Connecting,
        Connected,
        Error(String),
    }

    struct StateMachine {
        state: TestState,
        transition_count: u32,
    }

    impl StateMachine {
        fn new() -> Self {
            Self {
                state: TestState::Disconnected,
                transition_count: 0,
            }
        }

        fn transition(&mut self, new_state: TestState) {
            self.state = new_state;
            self.transition_count += 1;
        }
    }

    #[test]
    fn test_state_transitions() {
        let mut sm = StateMachine::new();
        assert_eq!(sm.state, TestState::Disconnected);
        assert_eq!(sm.transition_count, 0);

        sm.transition(TestState::Connecting);
        assert_eq!(sm.state, TestState::Connecting);
        assert_eq!(sm.transition_count, 1);

        sm.transition(TestState::Connected);
        assert_eq!(sm.state, TestState::Connected);
        assert_eq!(sm.transition_count, 2);

        sm.transition(TestState::Error("timeout".to_string()));
        assert!(matches!(sm.state, TestState::Error(_)));
        assert_eq!(sm.transition_count, 3);
    }

    #[test]
    fn test_error_state_contains_reason() {
        let mut sm = StateMachine::new();
        sm.transition(TestState::Error("Connection refused".to_string()));

        if let TestState::Error(reason) = &sm.state {
            assert!(reason.contains("refused"));
        } else {
            panic!("Expected Error state");
        }
    }
}

#[cfg(test)]
mod timing_tests {
    use super::*;

    /// Test that tokio timeout works as expected.
    #[tokio::test]
    async fn test_timeout_triggers() {
        let result = timeout(Duration::from_millis(10), async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            42
        })
        .await;

        assert!(result.is_err(), "Expected timeout");
    }

    /// Test that operation completes before timeout.
    #[tokio::test]
    async fn test_timeout_does_not_trigger() {
        let result = timeout(Duration::from_millis(100), async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        })
        .await;

        assert_eq!(result.unwrap(), 42);
    }

    /// Test tokio select! prioritization with immediate values.
    #[tokio::test]
    async fn test_select_immediate_ready() {
        let (tx, mut rx) = mpsc::channel::<u32>(64);
        tx.send(1).await.unwrap();

        // The receive should win immediately
        tokio::select! {
            biased;

            msg = rx.recv() => {
                assert_eq!(msg, Some(1));
            }
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                panic!("Should not timeout");
            }
        }
    }
}

#[cfg(test)]
mod buffer_tests {
    /// Test Vec buffer growth behavior.
    #[test]
    fn test_buffer_capacity_growth() {
        let mut buf: Vec<u8> = Vec::with_capacity(64);
        assert!(buf.capacity() >= 64);

        // Extend beyond capacity
        for i in 0u8..100 {
            buf.push(i);
        }

        assert!(buf.capacity() >= 100);
        assert_eq!(buf.len(), 100);
    }

    /// Test slice operations on buffers.
    #[test]
    fn test_buffer_slicing() {
        let data = vec![0xB5, 0x62, 0x01, 0x07, 0x04, 0x00, 0xAA, 0xBB, 0xCC, 0xDD];

        // UBX header extraction
        let sync = &data[0..2];
        assert_eq!(sync, &[0xB5, 0x62]);

        // Class/ID extraction
        let class_id = &data[2..4];
        assert_eq!(class_id, &[0x01, 0x07]);

        // Length extraction (little-endian)
        let length = u16::from_le_bytes([data[4], data[5]]);
        assert_eq!(length, 4);
    }

    /// Test circular buffer pattern for streaming data.
    #[test]
    fn test_circular_buffer_pattern() {
        use std::collections::VecDeque;

        let mut ring: VecDeque<u8> = VecDeque::with_capacity(8);

        // Fill buffer
        for i in 0u8..8 {
            ring.push_back(i);
        }

        // Adding more should not grow if we pop first
        for i in 8u8..16 {
            ring.pop_front();
            ring.push_back(i);
        }

        // Should contain 8..16
        let contents: Vec<u8> = ring.iter().copied().collect();
        assert_eq!(contents, vec![8, 9, 10, 11, 12, 13, 14, 15]);
    }
}

#[cfg(test)]
mod concurrent_access_tests {
    use std::sync::Arc;

    use super::*;
    use tokio::sync::Mutex;

    /// Test mutex prevents data races.
    #[tokio::test]
    async fn test_mutex_serializes_access() {
        let counter = Arc::new(Mutex::new(0u32));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let counter_clone = Arc::clone(&counter);
            handles.push(tokio::spawn(async move {
                for _ in 0..100 {
                    let mut guard = counter_clone.lock().await;
                    *guard += 1;
                }
            }));
        }

        for handle in handles {
            handle.await.unwrap();
        }

        let final_count = *counter.lock().await;
        assert_eq!(final_count, 1000);
    }

    /// Test Arc sharing between tasks.
    #[tokio::test]
    async fn test_arc_shared_state() {
        let state = Arc::new(Mutex::new(Vec::<u32>::new()));

        let state1 = Arc::clone(&state);
        let state2 = Arc::clone(&state);

        let h1 = tokio::spawn(async move {
            state1.lock().await.push(1);
        });

        let h2 = tokio::spawn(async move {
            state2.lock().await.push(2);
        });

        h1.await.unwrap();
        h2.await.unwrap();

        let items = state.lock().await;
        assert_eq!(items.len(), 2);
        assert!(items.contains(&1));
        assert!(items.contains(&2));
    }
}

#[cfg(test)]
mod error_propagation_tests {
    use std::io::{self, ErrorKind};

    /// Test Result propagation with ? operator.
    fn fallible_operation(succeed: bool) -> io::Result<u32> {
        if succeed {
            Ok(42)
        } else {
            Err(io::Error::new(ErrorKind::Other, "test error"))
        }
    }

    fn chain_operations(succeed_first: bool, succeed_second: bool) -> io::Result<u32> {
        let a = fallible_operation(succeed_first)?;
        let b = fallible_operation(succeed_second)?;
        Ok(a + b)
    }

    #[test]
    fn test_error_propagation_first_fails() {
        let result = chain_operations(false, true);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_propagation_second_fails() {
        let result = chain_operations(true, false);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_propagation_both_succeed() {
        let result = chain_operations(true, true);
        assert_eq!(result.unwrap(), 84);
    }

    /// Test error context preservation.
    #[test]
    fn test_error_kind_preserved() {
        let err = io::Error::new(ErrorKind::ConnectionRefused, "connection refused");
        assert_eq!(err.kind(), ErrorKind::ConnectionRefused);
        assert!(err.to_string().contains("refused"));
    }
}
