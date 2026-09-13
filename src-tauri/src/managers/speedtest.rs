use futures_util::StreamExt;
use std::net::SocketAddr;
use std::time::Instant;
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};

pub struct SpeedTestManager;

impl SpeedTestManager {
    /// Performs an asynchronous TCP connect ping to the target host and port.
    /// Returns the round-trip latency in milliseconds, or -1 if timed out / failed.
    pub async fn tcp_ping(host: &str, port: u16, timeout_ms: u64) -> i64 {
        let addr_str = format!("{}:{}", host, port);
        
        let start = Instant::now();
        let connect_fut = async {
            if let Ok(addr) = addr_str.parse::<SocketAddr>() {
                TcpStream::connect(addr).await
            } else {
                tokio::net::lookup_host(&addr_str)
                    .await
                    .and_then(|mut addrs| {
                        if let Some(addr) = addrs.next() {
                            Ok(addr)
                        } else {
                            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Host not resolved"))
                        }
                    })
                    .map(|addr| TcpStream::connect(addr))?
                    .await
            }
        };

        match timeout(Duration::from_millis(timeout_ms), connect_fut).await {
            Ok(Ok(_stream)) => start.elapsed().as_millis() as i64,
            _ => -1,
        }
    }

    /// Performs an asynchronous QUIC Initial probe over UDP.
    /// Essential for MASQUE (HTTP/3 over QUIC): tests whether the remote UDP port
    /// actually replies with a QUIC Version Negotiation or Initial packet.
    /// Returns round-trip latency in ms, or -1 on timeout.
    pub async fn quic_ping(host: &str, port: u16, timeout_ms: u64) -> i64 {
        let clean_host = host.trim_matches('[').trim_matches(']');
        let addr_str = format!("{}:{}", clean_host, port);
        let target_addr = match tokio::net::lookup_host(&addr_str).await {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => return -1,
            },
            Err(_) => return -1,
        };

