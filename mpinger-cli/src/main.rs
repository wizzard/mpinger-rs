use anyhow::Result;
use clap::Parser;
use log::error;
use mpinger::{MPinger, MPingerConfig, MPingerType, RunningAverage};
use std::process;
use std::sync::{Arc, Mutex};
use time::{OffsetDateTime, format_description};
use tprint::{TPrint, TPrintAlign};

#[derive(Parser)]
struct Args {
    #[arg(short, long)]
    debug: bool,
    /// Number of pings to send, 0 for infinite pings (press Ctrl+C to stop)
    #[arg(short, long, default_value = "5")]
    count: usize,
    /// Interval between pings in ms
    #[arg(short, long, default_value = "1000")]
    interval: u64,
    /// List of comma separated addresses to perform ICMP pings
    #[arg(long)]
    icmp: Option<String>,
    /// List of comma separated addresses to perform TCP connect pings (default port 80)
    #[arg(long)]
    connect: Option<String>,
    /// List of comma separated addresses to perform HTTP keepalive pings (default port 80)
    #[arg(long)]
    http: Option<String>,
    /// List of comma separated addresses to perform UDP pings (default port 8888)
    #[arg(long)]
    udp: Option<String>,
}

struct PingStats {
    idx: usize,
    label: String,
    ping_type: String,
    count: usize,
    timeouts: usize,
    min_ping: Option<u64>,
    max_ping: Option<u64>,
    avg_ping: RunningAverage,
}

// Format duration in milliseconds
fn format_duration_u64(duration: u64) -> String {
    format!("{:.2} ms", duration as f64 / 1_000.0)
}

fn format_duration_f64(duration: f64) -> String {
    format!("{:.2} ms", duration / 1_000.0)
}

fn print_stats(ping_stats: &[PingStats]) {
    let mut tp = TPrint::new(true, true, 0, 3);

    tp.column_add("Address", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Type", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Total pings", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Timeouts", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Min ping", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Max ping", TPrintAlign::Center, TPrintAlign::Left)
        .column_add("Avg ping", TPrintAlign::Center, TPrintAlign::Left);

    for ping_stat in ping_stats.iter() {
        tp.add_data(&ping_stat.label)
            .add_data(&ping_stat.ping_type)
            .add_data(ping_stat.count)
            .add_data(ping_stat.timeouts)
            .add_data(format_duration_u64(ping_stat.min_ping.unwrap_or(0)))
            .add_data(format_duration_u64(ping_stat.max_ping.unwrap_or(0)))
            .add_data(format_duration_f64(ping_stat.avg_ping.get().unwrap_or(0.0)));
    }
    tp.print().unwrap();
}

const MAX_AVG_PINGS: usize = 100;

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
        next_timeout: 3000,
        ..Default::default()
    };
    let mut ping_cli = MPinger::new(conf);
    ping_cli.set_ping_interval(args.interval);

    let mut total_addresses = 0;

    let mut ping_stats = Vec::new();

    if let Some(icmp) = args.icmp {
        let icmp_addresses: Vec<&str> = icmp.split(',').collect();
        for address in icmp_addresses {
            let id = ping_cli.add_destination(MPingerType::ICMPPing, address)?;

            ping_stats.push(PingStats {
                idx: id,
                label: address.to_string(),
                ping_type: "ICMP".to_string(),
                count: 0,
                timeouts: 0,
                min_ping: None,
                max_ping: None,
                avg_ping: RunningAverage::new(MAX_AVG_PINGS),
            });
            total_addresses += 1;
        }
    }

    if let Some(connect) = args.connect {
        let connect_addresses: Vec<&str> = connect.split(',').collect();
        for address in connect_addresses {
            let id = ping_cli.add_destination(MPingerType::TCPConnect, address)?;

            ping_stats.push(PingStats {
                idx: id,
                label: address.to_string(),
                ping_type: "CONN".to_string(),
                count: 0,
                timeouts: 0,
                min_ping: None,
                max_ping: None,
                avg_ping: RunningAverage::new(MAX_AVG_PINGS),
            });
            total_addresses += 1;
        }
    }

    if let Some(http) = args.http {
        let http_addresses: Vec<&str> = http.split(',').collect();
        for address in http_addresses {
            let id = ping_cli.add_destination(MPingerType::HTTPKeepAlive, address)?;

            ping_stats.push(PingStats {
                idx: id,
                label: address.to_string(),
                ping_type: "HTTP".to_string(),
                count: 0,
                timeouts: 0,
                min_ping: None,
                max_ping: None,
                avg_ping: RunningAverage::new(MAX_AVG_PINGS),
            });
            total_addresses += 1;
        }
    }

    if let Some(udp) = args.udp {
        let udp_addresses: Vec<&str> = udp.split(',').collect();
        for address in udp_addresses {
            let id = ping_cli.add_destination(MPingerType::UDPPing, address)?;

            ping_stats.push(PingStats {
                idx: id,
                label: address.to_string(),
                ping_type: "UDP".to_string(),
                count: 0,
                timeouts: 0,
                min_ping: None,
                max_ping: None,
                avg_ping: RunningAverage::new(MAX_AVG_PINGS),
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

        print_stats(&ping_stats);
        process::exit(0);
    })
    .expect("Error setting Ctrl+C handler");

    let pinger_reader = ping_cli.start(args.count);

    /*
    let rx = pinger_reader.get_rx();
    let mut iter = rx.try_iter();
    loop {
        loop {
            let val = iter.next();
            if val.is_none() {
                println!("No more values");
                break;
            }
            let ping_message = val.unwrap();
            if ping_message.is_error {
                continue;
            }
            println!("Ping message: {:?}", ping_message);
        }
        println!("Sleeping for 5 second");
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
    */

    for ping_message in pinger_reader {
        let date: OffsetDateTime =
            OffsetDateTime::from_unix_timestamp(ping_message.start_timestamp)?;
        let format = format_description::parse("[year]-[month]-[day] [hour]:[minute]:[second]")?;
        let date = date.format(&format)?;

        println!(
            "[{}] [{}] [{}] {}: {} ms",
            date,
            ping_cli.get_runner_description(&ping_message.runner_type),
            ping_message.ping_nr + 1,
            ping_cli
                .get_destination_by_id(ping_message.destination_id)
                .unwrap()
                .address,
            ping_message.duration
        );

        let mut ps = ping_stats.lock().unwrap();
        if let Some(stat) = ps
            .iter_mut()
            .find(|stat| stat.idx == ping_message.destination_id)
        {
            stat.count += 1;

            if ping_message.is_error || ping_message.duration == 0 {
                stat.timeouts += 1;
            } else {
                stat.avg_ping.add(ping_message.duration);
            }

            if stat.min_ping.is_none() || Some(ping_message.duration) < stat.min_ping {
                stat.min_ping = Some(ping_message.duration);
            }

            if stat.max_ping.is_none() || Some(ping_message.duration) > stat.max_ping {
                stat.max_ping = Some(ping_message.duration);
            }
        }
    }

    let ping_stats = ping_stats.lock().unwrap();
    print_stats(&ping_stats);

    Ok(())
}
