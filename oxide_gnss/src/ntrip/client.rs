//! NTRIP HTTP client for connecting to casters and streaming corrections.
//!
//! Implements NTRIP v1 protocol over raw TCP sockets to support legacy
//! "ICY 200 OK" responses which are rejected by standard HTTP libraries.
//!
//! Features:
//! - Raw TCP socket connection
//! - Manual HTTP/1.0 request construction
//! - Custom response parsing accepting ICY and HTTP status codes
//! - Basic Authentication
//! - Chunked transfer decoding (basic implementation)
//! - GGA position reporting

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tracing::{debug, error, info};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::config::NtripConfig;
use crate::error::NtripError;

use super::gga::GgaSentence;

/// NTRIP v1 client for streaming corrections from a caster.
pub struct NtripClient {
    /// Configuration
    config: NtripConfig,
    /// Active TCP stream (if connected)
    stream: Option<TcpStream>,
}

impl NtripClient {
    /// Create a new NTRIP client with the given configuration.
    pub fn new(config: NtripConfig) -> Result<Self, NtripError> {
        Ok(Self {
            config,
            stream: None,
        })
    }

    /// Connect to the NTRIP caster and start streaming.
    pub async fn connect(&mut self) -> Result<(), NtripError> {
        let host = &self.config.host;
        let port = self.config.port;
        let addr = format!("{}:{}", host, port);

        info!(addr = %addr, "Connecting to NTRIP caster");

        // 1. Establish TCP connection
        let stream = match tokio::time::timeout(
            Duration::from_secs(self.config.connection.timeout_secs as u64),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(NtripError::connection_failed(host, port, e)),
            Err(_) => {
                return Err(NtripError::Timeout {
                    timeout_secs: self.config.connection.timeout_secs,
                })
            }
        };

        let mut stream = stream;

        // 2. Construct HTTP Request
        let mountpoint = &self.config.mountpoint;
        let user_agent = "NTRIP oxide_gnss/0.1";

        let auth_header =
            if let (Some(user), Some(pass)) = (&self.config.username, &self.config.password) {
                let credentials = format!("{}:{}", user, pass);
                let encoded = BASE64.encode(credentials);
                format!("Authorization: Basic {}\r\n", encoded)
            } else {
                String::new()
            };

        // HTTP/1.0 request - connection stays open for NTRIP streaming
        let request = format!(
            "GET /{} HTTP/1.0\r\n\
             User-Agent: {}\r\n\
             Host: {}:{}\r\n\
             Accept: */*\r\n\
             Connection: close\r\n\
             {}\
             \r\n",
            mountpoint, user_agent, host, port, auth_header
        );

        // Log request with Authorization header redacted for security
        let redacted_request = if auth_header.is_empty() {
            request.clone()
        } else {
            request.replace(&auth_header, "Authorization: Basic [REDACTED]\r\n")
        };
        debug!(request = %redacted_request, "Sending NTRIP request");

        // 3. Send Request
        if let Err(e) = stream.write_all(request.as_bytes()).await {
            return Err(NtripError::NetworkError { source: e });
        }

        // 4. Read Response Headers
        // We read byte-by-byte until we find \r\n\r\n
        let mut headers = Vec::new();
        let mut buffer = [0u8; 1];
        let mut header_end_found = false;

        // Limit header size to avoid DoS
        const MAX_HEADER_SIZE: usize = 4096;

        while headers.len() < MAX_HEADER_SIZE {
            match stream.read_exact(&mut buffer).await {
                Ok(_) => {
                    headers.push(buffer[0]);

                    // Check for standard HTTP header end (\r\n\r\n)
                    if headers.len() >= 4 && &headers[headers.len() - 4..] == b"\r\n\r\n" {
                        header_end_found = true;
                        break;
                    }

                    // Check for ICY 200 OK response (often followed by single \r\n then data)
                    // "ICY 200 OK\r\n" is 12 bytes
                    if headers.len() >= 12 && headers.starts_with(b"ICY 200 OK\r\n") {
                        header_end_found = true;
                        debug!("Detected legacy ICY 200 OK response, starting stream");
                        break;
                    }
                }
                Err(e) => return Err(NtripError::NetworkError { source: e }),
            }
        }

        if !header_end_found {
            return Err(NtripError::NetworkError {
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Header too large or incomplete",
                ),
            });
        }

        // 5. Parse Status Line
        let response = String::from_utf8_lossy(&headers);
        let status_line = response.lines().next().unwrap_or("");

        debug!(status_line = %status_line, "Received NTRIP response");

        if status_line.starts_with("ICY 200") || status_line.contains("200 OK") {
            info!(mountpoint = %mountpoint, "Connected to NTRIP caster");
            self.stream = Some(stream);
            Ok(())
        } else if status_line.contains("401 Unauthorized") {
            error!("NTRIP authentication failed");
            Err(NtripError::AuthenticationFailed {
                host: host.clone(),
                username: self.config.username.clone().unwrap_or_default(),
            })
        } else if status_line.contains("404 Not Found") {
            error!(mountpoint = %mountpoint, "Mountpoint not found");
            Err(NtripError::MountpointNotFound {
                host: host.clone(),
                mountpoint: mountpoint.clone(),
            })
        } else {
            error!(status = %status_line, "NTRIP caster error");
            Err(NtripError::HttpError {
                status: 0, // Unknown status code extraction
                message: status_line.to_string(),
            })
        }
    }

    /// Read a chunk of RTCM data from the stream with timeout.
    pub async fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, NtripError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| NtripError::StreamDisconnected {
                reason: "Not connected".to_string(),
            })?;

        let read_timeout_secs = self.config.connection.read_timeout_secs;
        let timeout_duration = Duration::from_secs(read_timeout_secs as u64);

        // Read raw RTCM binary data (NTRIP 1.0 / ICY protocol) with timeout
        let read_result = tokio::time::timeout(timeout_duration, stream.read(buf)).await;

        match read_result {
            Ok(Ok(0)) => {
                // EOF
                self.stream = None;
                Err(NtripError::StreamDisconnected {
                    reason: "Server closed connection".to_string(),
                })
            }
            Ok(Ok(n)) => {
                debug!(bytes = n, "Received data");
                Ok(n)
            }
            Ok(Err(e)) => {
                self.stream = None;
                Err(NtripError::NetworkError { source: e })
            }
            Err(_) => {
                // Timeout elapsed
                self.stream = None;
                Err(NtripError::ReadTimeout {
                    timeout_secs: read_timeout_secs,
                })
            }
        }
    }

    /// Check if connected.
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// Disconnect from the caster.
    pub fn disconnect(&mut self) {
        if self.stream.take().is_some() {
            info!("Disconnected from NTRIP caster");
        }
    }

    /// Send a GGA position report to the caster on the existing stream.
    ///
    /// NTRIP v1 expects GGA sentences sent on the same socket during streaming.
    /// This provides the caster with rover position for VRS/nearest-base selection.
    pub async fn send_gga(&mut self, gga: &GgaSentence) -> Result<(), NtripError> {
        if !self.config.send_gga {
            return Ok(());
        }

        let stream = self.stream.as_mut().ok_or(NtripError::StreamDisconnected {
            reason: "Not connected".into(),
        })?;

        let nmea = gga.to_nmea();
        debug!(gga = %nmea.trim(), "Sending GGA to caster");

        stream
            .write_all(nmea.as_bytes())
            .await
            .map_err(|e| NtripError::GgaSendFailed { source: e })?;

        Ok(())
    }

    /// Get the configuration.
    pub fn config(&self) -> &NtripConfig {
        &self.config
    }
}

impl Drop for NtripClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}
