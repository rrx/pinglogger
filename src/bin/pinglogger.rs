extern crate clap;
extern crate pinglogger;
extern crate pretty_env_logger;
#[macro_use] extern crate log;
use log::{LevelFilter};
use std::error::Error;
use clap::{Arg, App};
//use std::convert::TryInto;

use std::thread::{sleep};
use std::thread;
use std::time::{Duration };
//use dipstick;
//use dipstick::*;
use pinglogger::pinger as pinger;

fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    let matches = App::new("ping")
    .version("1.0")
    .author("Ryan Sadler <rrsadler@gmail.com>")
    .about("Pings stuff")
    // listen
    .arg(
        Arg::with_name("LISTEN")
        .short("l")
        .multiple(false)
        .long("listen")
        .help("listen and dump packets")
    ).arg(Arg::with_name("4")
        .short("4")
        .help("IPV4")
    ).arg(Arg::with_name("6")
        .short("6")
        .help("IPV6")
    ).arg(Arg::with_name("v")
        .short("v")
        .multiple(true)
        .help("Sets the level of verbosity")
    ).arg(Arg::with_name("HOST")
        .help("Sets the input file to use")
        .multiple(true)
    ).get_matches();

    //debug!("{:?}", matches);

    let verbose = matches.occurrences_of("v");
    if verbose > 0 {
        log::set_max_level(LevelFilter::Debug);
    } else {
        log::set_max_level(LevelFilter::Info);
    }

    let hosts: Vec<_> = matches.values_of("HOST").expect("Missing hosts").collect();

    let mut versions: Vec<pinger::SelectVersion> = vec![];

    if matches.occurrences_of("6") > 0 {
        versions.push(pinger::SelectVersion::V6);
    }
    
    if matches.occurrences_of("4") > 0 {
        versions.push(pinger::SelectVersion::V4);
    }

    //let start = Instant::now();
 
    if let None = matches.value_of("LISTEN") {
        let mut targets = pinglogger::pinger::generate_targets(hosts.clone(), &versions).unwrap();
        for site in &targets.output {
            debug!("pinging {}", site.sock_addr.ip().to_string());
        }


        thread::spawn(move || {
            //let metrics = Graphite::send_to("localhost:2003")
                //.expect("Connected")
                //.named("my_app")
                //.metrics();

            let mut p = pinger::Pinger::default();
            loop {
                for site in targets.output.iter_mut() {
                    p.ping(&site);

                }

                sleep(Duration::from_secs(1));
                p.count += 1;
            }
        });
    }

    let mut p = pinger::Pinger::default();
    let targets = pinglogger::pinger::generate_targets(hosts, &versions).unwrap();
    p.poll(&targets)?;
    Ok(())
}
