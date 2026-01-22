use anyhow::Result;
use clap::Parser;
use log::{error, info};
use std::sync::Arc;
use tokio::net::UdpSocket;

/// UDP Ping-Pong Server
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Port to bind the UDP server to
    #[arg(short, long, default_value = "8888")]
    port: u16,

    /// IP address to bind to
    #[arg(short, long, default_value = "0.0.0.0")]
    address: String,

    /// Enable debug logging
    #[arg(short, long, default_value_t = false)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.debug {
        env_logger::builder()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::builder()
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    let bind_addr = format!("{}:{}", args.address, args.port);

    info!("Starting UDP ping-pong server on {}", bind_addr);

    // Bind UDP socket
    let socket = UdpSocket::bind(&bind_addr).await?;
    info!("Server listening on {}", socket.local_addr()?);

    let socket = Arc::new(socket);
    let mut buf = vec![0u8; 65536]; // Max UDP packet size

    loop {
        let (len, addr) = match socket.recv_from(&mut buf).await {
            Ok(result) => result,
            Err(e) => {
                error!("Failed to receive data: {}", e);
                continue;
            }
        };

        let received_data = &buf[..len];

        // Check if the received message is "ping"
        if let Ok(message) = std::str::from_utf8(received_data) {
            info!("Received '{}' from {}", message, addr);

            if message.trim().eq_ignore_ascii_case("ping") {
                // Send back "pong"
                let response = b"pong";
                match socket.send_to(response, addr).await {
                    Ok(_) => info!("Sent 'pong' to {}", addr),
                    Err(e) => error!("Failed to send pong to {}: {}", addr, e),
                }
            } else {
                // Echo back whatever was received
                match socket.send_to(received_data, addr).await {
                    Ok(_) => info!("Echoed {} bytes to {}", len, addr),
                    Err(e) => error!("Failed to echo to {}: {}", addr, e),
                }
            }
        } else {
            // If not valid UTF-8, echo back the raw bytes
            match socket.send_to(received_data, addr).await {
                Ok(_) => info!("Echoed {} bytes (binary) to {}", len, addr),
                Err(e) => error!("Failed to echo to {}: {}", addr, e),
            }
        }
    }
}
