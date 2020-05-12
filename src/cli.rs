use clap::{Arg, App};
use log::LevelFilter;
use crate::pinger::{SelectVersion, generate_targets, PingTargets};

pub fn init() -> (PingTargets, PingTargets) {
    simple_logger::init().unwrap();
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

    let mut versions: Vec<SelectVersion> = vec![];

    if matches.occurrences_of("6") > 0 {
        versions.push(SelectVersion::V6);
    }

    if matches.occurrences_of("4") > 0 {
        versions.push(SelectVersion::V4);
    }

    let mut targets = generate_targets(hosts.clone(), &versions).unwrap();
    let mut targets2 = generate_targets(hosts.clone(), &versions).unwrap();
    targets.start();
    targets2.start();
    (targets, targets2)
}
