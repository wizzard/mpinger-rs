use std::net::{Ipv4Addr, ToSocketAddrs};

pub fn parse_host_port(host_port: &str, default_port: u16) -> Result<(Ipv4Addr, u16), String> {
    let parts: Vec<&str> = host_port.split(':').collect();

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

pub struct RunningAverage {
    values: Vec<u64>,
    capacity: usize,
    sum: u64,
    position: usize,
    count: usize,
}

impl RunningAverage {
    pub fn new(capacity: usize) -> Self {
        if capacity == 0 {
            panic!("Capacity must be greater than 0");
        }

        RunningAverage {
            values: vec![0; capacity],
            capacity,
            sum: 0,
            position: 0,
            count: 0,
        }
    }

    pub fn add(&mut self, value: u64) {
        if self.count == self.capacity {
            self.sum = self.sum.saturating_sub(self.values[self.position]);
        } else {
            self.count += 1;
        }

        self.values[self.position] = value;
        self.sum = self.sum.saturating_add(value);

        self.position = (self.position + 1) % self.capacity;
    }

    pub fn get(&self) -> Option<f64> {
        if self.count == 0 {
            None
        } else {
            Some(self.sum as f64 / self.count as f64)
        }
    }

    pub fn count(&self) -> usize {
        self.count
    }

    pub fn clear(&mut self) {
        self.values.fill(0);
        self.sum = 0;
        self.position = 0;
        self.count = 0;
    }
}
