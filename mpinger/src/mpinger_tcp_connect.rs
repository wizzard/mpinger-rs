use crate::mpinger::{MPingDestination, MPingerConfigShared, MPingerMessage, MPingerType};
use log::{debug, error};
use socket2::{Domain, Protocol, Socket, Type};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use time::OffsetDateTime;

pub struct MPingerTCPConnect();

impl MPingerTCPConnect {
    pub fn start(
        config: MPingerConfigShared,
        dest: &MPingDestination,
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
            let result = socket.connect_timeout(&dest.sock_addr, timeout);
            let duration = start_time.elapsed().as_millis() as u32;

            let duration = match result {
                Ok(_) => duration,
                Err(e) => {
                    debug!("Error connecting: {}", e);
                    0
                }
            };

            let result = tx.send(MPingerMessage {
                destination_id: dest.id,
                ping_nr: i,
                runner_type: MPingerType::TCPConnect,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration,
                is_error: false,
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
}
