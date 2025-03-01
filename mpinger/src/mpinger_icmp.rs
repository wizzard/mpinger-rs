use crate::mpinger::{MPingerConfigShared, MPingerMessage, MPingerRunner, MPingerType};
use crate::utils;
use anyhow::Result;
use log::{debug, error};
use rand::random;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::mem::MaybeUninit;
use std::net::IpAddr;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

const ICMP_SIZE: usize = 64;

pub struct MPingerICMP {
    config: MPingerConfigShared,
    addrs: Vec<String>,
    tx: mpsc::Sender<MPingerMessage>,
}

struct PingData {
    dest_addr: SockAddr,
    socket: Socket,
    idx: usize,
}

impl MPingerICMP {
    pub fn new(config: MPingerConfigShared, tx: mpsc::Sender<MPingerMessage>) -> Self {
        Self {
            config,
            tx,
            addrs: Vec::new(),
        }
    }

    fn perfrom_icmp_ping(
        config: MPingerConfigShared,
        ping_data: &PingData,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let mut i = 0;
        loop {
            // Build ICMP Echo Request
            let identifier = random::<u16>();
            let sequence: u16 = i as u16;
            let packet = MPingerICMP::build_icmp_echo_request(identifier, sequence, b"");

            let start_time = Instant::now();
            // send Echo Request
            match ping_data.socket.send_to(&packet, &ping_data.dest_addr) {
                Ok(_) => {}
                Err(e) => {
                    debug!("Error sending ICMP packet: {}", e);
                    let _ = tx.send(MPingerMessage {
                        idx: ping_data.idx,
                        runner_type: MPingerType::ICMPPing,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: 0,
                    });
                    break;
                }
            }

            let mut buffer = [MaybeUninit::uninit(); ICMP_SIZE];
            let mut found = false;
            for _ in 0..config.read().unwrap().ping_retries {
                // receive Echo Reply
                let (recv_size, _) = match ping_data.socket.recv_from(&mut buffer) {
                    Ok((size, _)) => (size, true),
                    Err(e) => {
                        debug!("Error receiving ICMP packet: {}", e);
                        break;
                    }
                };
                let buffer: [u8; ICMP_SIZE] = unsafe { std::mem::transmute(buffer) };

                if MPingerICMP::is_valid_icmp_echo_response(
                    &buffer, recv_size, identifier, sequence,
                ) {
                    let rtt = Instant::now().duration_since(start_time);
                    let _ = tx.send(MPingerMessage {
                        idx: ping_data.idx,
                        runner_type: MPingerType::ICMPPing,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: rtt.as_millis() as u32,
                    });
                    found = true;
                    break;
                }
            }

            if !found {
                debug!("No ICMP Echo Reply received for {:?}", ping_data.dest_addr);
                let _ = tx.send(MPingerMessage {
                    idx: ping_data.idx,
                    runner_type: MPingerType::ICMPPing,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                });
            }

            i += 1;
            if count > 0 && i >= count {
                break;
            }
            std::thread::sleep(Duration::from_millis(config.read().unwrap().ping_interval));
        }
    }

    fn prepare(
        config: MPingerConfigShared,
        addrs: Vec<String>,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        // prepare PingData list
        let mut ping_data: Vec<PingData> = Vec::new();
        for (idx, addr) in addrs.iter().enumerate() {
            let ip = match utils::parse_host_port(addr, Some(0)) {
                Ok(ip) => ip,
                Err(_) => {
                    error!("Error parsing address: {}", addr);
                    continue;
                }
            };

            let socket = match Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4)) {
                Ok(socket) => socket,
                Err(e) => {
                    error!("Error creating ICMP socket: {}", e);
                    continue;
                }
            };

            socket
                .set_read_timeout(Some(std::time::Duration::from_millis(
                    config.read().unwrap().timeout,
                )))
                .unwrap();
            socket
                .set_write_timeout(Some(std::time::Duration::from_millis(
                    config.read().unwrap().timeout,
                )))
                .unwrap();

            let dest_addr = SockAddr::from(std::net::SocketAddr::new(IpAddr::V4(ip.0), 0));

            ping_data.push(PingData {
                dest_addr,
                socket,
                idx,
            });
        }

        // run ICMP pings
        let handles: Vec<_> = ping_data
            .into_iter()
            .map(|pdata| {
                let tx = tx.clone();
                let config = config.clone();
                thread::spawn(move || {
                    MPingerICMP::perfrom_icmp_ping(config, &pdata, tx, count);
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    // Calculate the ICMP checksum (16-bit one's complement sum)
    fn calculate_checksum(data: &[u8]) -> u16 {
        let mut sum = 0u32;
        let mut i = 0;

        while i < data.len() - 1 {
            let word = ((data[i] as u32) << 8) | (data[i + 1] as u32);
            sum += word;
            i += 2;
        }

        if i < data.len() {
            sum += (data[i] as u32) << 8;
        }

        while sum >> 16 != 0 {
            sum = (sum & 0xFFFF) + (sum >> 16);
        }

        !sum as u16
    }

    fn build_icmp_echo_request(identifier: u16, sequence: u16, payload: &[u8]) -> Vec<u8> {
        let mut packet = Vec::with_capacity(8 + payload.len());

        // ICMP Header
        packet.push(8); // Type: Echo Request
        packet.push(0); // Code: 0
        packet.push(0); // Checksum placeholder (high byte)
        packet.push(0); // Checksum placeholder (low byte)
        packet.extend_from_slice(&identifier.to_be_bytes());
        packet.extend_from_slice(&sequence.to_be_bytes());
        packet.extend_from_slice(payload); // Payload

        // Calculate checksum over the entire packet
        let checksum = MPingerICMP::calculate_checksum(&packet);
        packet[2] = (checksum >> 8) as u8; // High byte
        packet[3] = checksum as u8; // Low byte

        packet
    }

    fn is_valid_icmp_echo_response(
        buffer: &[u8; ICMP_SIZE],
        recv_size: usize,
        identifier: u16,
        sequence: u16,
    ) -> bool {
        let ip_header_len = (&buffer[0] & 0x0F) as usize * 4;
        let icmp_start = ip_header_len;

        if recv_size > icmp_start + 7 {
            let icmp_type = buffer[icmp_start];
            let icmp_code = buffer[icmp_start + 1];
            let recv_identifier =
                u16::from_be_bytes([buffer[icmp_start + 4], buffer[icmp_start + 5]]);
            let recv_sequence =
                u16::from_be_bytes([buffer[icmp_start + 6], buffer[icmp_start + 7]]);

            if icmp_type == 0
                && icmp_code == 0
                && recv_identifier == identifier
                && recv_sequence == sequence
            {
                return true;
            }
        }

        false
    }
}

impl MPingerRunner for MPingerICMP {
    fn add(&mut self, addr: &str) {
        self.addrs.push(addr.to_string());
    }

    fn get_addr_by_idx(&self, idx: usize) -> Option<&String> {
        self.addrs.get(idx)
    }

    fn start(&self, count: usize) -> Result<()> {
        if self.addrs.is_empty() {
            return Ok(());
        }
        let addrs = self.addrs.clone();
        let tx = self.tx.clone();
        let config = self.config.clone();

        thread::spawn(move || {
            MPingerICMP::prepare(config, addrs, tx, count);
        });

        Ok(())
    }
}
