use crate::mpinger::{MPingerConfigShared, MPingerMessage, MPingerRunner, MPingerType};
use crate::utils;
use anyhow::Result;
use log::{debug, error};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use time::OffsetDateTime;

pub struct MPingerHTTPKeepAlive {
    config: MPingerConfigShared,
    addrs: Vec<String>,
    tx: mpsc::Sender<MPingerMessage>,
}

struct PingData {
    idx: usize,
    host: String,
    stream: TcpStream,
}

impl MPingerHTTPKeepAlive {
    pub fn new(config: MPingerConfigShared, tx: mpsc::Sender<MPingerMessage>) -> Self {
        Self {
            config,
            tx,
            addrs: Vec::new(),
        }
    }

    fn perform_http_keepalive_ping(
        config: MPingerConfigShared,
        ping_data: &mut PingData,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let req = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", ping_data.host);

        let mut i = 0;
        loop {
            let start_time = Instant::now();

            let result = ping_data.stream.write_all(req.as_bytes());
            if result.is_err() {
                debug!("Error sending HTTP Request: {}", result.err().unwrap());
                let _ = tx.send(MPingerMessage {
                    idx: ping_data.idx,
                    runner_type: MPingerType::HTTPKeepAlive,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                });
                return;
            }

            // Read the response
            // TODO! read Content-Length and read only that much data
            // TODO! add support for chunked encoding
            const BUFFER_SIZE: usize = 4096;
            let mut buffer = [0; BUFFER_SIZE];
            let mut n = match ping_data.stream.read(&mut buffer) {
                Ok(n) => n,
                Err(e) => {
                    debug!("Error reading HTTP Response: {}", e);
                    let _ = tx.send(MPingerMessage {
                        idx: ping_data.idx,
                        runner_type: MPingerType::HTTPKeepAlive,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: 0,
                    });
                    return;
                }
            };

            let duration = Instant::now().duration_since(start_time).as_millis() as u32;

            while n >= BUFFER_SIZE {
                let mut buffer = [0; BUFFER_SIZE];
                n = ping_data.stream.read(&mut buffer).unwrap_or(0);
            }

            let result = tx.send(MPingerMessage {
                idx: ping_data.idx,
                runner_type: MPingerType::HTTPKeepAlive,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration,
            });
            if result.is_err() {
                debug!("Error sending message: {:?}", result);
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

            let timeout = std::time::Duration::from_millis(config.read().unwrap().timeout);
            let ad = SocketAddr::new(IpAddr::V4(ip), port);
            let stream = match TcpStream::connect_timeout(&ad, timeout) {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Error connecting: {}", e);
                    let _ = tx.send(MPingerMessage {
                        idx,
                        runner_type: MPingerType::HTTPKeepAlive,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: 0,
                    });
                    continue;
                }
            };
            stream
                .set_read_timeout(Some(std::time::Duration::from_millis(
                    config.read().unwrap().timeout,
                )))
                .unwrap();
            stream
                .set_write_timeout(Some(std::time::Duration::from_millis(
                    config.read().unwrap().timeout,
                )))
                .unwrap();

            ping_data.push(PingData {
                idx,
                host: addr.clone(),
                stream,
            });
        }

        // run Connect pings
        let handles: Vec<_> = ping_data
            .into_iter()
            .map(|mut pdata| {
                let tx = tx.clone();
                let config = config.clone();
                thread::spawn(move || {
                    MPingerHTTPKeepAlive::perform_http_keepalive_ping(
                        config, &mut pdata, tx, count,
                    );
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }
}

impl MPingerRunner for MPingerHTTPKeepAlive {
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
            MPingerHTTPKeepAlive::prepare(config, addrs, tx, count);
        });

        Ok(())
    }
}
