//! NTRIP HTTP client for connecting to casters and streaming corrections.
//!
//! Supports both NTRIP v1 (legacy ICY protocol) and NTRIP v2 (HTTP/1.1 with
//! chunked transfer encoding).
//!
//! Features:
//! - Raw TCP socket connection (plain or TLS-encrypted)
//! - NTRIP v1: HTTP/1.0, ICY 200 OK response
//! - NTRIP v2: HTTP/1.1, chunked transfer encoding
//! - Auto-detection of protocol version from server response
//! - Basic Authentication
//! - TLS/HTTPS support for secure connections
//! - GGA position reporting

use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tracing::{debug, error, info, warn};

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};

use crate::config::{NtripConfig, NtripVersion};
use crate::error::NtripError;

use super::gga::GgaSentence;
use super::stream::NtripStream;

/// Detected protocol version after connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DetectedProtocol {
    /// NTRIP v1 - raw binary stream
    V1,
    /// NTRIP v2 - chunked transfer encoding
    V2Chunked,
}

/// NTRIP client for streaming corrections from a caster.
/// Supports both v1 (ICY) and v2 (HTTP/1.1) protocols.
pub struct NtripClient {
    /// Configuration
    config: NtripConfig,
    /// Active stream (plain TCP or TLS, if connected)
    stream: Option<NtripStream>,
    /// Detected protocol after connection
    protocol: Option<DetectedProtocol>,
    /// Buffer for chunked decoding (holds partial chunk data)
    chunk_buffer: Vec<u8>,
    /// Remaining bytes in current chunk (for v2 chunked mode)
    chunk_remaining: usize,
}

impl NtripClient {
    /// Create a new NTRIP client with the given configuration.
    pub fn new(config: NtripConfig) -> Result<Self, NtripError> {
        Ok(Self {
            config,
            stream: None,
            protocol: None,
            chunk_buffer: Vec::new(),
            chunk_remaining: 0,
        })
    }

    /// Connect to the NTRIP caster and start streaming.
    ///
    /// For NTRIP v2, consider using `connect_with_gga()` to send an initial
    /// position in the request headers for faster VRS/nearest-base selection.
    pub async fn connect(&mut self) -> Result<(), NtripError> {
        self.connect_with_gga(None).await
    }