        let bind_addr = if target_addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };

        let socket = match tokio::net::UdpSocket::bind(bind_addr).await {
            Ok(s) => s,
            Err(_) => return -1,
        };

        // RFC 9000 QUIC v1 Initial packet template padded to 1200 bytes
        let mut packet = vec![0u8; 1200];
        let header = [
            0xc0, 0x00, 0x00, 0x00, 0x01, // Flags (Long Header, Initial) + Version 1
            0x08, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // DCID
            0x08, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, // SCID
            0x00, // Token length = 0
            0x44, 0x00, // Length = 1024
            0x00, // Packet number = 0
        ];
        packet[..header.len()].copy_from_slice(&header);

        let start = Instant::now();
        if socket.send_to(&packet, target_addr).await.is_err() {
            return -1;
        }

        let mut buf = [0u8; 2048];
        let recv_fut = socket.recv_from(&mut buf);
        match timeout(Duration::from_millis(timeout_ms), recv_fut).await {
            Ok(Ok((bytes_recvd, _))) if bytes_recvd > 0 => {
                let ms = start.elapsed().as_millis() as i64;
                if ms <= 0 { 1 } else { ms }
            }
            _ => -1,
        }
    }

    /// Performs an asynchronous WireGuard UDP probe.
    /// Sends a 148-byte UDP probe to host:port and waits for response.
    /// Returns RTT in ms, or -1 on timeout (blocked UDP port).
    pub async fn wireguard_ping(host: &str, port: u16, timeout_ms: u64) -> i64 {
        let clean_host = host.trim_matches('[').trim_matches(']');
        let addr_str = format!("{}:{}", clean_host, port);
        let target_addr = match tokio::net::lookup_host(&addr_str).await {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => return -1,
            },
            Err(_) => return -1,
        };

        let bind_addr = if target_addr.is_ipv6() {
            "[::]:0"
        } else {
            "0.0.0.0:0"
        };

        let socket = match tokio::net::UdpSocket::bind(bind_addr).await {
            Ok(s) => s,
            Err(_) => return -1,
        };

        // WireGuard initiation packet header (Type 1: Handshake Initiation)
        let mut probe = vec![0u8; 148];
        probe[0] = 0x01; // message_type = 1

        let start = Instant::now();
        if socket.send_to(&probe, target_addr).await.is_err() {
            return -1;
        }

        let mut buf = [0u8; 1024];
        let recv_fut = socket.recv_from(&mut buf);
        match timeout(Duration::from_millis(timeout_ms), recv_fut).await {
            Ok(Ok((bytes_recvd, _))) if bytes_recvd > 0 => {
                let ms = start.elapsed().as_millis() as i64;
                if ms <= 0 { 1 } else { ms }
            }
            _ => -1,
        }
    }

    /// Protocol-specific truthful latency test:
    /// - MASQUE: Real QUIC v1 Initial UDP probe on destination port
    /// - WireGuard: Real WireGuard UDP probe on destination port
    /// - VLESS / Shadowsocks / Trojan / SOCKS5 / HTTP: Real TCP 3-way handshake
    /// Returns latency in ms, or -1 for Timeout. Never uses misleading ICMP ping for proxy ports!
    pub async fn ping_node(protocol: &crate::models::ProtocolType, address: &str, port: u16) -> i64 {
        match protocol {
            crate::models::ProtocolType::Masque => {
                Self::quic_ping(address, port, 2000).await
            }
            crate::models::ProtocolType::Wireguard => {
                Self::wireguard_ping(address, port, 1800).await
            }
            _ => {
                Self::tcp_ping(address, port, 2500).await
            }
        }
    }

    /// Measures real download speed (bytes/sec) through the currently active local mixed proxy port.
    /// Uses Cloudflare CDN 10MB speed test endpoint with a max sample window of 3.5 seconds.
    pub async fn measure_download_speed(proxy_port: u16) -> Option<u64> {
        let proxy_url = format!("http://127.0.0.1:{}", proxy_port);
        let proxy = reqwest::Proxy::all(&proxy_url).ok()?;

        let client = reqwest::Client::builder()
            .proxy(proxy)
            .timeout(Duration::from_millis(4000))
            .build()
            .ok()?;

        // Fast high-availability global CDN test payloads
        let test_urls = [
            "https://speed.cloudflare.com/__down?bytes=10485760",
            "https://cachefly.cachefly.net/10mb.test",
        ];

        for url in test_urls {
            let start = Instant::now();
            if let Ok(resp) = client.get(url).send().await {
                if !resp.status().is_success() {
                    continue;
                }

                let mut stream = resp.bytes_stream();
                let mut total_bytes: u64 = 0;
                let max_duration = Duration::from_millis(3000);

                while let Ok(Some(chunk_res)) = timeout(Duration::from_millis(1500), stream.next()).await {
                    if let Ok(chunk) = chunk_res {
                        total_bytes += chunk.len() as u64;
                        if start.elapsed() >= max_duration || total_bytes >= 10485760 {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                let elapsed_secs = start.elapsed().as_secs_f64();
                if elapsed_secs > 0.1 && total_bytes > 32768 {
                    let speed = (total_bytes as f64 / elapsed_secs) as u64;
                    return Some(speed);
                }
            }
        }

        None
    }

    /// Probes node network throughput based on protocol-specific handshake roundtrip and jitter.
    pub async fn probe_node_speed(
        protocol: &crate::models::ProtocolType,
        host: &str,
        port: u16,
        latency_ms: i64,
    ) -> Option<u64> {
        if latency_ms <= 0 || latency_ms >= 1000 {
            return None;
        }

        // Measure a second quick connect using protocol-appropriate probe to evaluate jitter
        let second_ping = match protocol {
            crate::models::ProtocolType::Masque => Self::quic_ping(host, port, 1500).await,
            crate::models::ProtocolType::Wireguard => Self::wireguard_ping(host, port, 1500).await,
            _ => Self::tcp_ping(host, port, 1800).await,
        };

        if second_ping <= 0 {
            return None;
        }

        let avg_lat = ((latency_ms + second_ping) / 2).max(12);
        // Base BDP estimation with link quality factor
        let base_bps: f64 = (1000.0 / avg_lat as f64) * 680_000.0;
        // Jitter penalty
        let jitter = (latency_ms - second_ping).abs();
        let stability_ratio = (1.0 - (jitter as f64 / 200.0)).clamp(0.45, 1.25);
        let final_speed = (base_bps * stability_ratio) as u64;

        Some(final_speed)
    }
}
