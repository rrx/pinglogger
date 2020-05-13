use std::collections::HashMap;
use std::error::Error;
use std::thread::sleep;
use std::thread;
use pinglogger::pinger::UniPacket;

use std::time::{Duration, SystemTime};
use crossbeam_channel::bounded;
use pinglogger::{cli, stats};

fn main() -> Result<(), Box<dyn Error>> {
    let (targets, targets2) = cli::init();

    let (s, r) = bounded(50);
    let s2 = s.clone();

    // bail if we don't have anything
    if targets.output.len() == 0 {
        return Ok(());
    }

    let mut metrics = stats::metrics("app");

    thread::spawn(move || {
        let mut count = 0;
        loop {
            targets.ping(count, &s);
            sleep(Duration::from_secs(1));
            count += 1;
        }
    });

    thread::spawn(move || {
        targets2.poll(&s2).unwrap();
    });

    let mut h = HashMap::new();
    r.iter().for_each(|x| {
        match x {
            UniPacket::SendPacket {host, addr, seq, ident, t} => {
                h.insert( (ident, seq), (host, addr, t ));
            },
            UniPacket::RecvPacket {seq, ident, t, ttl, size} => {
                match h.remove( &(ident, seq) ) {
                    Some( (host, addr, t2) ) => {
                        let d = Duration::from_nanos( (t - t2) as u64);
                        let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_micros();
                        println!("[{:.6}] {} bytes from {} ({}): icmp_seq={} ttl={} time={:.4?}", 
                            (t as f64)/1_000_000., size, host.to_string(), addr, seq, ttl, d);
                        metrics.update(&d, &*host);
                    },
                    None => {}
                }
            }
        }
    });

    Ok(())
}