    /// Connect to the NTRIP caster with an optional initial GGA position.
    ///
    /// For NTRIP v2, the GGA sentence is sent in the `Ntrip-GGA` header,
    /// allowing the caster to select the appropriate VRS or nearest base
    /// immediately, without waiting for a post-connection GGA report.
    ///
    /// For NTRIP v1, the initial_gga is ignored (GGA must be sent post-connection).
    pub async fn connect_with_gga(
        &mut self,
        initial_gga: Option<&GgaSentence>,
    ) -> Result<(), NtripError> {
        let host = &self.config.host;
        let port = self.config.port;
        let addr = format!("{}:{}", host, port);

        info!(addr = %addr, "Connecting to NTRIP caster");

        // 1. Establish TCP connection
        let tcp_stream = match tokio::time::timeout(
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

        // 2. Optionally upgrade to TLS
        let mut stream: NtripStream = if self.config.use_https {
            if self.config.tls_skip_verify {
                warn!("TLS certificate verification is disabled - connection is not fully secure");
            }
            NtripStream::connect_tls(tcp_stream, host, self.config.tls_skip_verify).await?
        } else {
            NtripStream::plain(tcp_stream)
        };

        // 3. Construct HTTP Request based on configured version
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

        // Build Ntrip-GGA header for v2 if initial position provided
        let gga_header = match (&self.config.ntrip_version, initial_gga) {
            (NtripVersion::V2, Some(gga)) | (NtripVersion::Auto, Some(gga)) => {
                let nmea = gga.to_nmea();
                // Remove trailing newline for header format
                let nmea_trimmed = nmea.trim();
                debug!(gga = %nmea_trimmed, "Including initial GGA in request header");
                format!("Ntrip-GGA: {}\r\n", nmea_trimmed)
            }
            _ => String::new(),
        };

        // Build request based on configured NTRIP version
        let request = match self.config.ntrip_version {
            NtripVersion::V1 => {
                // NTRIP v1: HTTP/1.0 request (no Ntrip-GGA header support)
                debug!("Using NTRIP v1 protocol");
                format!(
                    "GET /{} HTTP/1.0\r\n\
                     User-Agent: {}\r\n\
                     Host: {}:{}\r\n\
                     Accept: */*\r\n\
                     Connection: close\r\n\
                     {}\r\n",
                    mountpoint, user_agent, host, port, auth_header
                )
            }
            NtripVersion::V2 | NtripVersion::Auto => {
                // NTRIP v2: HTTP/1.1 with Ntrip-Version header and optional Ntrip-GGA
                debug!("Using NTRIP v2 protocol request");
                format!(
                    "GET /{} HTTP/1.1\r\n\
                     Host: {}:{}\r\n\
                     User-Agent: {}\r\n\
                     Ntrip-Version: Ntrip/2.0\r\n\
                     {}\
                     Accept: */*\r\n\
                     Connection: close\r\n\
                     {}\r\n",
                    mountpoint, host, port, user_agent, gga_header, auth_header
                )
            }
        };

        // Log request with Authorization header redacted for security
        let redacted_request = if auth_header.is_empty() {
            request.clone()
        } else {
            request.replace(&auth_header, "Authorization: Basic [REDACTED]\r\n")
        };
        debug!(request = %redacted_request, "Sending NTRIP request");

        // 4. Send Request
        if let Err(e) = stream.write_all(request.as_bytes()).await {
            return Err(NtripError::NetworkError { source: e });
        }

        // 5. Read Response Headers
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

        // 6. Parse Status Line and detect protocol version
        let response = String::from_utf8_lossy(&headers);
        let status_line = response.lines().next().unwrap_or("");

        debug!(status_line = %status_line, "Received NTRIP response");

        if status_line.starts_with("ICY 200") || status_line.contains("200 OK") {
            // Detect protocol from response
            let detected = self.detect_protocol(&response, status_line);
            info!(
                mountpoint = %mountpoint,
                protocol = ?detected,
                "Connected to NTRIP caster"
            );

            self.stream = Some(stream);
            self.protocol = Some(detected);
            self.chunk_buffer.clear();
            self.chunk_remaining = 0;
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
    /// Automatically handles chunked transfer encoding for NTRIP v2.
    pub async fn read_chunk(&mut self, buf: &mut [u8]) -> Result<usize, NtripError> {
        if self.stream.is_none() {
            return Err(NtripError::StreamDisconnected {
                reason: "Not connected".to_string(),
            });
        }

        let read_timeout_secs = self.config.connection.read_timeout_secs;
        let timeout_duration = Duration::from_secs(read_timeout_secs as u64);

        // Read data with protocol-appropriate method (raw or chunked) with timeout
        let read_result =
            tokio::time::timeout(timeout_duration, self.read_raw_or_chunked(buf)).await;

        match read_result {
            Ok(Ok(n)) => {
                debug!(bytes = n, "Received data");
                Ok(n)
            }
            Ok(Err(e)) => Err(e),
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

    /// Fetch the sourcetable from the NTRIP caster.
    ///
    /// This is a one-shot operation that connects, retrieves the sourcetable,
    /// and disconnects. It does not affect the current streaming connection.
    ///
    /// # Example
    /// ```ignore
    /// let config = NtripConfig { host: "caster.example.com".into(), .. };
    /// let table = NtripClient::get_sourcetable(&config).await?;
    /// for stream in table.rtcm_streams() {
    ///     println!("{}: {}", stream.mountpoint, stream.format);
    /// }
    /// ```
    pub async fn get_sourcetable(
        config: &NtripConfig,
    ) -> Result<super::sourcetable::Sourcetable, NtripError> {
        let host = &config.host;
        let port = config.port;
        let addr = format!("{}:{}", host, port);

        info!(addr = %addr, "Fetching sourcetable from NTRIP caster");

        // 1. Establish TCP connection
        let tcp_stream = match tokio::time::timeout(
            Duration::from_secs(config.connection.timeout_secs as u64),
            TcpStream::connect(&addr),
        )
        .await
        {
            Ok(Ok(s)) => s,
            Ok(Err(e)) => return Err(NtripError::connection_failed(host, port, e)),
            Err(_) => {
                return Err(NtripError::Timeout {
                    timeout_secs: config.connection.timeout_secs,
                })
            }
        };

        // 2. Optionally upgrade to TLS
        let mut stream: NtripStream = if config.use_https {
            NtripStream::connect_tls(tcp_stream, host, config.tls_skip_verify).await?
        } else {
            NtripStream::plain(tcp_stream)
        };

        // 3. Send sourcetable request (GET / instead of GET /mountpoint)
        let user_agent = "NTRIP oxide_gnss/0.1";

        // Include authentication if credentials are provided (some casters require it)
        let auth_header = if let (Some(user), Some(pass)) = (&config.username, &config.password) {
            let credentials = format!("{}:{}", user, pass);
            let encoded = BASE64.encode(credentials);
            format!("Authorization: Basic {}\r\n", encoded)
        } else {
            String::new()
        };

        let request = format!(
            "GET / HTTP/1.0\r\n\
             User-Agent: {}\r\n\
             Host: {}:{}\r\n\
             Accept: */*\r\n\
             Connection: close\r\n\
             {}\r\n",
            user_agent, host, port, auth_header
        );

        debug!(request = %request, "Sending sourcetable request");

        if let Err(e) = stream.write_all(request.as_bytes()).await {
            return Err(NtripError::NetworkError { source: e });
        }

        // 4. Read entire response
        let mut response = Vec::new();
        let mut buf = [0u8; 4096];

        loop {
            match tokio::time::timeout(
                Duration::from_secs(config.connection.timeout_secs as u64),
                stream.read(&mut buf),
            )
            .await
            {
                Ok(Ok(0)) => break, // EOF
                Ok(Ok(n)) => response.extend_from_slice(&buf[..n]),
                Ok(Err(e)) => return Err(NtripError::NetworkError { source: e }),
                Err(_) => {
                    return Err(NtripError::Timeout {
                        timeout_secs: config.connection.timeout_secs,
                    })
                }
            }

            // Limit response size to prevent DoS
            if response.len() > 1_000_000 {
                return Err(NtripError::InvalidSourcetable {
                    message: "Sourcetable too large (>1MB)".to_string(),
                });
            }
        }

        // 5. Parse response
        let response_str = String::from_utf8_lossy(&response);

        // Check for success status
        let first_line = response_str.lines().next().unwrap_or("");
        if !first_line.contains("200") {
            return Err(NtripError::HttpError {
                status: 0,
                message: first_line.to_string(),
            });
        }

        // Find body (after \r\n\r\n)
        let body = if let Some(idx) = response_str.find("\r\n\r\n") {
            &response_str[idx + 4..]
        } else {
            &response_str[..]
        };

        let table = super::sourcetable::Sourcetable::parse(body);

        info!(
            streams = table.streams.len(),
            casters = table.casters.len(),
            networks = table.networks.len(),
            "Parsed sourcetable"
        );

        Ok(table)
    }

    /// Detect protocol version from server response headers.
    fn detect_protocol(&self, response: &str, status_line: &str) -> DetectedProtocol {
        // ICY responses are always v1
        if status_line.starts_with("ICY") {
            debug!("Detected NTRIP v1 (ICY response)");
            return DetectedProtocol::V1;
        }

        // Check for chunked transfer encoding (v2)
        let response_lower = response.to_lowercase();
        if response_lower.contains("transfer-encoding: chunked") {
            debug!("Detected NTRIP v2 (chunked transfer encoding)");
            return DetectedProtocol::V2Chunked;
        }

        // Check for Ntrip-Version header in response
        if response_lower.contains("ntrip-version:") {
            debug!("Detected NTRIP v2 (Ntrip-Version header present)");
            // Even without chunked, v2 servers may send raw - treat as v1 for data handling
            return DetectedProtocol::V1;
        }

        // Default to v1 (raw stream)
        debug!("Defaulting to NTRIP v1 protocol (raw stream)");
        DetectedProtocol::V1
    }

    /// Read a chunk of data, handling chunked transfer encoding for v2.
    async fn read_raw_or_chunked(&mut self, buf: &mut [u8]) -> Result<usize, NtripError> {
        let protocol = self.protocol.unwrap_or(DetectedProtocol::V1);

        match protocol {
            DetectedProtocol::V1 => self.read_raw(buf).await,
            DetectedProtocol::V2Chunked => self.read_chunked(buf).await,
        }
    }

    /// Read raw data from stream (v1 protocol).
    async fn read_raw(&mut self, buf: &mut [u8]) -> Result<usize, NtripError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| NtripError::StreamDisconnected {
                reason: "Not connected".to_string(),
            })?;

        match stream.read(buf).await {
            Ok(0) => {
                self.stream = None;
                Err(NtripError::StreamDisconnected {
                    reason: "Server closed connection".to_string(),
                })
            }
            Ok(n) => Ok(n),
            Err(e) => {
                self.stream = None;
                Err(NtripError::NetworkError { source: e })
            }
        }
    }

    /// Read chunked transfer encoded data (v2 protocol).
    async fn read_chunked(&mut self, buf: &mut [u8]) -> Result<usize, NtripError> {
        // If we have buffered data from a previous chunk, return that first
        if !self.chunk_buffer.is_empty() {
            let to_copy = std::cmp::min(buf.len(), self.chunk_buffer.len());
            buf[..to_copy].copy_from_slice(&self.chunk_buffer[..to_copy]);
            self.chunk_buffer.drain(..to_copy);
            return Ok(to_copy);
        }

        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| NtripError::StreamDisconnected {
                reason: "Not connected".to_string(),
            })?;

        // If we're in the middle of a chunk, read remaining data
        if self.chunk_remaining > 0 {
            let to_read = std::cmp::min(buf.len(), self.chunk_remaining);
            match stream.read(&mut buf[..to_read]).await {
                Ok(0) => {
                    self.stream = None;
                    return Err(NtripError::StreamDisconnected {
                        reason: "Server closed connection".to_string(),
                    });
                }
                Ok(n) => {
                    self.chunk_remaining -= n;
                    // If chunk is complete, consume trailing \r\n
                    if self.chunk_remaining == 0 {
                        let mut crlf = [0u8; 2];
                        let _ = stream.read_exact(&mut crlf).await;
                    }
                    return Ok(n);
                }
                Err(e) => {
                    self.stream = None;
                    return Err(NtripError::NetworkError { source: e });
                }
            }
        }

        // Read chunk size line
        let mut size_line = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            match stream.read_exact(&mut byte).await {
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    if byte[0] != b'\r' {
                        size_line.push(byte[0]);
                    }
                }
                Err(e) => {
                    self.stream = None;
                    return Err(NtripError::NetworkError { source: e });
                }
            }
            // Protect against malformed chunk headers
            if size_line.len() > 16 {
                return Err(NtripError::NetworkError {
                    source: std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Chunk size line too long",
                    ),
                });
            }
        }

