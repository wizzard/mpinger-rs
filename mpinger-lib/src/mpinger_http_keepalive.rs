use crate::mpinger::{MPingDestination, MPingerConfigShared, MPingerMessage, MPingerType};
use log::{debug, error};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use time::OffsetDateTime;

pub struct MPingerHTTPKeepAlive();

impl MPingerHTTPKeepAlive {
    pub fn start(
        config: MPingerConfigShared,
        dest: &MPingDestination,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let req = format!("GET / HTTP/1.1\r\nHost: {}\r\n\r\n", dest.host);

        let timeout = std::time::Duration::from_millis(config.read().unwrap().timeout);

        let sock = match dest.sock_addr.as_socket() {
            Some(addr) => addr,
            None => {
                error!("Invalid socket address");
                let _ = tx.send(MPingerMessage {
                    destination_id: dest.id,
                    ping_nr: 0,
                    runner_type: MPingerType::HTTPKeepAlive,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                    is_error: true,
                });
                return;
            }
        };

        let mut stream = match TcpStream::connect_timeout(&sock, timeout) {
            Ok(stream) => stream,
            Err(e) => {
                error!("Error connecting: {}", e);
                let _ = tx.send(MPingerMessage {
                    destination_id: dest.id,
                    ping_nr: 0,
                    runner_type: MPingerType::HTTPKeepAlive,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                    is_error: true,
                });
                return;
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

        let mut i = 0;
        loop {
            let start_time = Instant::now();

            let result = stream.write_all(req.as_bytes());
            if result.is_err() {
                debug!("Error sending HTTP Request: {}", result.err().unwrap());
                let _ = tx.send(MPingerMessage {
                    destination_id: dest.id,
                    ping_nr: i,
                    runner_type: MPingerType::HTTPKeepAlive,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                    is_error: true,
                });
                return;
            }

            // Read the response
            // TODO! read Content-Length and read only that much data
            // TODO! add support for chunked encoding
            const BUFFER_SIZE: usize = 4096;
            let mut buffer = [0; BUFFER_SIZE];
            let mut n = match stream.read(&mut buffer) {
                Ok(n) => n,
                Err(e) => {
                    debug!("Error reading HTTP Response: {}", e);
                    let _ = tx.send(MPingerMessage {
                        destination_id: dest.id,
                        ping_nr: i,
                        runner_type: MPingerType::HTTPKeepAlive,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: 0,
                        is_error: true,
                    });
                    return;
                }
            };

            let duration = Instant::now().duration_since(start_time).as_millis() as u32;

            while n >= BUFFER_SIZE {
                let mut buffer = [0; BUFFER_SIZE];
                n = stream.read(&mut buffer).unwrap_or(0);
            }

            let result = tx.send(MPingerMessage {
                destination_id: dest.id,
                ping_nr: i,
                runner_type: MPingerType::HTTPKeepAlive,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration: duration as u64,
                is_error: false,
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
}
