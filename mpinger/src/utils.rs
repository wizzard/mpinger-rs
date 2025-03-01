use std::net::{Ipv4Addr, ToSocketAddrs};

pub fn parse_host_port(
    host_port: &str,
    default_port: Option<u16>,
) -> Result<(Ipv4Addr, u16), String> {
    let parts: Vec<&str> = host_port.split(':').collect();

    let default_port = default_port.unwrap_or(0);

    let (host, port) = match parts.len() {
        1 => (parts[0], Some(default_port)),
        2 => (
            parts[0],
            Some(
                parts[1]
                    .parse::<u16>()
                    .map_err(|e| format!("Invalid port: {}", e))?,
            ),
        ),
        _ => (host_port, Some(default_port)),
    };

    let port = port.unwrap_or(default_port);

    // Resolve hostname to IP address
    let socket_addr = format!("{}:{}", host, port)
        .to_socket_addrs()
        .map_err(|e| format!("Failed to resolve hostname: {}", e))?
        .next()
        .ok_or("No address resolved".to_string())?;

    match socket_addr.ip() {
        std::net::IpAddr::V4(ipv4) => Ok((ipv4, port)),
        _ => Err("IPv6 addresses not supported".to_string()),
    }
}