        // Parse chunk size (hex)
        let size_str = String::from_utf8_lossy(&size_line);
        let chunk_size =
            usize::from_str_radix(size_str.trim(), 16).map_err(|_| NtripError::NetworkError {
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Invalid chunk size: {}", size_str),
                ),
            })?;

        // Chunk size 0 means end of stream
        if chunk_size == 0 {
            self.stream = None;
            return Err(NtripError::StreamDisconnected {
                reason: "Chunked stream ended".to_string(),
            });
        }

        debug!(chunk_size = chunk_size, "Reading chunked data");

        // Read chunk data
        self.chunk_remaining = chunk_size;
        let to_read = std::cmp::min(buf.len(), self.chunk_remaining);

        match stream.read(&mut buf[..to_read]).await {
            Ok(0) => {
                self.stream = None;
                Err(NtripError::StreamDisconnected {
                    reason: "Server closed connection".to_string(),
                })
            }
            Ok(n) => {
                self.chunk_remaining -= n;
                // If chunk is complete, consume trailing \r\n
                if self.chunk_remaining == 0 {
                    let mut crlf = [0u8; 2];
                    let _ = stream.read_exact(&mut crlf).await;
                }
                Ok(n)
            }
            Err(e) => {
                self.stream = None;
                Err(NtripError::NetworkError { source: e })
            }
        }
    }
}

impl Drop for NtripClient {
    fn drop(&mut self) {
        self.disconnect();
    }
}
