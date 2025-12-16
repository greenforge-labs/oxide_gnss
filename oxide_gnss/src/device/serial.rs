//! Async serial port wrapper for GNSS device communication.
//!
//! Provides a tokio-compatible serial port with:
//! - Automatic reconnection with exponential backoff
//! - Read buffer management
//! - Write queue for RTCM injection

use std::io;
use std::time::Duration;

use serial2_tokio::SerialPort as RawSerialPort;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

use crate::config::ReconnectConfig;
use crate::error::DeviceError;

/// Default read buffer size (4KB should handle most UBX messages)
const DEFAULT_READ_BUFFER_SIZE: usize = 4096;

/// Default write queue capacity
const DEFAULT_WRITE_QUEUE_SIZE: usize = 32;

/// Builder for configuring and opening a serial port.
#[derive(Debug, Clone)]
pub struct SerialPortBuilder {
    port_path: String,
    baud_rate: u32,
    read_buffer_size: usize,
    write_queue_size: usize,
    reconnect: ReconnectConfig,
}

impl SerialPortBuilder {
    /// Create a new builder with the given port path.
    pub fn new(port_path: impl Into<String>) -> Self {
        Self {
            port_path: port_path.into(),
            baud_rate: 460800,
            read_buffer_size: DEFAULT_READ_BUFFER_SIZE,
            write_queue_size: DEFAULT_WRITE_QUEUE_SIZE,
            reconnect: ReconnectConfig::default(),
        }
    }

    /// Set the baud rate.
    pub fn baud_rate(mut self, rate: u32) -> Self {
        self.baud_rate = rate;
        self
    }

    /// Set the read buffer size.
    pub fn read_buffer_size(mut self, size: usize) -> Self {
        self.read_buffer_size = size;
        self
    }

    /// Set the write queue capacity.
    pub fn write_queue_size(mut self, size: usize) -> Self {
        self.write_queue_size = size;
        self
    }

    /// Set reconnection configuration.
    pub fn reconnect_config(mut self, config: ReconnectConfig) -> Self {
        self.reconnect = config;
        self
    }

    /// Get the port path.
    pub fn port_path(&self) -> &str {
        &self.port_path
    }

    /// Attempt to open the serial port.
    pub async fn open(self) -> Result<SerialPort, DeviceError> {
        SerialPort::open(self).await
    }
}

/// Async serial port wrapper with reconnection support.
pub struct SerialPort {
    /// The underlying serial port (None if disconnected)
    inner: Option<RawSerialPort>,
    /// Port configuration for reconnection
    config: SerialPortBuilder,
    /// Read buffer
    #[allow(dead_code)] // Will be used when read loop is implemented
    read_buffer: Vec<u8>,
    /// Write queue sender
    write_tx: mpsc::Sender<Vec<u8>>,
    /// Write queue receiver
    write_rx: mpsc::Receiver<Vec<u8>>,
    /// Current reconnection attempt
    reconnect_attempt: u32,
}

impl SerialPort {
    /// Open a serial port with the given configuration.
    async fn open(config: SerialPortBuilder) -> Result<Self, DeviceError> {
        let port = Self::try_open_port(&config)?;

        let (write_tx, write_rx) = mpsc::channel(config.write_queue_size);

        info!(
            port = %config.port_path,
            baud = config.baud_rate,
            "Serial port opened"
        );

        Ok(Self {
            inner: Some(port),
            read_buffer: vec![0u8; config.read_buffer_size],
            config,
            write_tx,
            write_rx,
            reconnect_attempt: 0,
        })
    }

