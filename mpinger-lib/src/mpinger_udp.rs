use crate::mpinger::{MPingDestination, MPingerConfigShared, MPingerMessage, MPingerType};
use log::{debug, error};
use socket2::{Domain, Protocol, Socket, Type};
use std::mem::MaybeUninit;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use time::OffsetDateTime;

pub struct MPingerUDP();

impl MPingerUDP {
    pub fn start(
        config: MPingerConfigShared,
        dest: &MPingDestination,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let mut i = 0;
        loop {
            // Create UDP socket
            let socket = match Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)) {
                Ok(socket) => socket,
                Err(e) => {
                    error!("Error creating UDP socket: {:?}", e);
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

            // Send UDP message
            let message = b"ping";
            let start_time = Instant::now();

            if let Some(addr) = dest.sock_addr.as_socket() {
                let ip_str = addr.to_string();
                debug!("Sending UDP ping to {:?}", ip_str);
            }

            let send_result = socket.send_to(message, &dest.sock_addr);
            if let Err(e) = send_result {
                debug!("Error sending UDP packet: {:?}", e);
                let _ = tx.send(MPingerMessage {
                    destination_id: dest.id,
                    ping_nr: i,
                    runner_type: MPingerType::UDPPing,
                    start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                    duration: 0,
                    is_error: true,
                });
                i += 1;
                if count > 0 && i >= count {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(
                    config.read().unwrap().ping_interval,
                ));
                continue;
            }

            // Wait for response
            let mut buf = [MaybeUninit::<u8>::uninit(); 1024];
            let mut is_error = false;
            let duration = match socket.recv_from(&mut buf) {
                Ok(_) => start_time.elapsed().as_micros() as u64,
                Err(e) => {
                    debug!("Error receiving UDP response: {:?}", e);
                    is_error = true;
                    0
                }
            };

            let result = tx.send(MPingerMessage {
                destination_id: dest.id,
                ping_nr: i,
                runner_type: MPingerType::UDPPing,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration,
                is_error,
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
