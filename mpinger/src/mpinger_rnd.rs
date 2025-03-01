use crate::mpinger::{MPingerConfigShared, MPingerMessage, MPingerRunner, MPingerType};
use anyhow::Result;
use rand::prelude::*;
use std::sync::mpsc;
use std::thread;
use time::OffsetDateTime;

pub struct MPingerRnd {
    config: MPingerConfigShared,
    addrs: Vec<String>,
    tx: mpsc::Sender<MPingerMessage>,
}

impl MPingerRunner for MPingerRnd {
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
            let mut rng = ::rand::rngs::StdRng::from_os_rng();
            let mut i = 0;
            loop {
                for (idx, _addr) in addrs.iter().enumerate() {
                    let _ = tx.send(MPingerMessage {
                        idx,
                        runner_type: MPingerType::Rnd,
                        start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                        duration: rng.random_range(0..=300),
                    });
                }

                i += 1;
                if count > 0 && i >= count {
                    break;
                }
                thread::sleep(std::time::Duration::from_millis(
                    config.read().unwrap().ping_interval,
                ));
            }
        });

        Ok(())
    }
}

impl MPingerRnd {
    pub fn new(config: MPingerConfigShared, tx: mpsc::Sender<MPingerMessage>) -> Self {
        Self {
            config,
            tx,
            addrs: Vec::new(),
        }
    }
}
