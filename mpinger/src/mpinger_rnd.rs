use crate::mpinger::{MPingDestination, MPingerConfigShared, MPingerMessage, MPingerType};
use rand::prelude::*;
use std::sync::mpsc;
use std::thread;
use time::OffsetDateTime;

pub struct MPingerRnd();

impl MPingerRnd {
    pub fn start(
        config: MPingerConfigShared,
        dest: &MPingDestination,
        tx: mpsc::Sender<MPingerMessage>,
        count: usize,
    ) {
        let mut rng = ::rand::rngs::StdRng::from_os_rng();

        let mut i = 0;
        loop {
            let _ = tx.send(MPingerMessage {
                destination_id: dest.id,
                ping_nr: i,
                runner_type: MPingerType::Rnd,
                start_timestamp: OffsetDateTime::now_utc().unix_timestamp(),
                duration: rng.random_range(0..=300),
                is_error: false,
            });

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
