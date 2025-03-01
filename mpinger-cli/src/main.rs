use anyhow::Result;
use clap::Parser;
use log::error;
use mpinger::{MPinger, MPingerConfig, MPingerType};
use std::process;
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, format_description};
use tprint::{TPrint, TPrintAlign};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    debug: bool,
    #[arg(short, long, default_value = "5")]
    count: usize,
    #[arg(short, long, default_value = "1000")]
    interval: u64,
    #[arg(long)]
    icmp: Option<String>,
    #[arg(long)]
    connect: Option<String>,
    #[arg(long)]
    http: Option<String>,
}

struct PingStats {
    idx: usize,
    label: String,
    count: usize,
    timeouts: usize,
    min_ping: u32,
    max_ping: u32,
    //avg_ping: u32,
}

fn main() -> Result<()> {
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

    let conf = MPingerConfig {
        timeout: 1000,
        next_timeout: 4000,
        ..Default::default()
    };
    let mut ping_cli = MPinger::new(conf);
    ping_cli.set_ping_interval(args.interval);

    let mut total_addresses = 0;

    let mut ping_stats = Vec::new();

    if let Some(icmp) = args.icmp {
        let icmp_addresses: Vec<&str> = icmp.split(',').collect();
        for address in icmp_addresses {
            ping_cli.add(MPingerType::ICMPPing, address);

            ping_stats.push(PingStats {
                idx: total_addresses,
                label: address.to_string(),
                count: 0,
                timeouts: 0,
                min_ping: u32::MAX,
                max_ping: 0,
            });
            total_addresses += 1;
        }
    }

    if let Some(connect) = args.connect {
        let connect_addresses: Vec<&str> = connect.split(',').collect();
        for address in connect_addresses {
            ping_cli.add(MPingerType::TCPConnect, address);

            ping_stats.push(PingStats {
                idx: total_addresses,
                label: address.to_string(),
                count: 0,
                timeouts: 0,
                min_ping: u32::MAX,
                max_ping: 0,
            });
            total_addresses += 1;
        }
    }

    if let Some(http) = args.http {
        let http_addresses: Vec<&str> = http.split(',').collect();
        for address in http_addresses {
            ping_cli.add(MPingerType::HTTPKeepAlive, address);

            ping_stats.push(PingStats {
                idx: total_addresses,
                label: address.to_string(),
                count: 0,
                timeouts: 0,
                min_ping: u32::MAX,
                max_ping: 0,
            });
            total_addresses += 1;
        }
    }

    if total_addresses == 0 {
        error!("No addresses to ping!");
        return Ok(());
    }

    let ping_stats = Arc::new(Mutex::new(ping_stats));
    let ping_stats_clone = Arc::clone(&ping_stats);
    ctrlc::set_handler(move || {
        let ping_stats = ping_stats_clone.lock().unwrap();

        println!();
        let mut tp = TPrint::new(true, true, 0, 3);

        tp.column_add("", TPrintAlign::Center, TPrintAlign::Left)
            .column_add("Total pings", TPrintAlign::Center, TPrintAlign::Left)
            .column_add("Timeouts", TPrintAlign::Center, TPrintAlign::Left)
            .column_add("Min ping", TPrintAlign::Center, TPrintAlign::Left)
            .column_add("Max ping", TPrintAlign::Center, TPrintAlign::Left);

        for ping_stat in ping_stats.iter() {
            tp.add_data(&ping_stat.label)
                .add_data(ping_stat.count)
                .add_data(ping_stat.timeouts)
                .add_data(ping_stat.min_ping)
                .add_data(ping_stat.max_ping);
        }
        tp.print().unwrap();

        process::exit(0);
    })
    .expect("Error setting Ctrl+C handler");

    let pinger_reader = ping_cli.start(args.count);
    for pdata in pinger_reader {
        let date: OffsetDateTime = OffsetDateTime::from_unix_timestamp(pdata.start_timestamp)?;
        let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")?;
        let date = date.format(&format)?;

        println!(
            "[{}] [{}] [{}] {}: {} ms",
            date,
            ping_cli.get_runner_description(&pdata.runner_type),
            pdata.idx,
            ping_cli
                .get_addr_by_idx(pdata.runner_type, pdata.idx)
                .unwrap(),
            pdata.duration
        );

        let mut ps = ping_stats.lock().unwrap();
        if let Some(stat) = ps.iter_mut().find(|stat| stat.idx == pdata.idx) {
            stat.count += 1;

            if pdata.duration == 0 {
                stat.timeouts += 1;
            }

            if pdata.duration < stat.min_ping {
                stat.min_ping = pdata.duration;
            }
            if pdata.duration > stat.max_ping {
                stat.max_ping = pdata.duration;
            }
        }
    }

    Ok(())
}
