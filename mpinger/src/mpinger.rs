use crate::{
    mpinger_http_keepalive::MPingerHTTPKeepAlive, mpinger_icmp::MPingerICMP,
    mpinger_rnd::MPingerRnd, mpinger_tcp_connect::MPingerTCPConnect, utils,
};
use anyhow::Result;
use socket2::SockAddr;
use std::net::IpAddr;
use std::rc::Rc;
use std::sync::{Arc, RwLock, mpsc};
use std::thread;

#[derive(Debug)]
pub struct MPingerMessage {
    pub destination_id: usize,
    pub ping_nr: usize,
    pub runner_type: MPingerType,
    pub start_timestamp: i64,
    pub duration: u32,
    pub is_error: bool,
}
impl Iterator for MPingerReader {
    type Item = MPingerMessage;

    fn next(&mut self) -> Option<Self::Item> {
        let timeout = std::time::Duration::from_millis(self.config.read().unwrap().next_timeout);
        match self.rx.recv_timeout(timeout) {
            Ok(data) => Some(data),
            Err(_) => None,
        }
    }
}

pub struct MPingerReader {
    config: MPingerConfigShared,
    rx: Rc<mpsc::Receiver<MPingerMessage>>,
}

impl MPingerReader {
    pub fn new(config: MPingerConfigShared, rx: Rc<mpsc::Receiver<MPingerMessage>>) -> Self {
        Self { config, rx }
    }
}

#[derive(Debug, Clone)]
pub struct MPingDestination {
    // unique id
    pub id: usize,
    // original address
    pub address: String,
    // host part
    pub host: String,
    // port part
    pub port: u16,
    // resolved ip
    pub sock_addr: SockAddr,
    // type
    pub ping_type: MPingerType,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum MPingerType {
    ICMPPing,
    TCPConnect,
    HTTPKeepAlive,
    Rnd,
}

pub type MPingerConfigShared = Arc<RwLock<MPingerConfig>>;

#[derive(Debug, Clone)]
pub struct MPingerConfig {
    pub ping_interval: u64, // ms
    pub timeout: u64,       // ms
    pub next_timeout: u64,  // ms
    pub ping_retries: usize,
    pub default_port: u16,
}
impl Default for MPingerConfig {
    fn default() -> Self {
        MPingerConfig {
            ping_interval: 1000,
            timeout: 1000,
            next_timeout: 5000,
            ping_retries: 3,
            default_port: 80,
        }
    }
}

pub struct MPinger {
    config: MPingerConfigShared,
    // runners: HashMap<MPingerType, MPingerRunnerStruct>,
    rx: Rc<mpsc::Receiver<MPingerMessage>>,
    tx: mpsc::Sender<MPingerMessage>,
    total_addresses: usize,
    destinations: Vec<MPingDestination>,
}

impl MPinger {
    pub fn new(config: MPingerConfig) -> Self {
        let (tx, rx) = mpsc::channel();

        let config = Arc::new(RwLock::new(config));

        let rx = Rc::new(rx);

        Self {
            config,
            rx,
            tx,
            total_addresses: 0,
            destinations: Vec::new(),
        }
    }

    //try to parse and resolve, add to the appropiate runner
    pub fn add_destination(&mut self, runner_type: MPingerType, addr: &str) -> Result<usize> {
        let (ip, port) =
            match utils::parse_host_port(addr, self.config.read().unwrap().default_port) {
                Ok((ip, port)) => (ip, port),
                Err(e) => {
                    return Err(anyhow::anyhow!(e));
                }
            };

        let sock_addr = SockAddr::from(std::net::SocketAddr::new(IpAddr::V4(ip), port));

        self.total_addresses += 1;

        let dest = MPingDestination {
            id: self.total_addresses,
            ping_type: runner_type,
            address: addr.to_string(),
            host: ip.to_string(),
            port,
            sock_addr,
        };

        self.destinations.push(dest.clone());

        Ok(self.total_addresses)
    }

    pub fn get_destination_by_id(&self, id: usize) -> Option<&MPingDestination> {
        self.destinations.iter().find(|&dest| dest.id == id)
    }

    pub fn get_runner_description(&self, runner_type: &MPingerType) -> &str {
        match runner_type {
            MPingerType::ICMPPing => "ICMP ping",
            MPingerType::TCPConnect => "TCP Connect",
            MPingerType::HTTPKeepAlive => "HTTP Keep Alive",
            MPingerType::Rnd => "Random",
        }
    }

    pub fn set_ping_interval(&mut self, ping_interval: u64) -> &Self {
        let mut config = self.config.write().unwrap();
        config.ping_interval = ping_interval;

        self
    }

    pub fn get_ping_interval(&self) -> u64 {
        self.config.read().unwrap().ping_interval
    }

    fn ping_runner(
        config: MPingerConfigShared,
        destinations: Vec<MPingDestination>,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        // run Connect pings
        let handles: Vec<_> = destinations
            .iter()
            .map(|dest| {
                let tx = tx.clone();
                let config = config.clone();
                let dest = dest.clone();

                match dest.ping_type {
                    MPingerType::ICMPPing => thread::spawn(move || {
                        MPingerICMP::start(config.clone(), &dest, tx, count);
                    }),
                    MPingerType::TCPConnect => thread::spawn(move || {
                        MPingerTCPConnect::start(config.clone(), &dest, tx, count);
                    }),
                    MPingerType::HTTPKeepAlive => thread::spawn(move || {
                        MPingerHTTPKeepAlive::start(config.clone(), &dest, tx, count);
                    }),
                    MPingerType::Rnd => thread::spawn(move || {
                        MPingerRnd::start(config.clone(), &dest, tx, count);
                    }),
                }
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    pub fn start(&self, count: usize) -> MPingerReader {
        if !self.destinations.is_empty() {
            let destinations: Vec<MPingDestination> = self.destinations.clone();
            let tx = self.tx.clone();
            let config = self.config.clone();

            thread::spawn(move || {
                MPinger::ping_runner(config, destinations, tx, count);
            });
        }

        MPingerReader::new(self.config.clone(), self.rx.clone())
    }
}
