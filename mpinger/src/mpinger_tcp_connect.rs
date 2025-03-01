use crate::mpinger::{MPingerConfigShared, MPingerMessage, MPingerRunner, MPingerType};
use crate::utils;
use anyhow::Result;
use log::{debug, error};
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::IpAddr;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use time::OffsetDateTime;

pub struct MPingerTCPConnect {
    config: MPingerConfigShared,
    addrs: Vec<String>,
    tx: mpsc::Sender<MPingerMessage>,
}

struct PingData {
    dest_addr: SockAddr,
    idx: usize,
}

impl MPingerTCPConnect {
    pub fn new(config: MPingerConfigShared, tx: mpsc::Sender<MPingerMessage>) -> Self {
        Self {
            config,
            tx,
            addrs: Vec::new(),
        }
    }

    fn perfrom_connect_ping(
        config: MPingerConfigShared,
        ping_data: &PingData,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let timeout = std::time::Duration::from_millis(config.read().unwrap().timeout);

        let mut i = 0;
        loop {
            let socket = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
                Ok(socket) => socket,
                Err(e) => {
                    error!("Error creating socket: {:?}", e);
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

            let start_time = Instant::now();
            let result = socket.connect_timeout(&ping_data.dest_addr, timeout);
            let duration = start_time.elapsed().as_millis() as u32;

            let duration = match result {
                Ok(_) => duration,
                Err(e) => {
                    debug!("Error connecting: {}", e);
                    0
                }
            };

            let result = tx.send(MPingerMessage {
                idx: ping_data.idx,
                runner_type: MPingerType::TCPConnect,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration,
            });
            if result.is_err() {
                debug!("Error sending message: {:?}", result);
            }

            let result = socket.shutdown(std::net::Shutdown::Both);
            if result.is_err() {
                debug!("Error shutting down socket: {:?}", result);
            }

            i += 1;
            if count > 0 && i >= count {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(
                config.read().unwrap().ping_interval,
            ));
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
            let (ip, port) =
                match utils::parse_host_port(addr, Some(config.read().unwrap().default_port)) {
                    Ok((ip, port)) => (ip, port),
                    Err(_) => {
                        error!("Error parsing address: {}", addr);
                        continue;
                    }
                };

            let dest_addr = SockAddr::from(std::net::SocketAddr::new(IpAddr::V4(ip), port));

            ping_data.push(PingData { dest_addr, idx });
        }

        // run Connect pings
        let handles: Vec<_> = ping_data
            .into_iter()
            .map(|pdata| {
                let tx = tx.clone();
                let config = config.clone();
                thread::spawn(move || {
                    MPingerTCPConnect::perfrom_connect_ping(config, &pdata, tx, count);
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

impl MPingerRunner for MPingerTCPConnect {
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
            MPingerTCPConnect::prepare(config, addrs, tx, count);
        });

        Ok(())
    }
}