    /// Try to open the raw serial port.
    fn try_open_port(config: &SerialPortBuilder) -> Result<RawSerialPort, DeviceError> {
        let port = RawSerialPort::open(&config.port_path, config.baud_rate).map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                DeviceError::PortNotFound {
                    port: config.port_path.clone(),
                    source: e,
                }
            } else {
                DeviceError::OpenFailed {
                    port: config.port_path.clone(),
                    source: e,
                }
            }
        })?;

        Ok(port)
    }

    /// Check if the port is currently connected.
    pub fn is_connected(&self) -> bool {
        self.inner.is_some()
    }

    /// Get the port path.
    pub fn port_path(&self) -> &str {
        &self.config.port_path
    }

    /// Get a sender for the write queue.
    ///
    /// Use this to inject RTCM data from the NTRIP client.
    pub fn write_sender(&self) -> mpsc::Sender<Vec<u8>> {
        self.write_tx.clone()
    }

    /// Read data from the serial port.
    ///
    /// Returns the number of bytes read, or an error if disconnected.
    /// On disconnect, will attempt reconnection if enabled.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, DeviceError> {
        let port = self
            .inner
            .as_mut()
            .ok_or_else(|| DeviceError::Disconnected {
                port: self.config.port_path.clone(),
            })?;

        match port.read(buf).await {
            Ok(n) => {
                self.reconnect_attempt = 0; // Reset on successful read
                Ok(n)
            }
            Err(e) => {
                error!(port = %self.config.port_path, error = %e, "Serial read error");
                self.handle_disconnect().await;
                Err(DeviceError::ReadError {
                    port: self.config.port_path.clone(),
                    source: e,
                })
            }
        }
    }

    /// Write data to the serial port.
    pub async fn write(&mut self, data: &[u8]) -> Result<usize, DeviceError> {
        let port = self
            .inner
            .as_mut()
            .ok_or_else(|| DeviceError::Disconnected {
                port: self.config.port_path.clone(),
            })?;

        match port.write(data).await {
            Ok(n) => Ok(n),
            Err(e) => {
                error!(port = %self.config.port_path, error = %e, "Serial write error");
                self.handle_disconnect().await;
                Err(DeviceError::WriteError {
                    port: self.config.port_path.clone(),
                    source: e,
                })
            }
        }
    }

    /// Write all data from the write queue.
    ///
    /// Call this periodically to flush queued RTCM data to the device.
    pub async fn flush_write_queue(&mut self) -> Result<usize, DeviceError> {
        let mut total_written = 0;

        while let Ok(data) = self.write_rx.try_recv() {
            let n = self.write(&data).await?;
            total_written += n;
            debug!(bytes = n, "Wrote queued data to serial port");
        }

        Ok(total_written)
    }

    /// Handle a disconnection event.
    async fn handle_disconnect(&mut self) {
        self.inner = None;

        if !self.config.reconnect.enabled {
            warn!(port = %self.config.port_path, "Serial port disconnected, reconnection disabled");
            return;
        }

        // Attempt reconnection if within limits
        if self.config.reconnect.max_attempts > 0
            && self.reconnect_attempt >= self.config.reconnect.max_attempts
        {
            error!(
                port = %self.config.port_path,
                attempts = self.reconnect_attempt,
                "Max reconnection attempts reached"
            );
            return;
        }

        self.reconnect_attempt += 1;
    }

    /// Attempt to reconnect to the serial port.
    ///
    /// Uses exponential backoff based on the reconnect configuration.
    /// Returns Ok(true) if reconnected, Ok(false) if should retry later,
    /// or Err if max attempts reached.
    pub async fn try_reconnect(&mut self) -> Result<bool, DeviceError> {
        if self.inner.is_some() {
            return Ok(true); // Already connected
        }

        if !self.config.reconnect.enabled {
            return Err(DeviceError::Disconnected {
                port: self.config.port_path.clone(),
            });
        }

        // Check max attempts
        if self.config.reconnect.max_attempts > 0
            && self.reconnect_attempt > self.config.reconnect.max_attempts
        {
            return Err(DeviceError::Disconnected {
                port: self.config.port_path.clone(),
            });
        }

        // Calculate backoff delay
        let delay_secs = crate::util::calculate_backoff(
            self.reconnect_attempt,
            self.config.reconnect.initial_delay_secs,
            self.config.reconnect.max_delay_secs,
        );

        info!(
            port = %self.config.port_path,
            attempt = self.reconnect_attempt,
            delay_secs = delay_secs,
            "Attempting reconnection"
        );

        sleep(Duration::from_secs(delay_secs as u64)).await;

        match Self::try_open_port(&self.config) {
            Ok(port) => {
                self.inner = Some(port);
                self.reconnect_attempt = 0;
                info!(port = %self.config.port_path, "Reconnected successfully");
                Ok(true)
            }
            Err(e) => {
                warn!(
                    port = %self.config.port_path,
                    error = %e,
                    "Reconnection attempt failed"
                );
                self.reconnect_attempt += 1;
                Ok(false)
            }
        }
    }

    /// Close the serial port.
    pub fn close(&mut self) {
        if self.inner.take().is_some() {
            info!(port = %self.config.port_path, "Serial port closed");
        }
    }
}

impl Drop for SerialPort {
    fn drop(&mut self) {
        self.close();
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_defaults() {
        let builder = SerialPortBuilder::new("/dev/ttyACM0");
        assert_eq!(builder.port_path, "/dev/ttyACM0");
        assert_eq!(builder.baud_rate, 460800);
    }

    #[test]
    fn test_builder_chain() {
        let builder = SerialPortBuilder::new("/dev/ttyUSB0")
            .baud_rate(115200)
            .read_buffer_size(8192);

        assert_eq!(builder.baud_rate, 115200);
        assert_eq!(builder.read_buffer_size, 8192);
    }

}
