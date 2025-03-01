use crate::mpinger_http_keepalive::MPingerHTTPKeepAlive;
use crate::mpinger_icmp::MPingerICMP;
use crate::mpinger_rnd::MPingerRnd;
use crate::mpinger_tcp_connect::MPingerTCPConnect;
use anyhow::Result;
use log::error;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, RwLock, mpsc};

#[derive(Debug)]
pub struct MPingerMessage {
    pub idx: usize,
    pub runner_type: MPingerType,
    pub start_timestamp: i64,
    pub duration: u32,
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

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum MPingerType {
    ICMPPing,
    TCPConnect,
    HTTPKeepAlive,
    Rnd,
}

pub trait MPingerRunner {
    fn add(&mut self, addr: &str);
    fn start(&self, count: usize) -> Result<()>;
    fn get_addr_by_idx(&self, idx: usize) -> Option<&String>;
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
    runners: HashMap<MPingerType, Box<dyn MPingerRunner>>,
    rx: Rc<mpsc::Receiver<MPingerMessage>>,
}

impl MPinger {
    pub fn new(config: MPingerConfig) -> Self {
        let (tx, rx) = mpsc::channel();

        let config = Arc::new(RwLock::new(config));

        let mut runners: HashMap<MPingerType, Box<dyn MPingerRunner>> = HashMap::new();
        runners.insert(
            MPingerType::Rnd,
            Box::new(MPingerRnd::new(Arc::clone(&config), tx.clone())),
        );

        runners.insert(
            MPingerType::TCPConnect,
            Box::new(MPingerTCPConnect::new(config.clone(), tx.clone())),
        );

        runners.insert(
            MPingerType::HTTPKeepAlive,
            Box::new(MPingerHTTPKeepAlive::new(config.clone(), tx.clone())),
        );

        runners.insert(
            MPingerType::ICMPPing,
            Box::new(MPingerICMP::new(config.clone(), tx.clone())),
        );

        let rx = Rc::new(rx);

        Self {
            config,
            runners,
            rx,
        }
    }

    // TODO: return ID of the added address
    pub fn add(&mut self, runner_type: MPingerType, addr: &str) -> &mut Self {
        if let Some(runner) = self.runners.get_mut(&runner_type) {
            runner.add(addr);
        } else {
            error!("No object found for the given enum variant!");
        }

        self
    }

    pub fn get_addr_by_idx(&self, runner_type: MPingerType, idx: usize) -> Option<&String> {
        if let Some(runner) = self.runners.get(&runner_type) {
            runner.get_addr_by_idx(idx)
        } else {
            error!("No object found for the given enum variant!");
            None
        }
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

    pub fn start(&self, count: usize) -> MPingerReader {
        for runner in self.runners.values() {
            if let Err(e) = runner.start(count) {
                error!("Error starting runner: {}", e);
            }
        }
        MPingerReader::new(self.config.clone(), self.rx.clone())
    }
}
