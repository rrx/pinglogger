//extern crate clap;
//extern crate pinglogger;
//extern crate pretty_env_logger;
//extern crate log;
use std::collections::HashMap;
use log::LevelFilter;
use std::error::Error;
use clap::{Arg, App};
use std::thread::sleep;
use std::thread;
//use dipstick;
//use dipstick::*;
use pinglogger::pinger as pinger;

use std::time::{Instant, Duration, SystemTime};
use crossbeam_channel::bounded;
//use crossbeam_channel::{Sender, Receiver};

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let matches = App::new("ping")
        .version("1.0")
        .author("Ryan Sadler <rrsadler@gmail.com>")
        .about("Pings stuff")
        .arg(
            Arg::with_name("LISTEN")
            .short("l")
            .multiple(false)
            .long("listen")
            .help("listen and dump packets"))
        .arg(Arg::with_name("4")
            .short("4")
            .help("IPV4"))
        .arg(Arg::with_name("6")
            .short("6")
            .help("IPV6"))
        .arg(Arg::with_name("v")
            .short("v")
            .multiple(true)
            .help("Sets the level of verbosity"))
        .arg(Arg::with_name("HOST")
            .help("Sets the input file to use")
            .multiple(true)
        ).get_matches();

    let verbose = matches.occurrences_of("v");
    if verbose > 0 {
        log::set_max_level(LevelFilter::Debug);
    } else {
        log::set_max_level(LevelFilter::Info);
    }

    let hosts: Vec<_> = match matches.values_of("HOST") {
        Some(x) => x.collect(),
        None => Vec::new()
    };

    let mut versions: Vec<pinger::SelectVersion> = vec![];

    if matches.occurrences_of("6") > 0 {
        versions.push(pinger::SelectVersion::V6);
    }

    if matches.occurrences_of("4") > 0 {
        versions.push(pinger::SelectVersion::V4);
    }

    let (s, r) = bounded(50);
    let s2 = s.clone();
    let mut targets = pinglogger::pinger::generate_targets(hosts.clone(), &versions).unwrap();
    let mut targets2 = pinglogger::pinger::generate_targets(hosts.clone(), &versions).unwrap();
    targets.start();
    targets2.start();

    // bail if we don't have anything
    if targets.output.len() == 0 {
        return Ok(());
    }

    if let None = matches.value_of("LISTEN") {

        thread::spawn(move || {
            //let metrics = Graphite::send_to("localhost:2003")
            //.expect("Connected")
            //.named("my_app")
            //.metrics();
            let mut count = 0;
            loop {
                targets.ping(count, &s);
                sleep(Duration::from_secs(1));
                count += 1;
            }
        });
    }

    thread::spawn(move || {
        targets2.poll(&s2).unwrap();
    });

    let mut h = HashMap::new();
    r.iter().for_each(|x| {
        match x {
            pinger::UniPacket::SendPacket {host, addr, seq, ident, t} => {
                h.insert( (ident, seq), (host, addr, t ));
            },
            pinger::UniPacket::RecvPacket {seq, ident, t, ttl, size} => {
                match h.remove( &(ident, seq) ) {
                    Some( (host, addr, t2) ) => {
                        let d = Duration::from_nanos( (t - t2) as u64);
                        //println!("{} {:?}", addr, d);
                        let t: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_micros() as f64;
                        println!("[{:.6}] {} bytes from {} ({}): icmp_seq={} ttl={} time={:?}", t/1000_000., size, host, addr, seq, ttl, d);
                    },
                    None => {}
                }
            }
        }
        //println!("{:?}",x);
    });

    Ok(())
}
