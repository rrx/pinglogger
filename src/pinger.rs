use std::collections::HashMap;
use std::error::Error;
use std::process;
use std::collections::HashSet;
use std::convert::TryInto;

use pnet::packet::icmp::{echo_reply,IcmpType};
use std::os::unix::io::{AsRawFd};

use mio::{Events, Interest, Poll, Token};
use socket2::{Domain, Protocol, Type};
use mio::unix::SourceFd;
//use pnet::packet::icmp;
use pnet::packet::icmpv6::{Icmpv6Packet,Icmpv6Type};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use itertools::Itertools;

use std::time::{Instant, Duration, SystemTime};
use std::net::{SocketAddr};
use dns_lookup::lookup_host;
use log::*;

// Some tokens to allow us to identify which event is for which socket.
const PING: Token = Token(2);
const PING_V6: Token = Token(3);

const TOKEN_SIZE: usize = 24;
const ICMP_HEADER_SIZE: usize = 8;
const ECHO_REQUEST_BUFFER_SIZE: usize = ICMP_HEADER_SIZE + TOKEN_SIZE + 32;


pub struct Site {
    pub host: String,
    pub ident: u16,
    pub sock_addr: SocketAddr,
}

#[derive(PartialEq, Debug)]
pub enum SelectVersion {
    V4,
    V6
}

pub struct PingTargets {
    pub output: Vec<Site>,
    pub sources: HashMap<u16,String>,
    pub addrs: HashSet<std::net::IpAddr>,
}

impl Default for PingTargets {
    fn default() -> Self {
        PingTargets {
            output: vec![],
            sources: HashMap::new(),
            addrs: HashSet::new()
        }
    }
}

pub fn generate_targets(hosts: Vec<&str>, versions: &Vec<SelectVersion>) -> std::result::Result<PingTargets, Box<dyn Error>> {
    let mut result = PingTargets::default();

    hosts.iter().map(|&host| {
        match lookup_host(host) {
            Ok(r) => Some( r.into_iter().map(move |x| (host, x))),
            Err(e) => {
                error!("Err: {}", e);
                None
            }
        }
    }).filter_map(Option::Some).map(|x| {
        debug!("x{:?}", x);
        x
    }).flatten().flatten().enumerate().for_each(|(i, (host, x))| {
        debug!("y{:?} {:?} {:?}", i, host, x);
        let sock_addr: SocketAddr = (x, 0).into();

        let both = !versions.contains(&SelectVersion::V4) && !versions.contains(&SelectVersion::V6);

        match sock_addr {
            SocketAddr::V4(_) if both || versions.contains(&SelectVersion::V4) => {
                result.addrs.insert(x.clone());
                let site = Site {
                    host: host.to_string(),
                    ident: process::id() as u16 + i as u16,
                    sock_addr: sock_addr,
                };
                result.sources.insert(site.ident.clone(), site.host.to_string());
                result.output.push(site);
            }
            SocketAddr::V6(_) if both || versions.contains(&SelectVersion::V6) => {
                result.addrs.insert(x.clone());
                let site = Site {
                    host: host.to_string(),
                    ident: process::id() as u16 + i as u16,
                    sock_addr: sock_addr,
                };
                result.sources.insert(site.ident.clone(), site.host.to_string());
                result.output.push(site);
            }
            // default skip
            _ => {}
        }
    });
    Ok(result)
}

pub struct Pinger {
    pub ping: crate::icmp::Socket,
    pub ping_v6: crate::icmp::Socket,
    pub start_instant: Instant,
    pub count: u16
}

impl Default for Pinger {
    fn default() -> Self {
        Self {
            ping: crate::icmp::Socket::new(Domain::ipv4(), Type::raw(), Protocol::icmpv4()).unwrap(),
            ping_v6: crate::icmp::Socket::new(Domain::ipv6(), Type::raw(), Protocol::icmpv6()).unwrap(),
            start_instant: Instant::now(),
            count: 0
        }
    }
}

impl Pinger {
    pub fn start(&mut self) {
        self.start_instant = Instant::now();
    }

    pub fn ping(&self, site: &Site) {
        let mut ping_buffer = [0u8; ECHO_REQUEST_BUFFER_SIZE];
        let now: u128 = self.start_instant.elapsed().as_nanos();
        let seq = self.count;

        let request = crate::icmp::EchoRequest {
            ident: site.ident,
            seq_cnt: seq,
            payload: &now.to_be_bytes(),
        };

        let target: &SocketAddr = &site.sock_addr;

        match site.sock_addr {
            SocketAddr::V4(_) => {
                request.encode::<crate::icmp::IcmpV4>(&mut ping_buffer).unwrap();
                self.ping.send_to(&ping_buffer[..64], &target.clone().into()).unwrap();
                //ping.socket.flush().unwrap();
            },
            SocketAddr::V6(_) => {
                request.encode::<crate::icmp::IcmpV6>(&mut ping_buffer).unwrap();
                self.ping_v6.send_to(&ping_buffer[..64], &target.clone().into()).unwrap();
                //ping_v6.socket.flush().unwrap();

            }

        }

        //let packet = echo_request::EchoRequestPacket::new(&ping_buffer);
        // debug!("{:?}", packet);

        // if verbose > 0 {
        //     debug!("send: {} {:02x}", target, ping_buffer.iter().format(" "));
        // }
        // site.socket.send_to(&ping_buffer, &target.clone().into()).unwrap();
        // site.socket.flush().unwrap();
    }

    pub fn poll(&mut self, targets: &PingTargets) -> Result<(), Box<dyn Error>> {

        // Create a poll instance.
        let mut poll = Poll::new().unwrap();
        // Create storage for events.
        let mut events = Events::with_capacity(128);

        poll.registry().register(&mut SourceFd(&self.ping.socket.as_raw_fd()), PING, Interest::READABLE)?;
        poll.registry().register(&mut SourceFd(&self.ping_v6.socket.as_raw_fd()), PING_V6, Interest::READABLE)?;

        // Start an event loop.
        loop {
            // Poll Mio for events, blocking until we get an event.
            poll.poll(&mut events, None).unwrap();

            for event in events.iter() {
                match event.token() {
                    PING_V6 => {
                        loop {
                            let mut packet = [0u8;2048]; 
                            match self.ping_v6.recv(&mut packet) {
                                Ok(num) => {
                                    //debug!("Packet {:02x}", packet[..num].iter().format(" "));

                                    if let Some(icmpv6) = Icmpv6Packet::new(&packet[..num]) {
                                        if icmpv6.get_icmpv6_type() == Icmpv6Type(129) {
                                            //debug!("ICMPV6 Reply {:?} {:02x}", icmpv6, icmpv6.payload().iter().format(" "));

                                            if let Some(reply) = echo_reply::EchoReplyPacket::new(&packet[..num]) {
                                                //if verbose > 0 {
                                                    ////debug!("ECHO {:?} {:02x}", reply, reply.payload().iter().format(" "));
                                                //}
                                                match targets.sources.get(&reply.get_identifier()) {
                                                    Some(addr) => {
                                                        //debug!("Ident {:?}", reply.get_identifier());
                                                        //if reply.get_identifier() == ident {
                                                        let (int_bytes, _) = reply.payload().split_at(std::mem::size_of::<u128>());
                                                        let x = u128::from_be_bytes(int_bytes.try_into().unwrap());
                                                        //debug!("{:?}", x);
                                                        let e = self.start_instant.elapsed() - Duration::from_nanos(x as u64);

                                                        let size = num;
                                                        // let source = ipv6_packet.get_source();
                                                        let seq = reply.get_sequence_number();
                                                        // let ttl = ipv6_packet.get_hop_limit();
                                                        // //debug!("{:?}", (reply.get_identifier(), seq, x, e));
                                                        let t: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
                                                        // debug!("[{:.6}] {} bytes from {} ({}): icmp_seq={} ttl={} time={:?}", t/1000., size, addr, source, seq, ttl, e);
                                                        println!("[{:.6}] {} bytes from {}: icmp_seq={} time={:?}", t/1000., size, addr, seq, e);
                                                        }
                                                    _ => {}
                                                    }
                                                }

                                            } else {
                                                continue;
                                            }
                                        }

                                        if let Some(ipv6_packet) = Ipv6Packet::new(&packet[..num]) {
                                            debug!("IPV6-payload {:02x}", ipv6_packet.payload().iter().format(" "));
                                            debug!("IPV6 {:?}", ipv6_packet);
                                            if targets.addrs.contains(&ipv6_packet.get_source().into()) {
                                                //debug!("v6 {:02x}", packet.iter().format(" "));
                                                if let Some(icmpv6) = Icmpv6Packet::new(&packet[..num]) {
                                                    if icmpv6.get_icmpv6_type() == Icmpv6Type(129) {
                                                        //debug!("ICMPV6 Reply {:?} {:02x}", icmpv6, icmpv6.payload().iter().format(" "));
                                                    } else {
                                                        //debug!("OOB ICMPV6({}) {:?} {:02x}", icmpv6.get_icmpv6_type().0, icmpv6, icmpv6.payload().iter().format(" "));
                                                        continue;
                                                    }
                                                }

                                                if let Some(reply) = echo_reply::EchoReplyPacket::new(&packet[..num]) {
                                                    //if verbose > 0 {
                                                        debug!("ECHO {:?} {:02x}", reply, reply.payload().iter().format(" "));
                                                    //}
                                                    // } else {
                                                    //     debug!("ECHO {:?}", reply);
                                                    // }

                                            }

                                        } else {
                                            //debug!("OOB {:?}", ipv6_packet);
                                            continue;
                                        }
                                    }


                                }
                                Err(_) => {
                                    //error!("Error: {:?}", e);
                                    break;
                                }
                            }
                        }
                    }
                    PING => {
                        //debug!("ping {:?}", event);
                        loop {
                            let mut packet = [0u8;2048]; 
                            match self.ping.recv(&mut packet) {
                                Ok(num) => {                         
                                    if let Some(ipv4_packet) = Ipv4Packet::new(&packet[..num]) {
                                        //debug!("IPV4-payload {:02x}", ipv4_packet.payload().iter().format(" "));

                                        if targets.addrs.contains(&ipv4_packet.get_source().into()) {
                                            //debug!("IPV4 {:?}", ipv4_packet);
                                        } else {
                                            //debug!("OOB {:?}", ipv4_packet);
                                            //return Ok(());
                                        }

                                        if let Some(reply) = echo_reply::EchoReplyPacket::new(ipv4_packet.payload()) { //&packet[..num]) {
                                            //debug!("Ident {:?}", reply.get_identifier());
                                            if reply.get_icmp_type() == IcmpType(0) {

                                            } else {
                                                //debug!("OOB ECHO {:?} {:02x}", reply, reply.payload().iter().format(" "));
                                                //return Ok(());
                                            }

                                            match targets.sources.get(&reply.get_identifier()) {
                                                Some(addr) => {
                                                    //if reply.get_identifier() == ident {
                                                    let (int_bytes, _) = reply.payload().split_at(std::mem::size_of::<u128>());
                                                    let x = u128::from_be_bytes(int_bytes.try_into().unwrap());
                                                    let e = self.start_instant.elapsed() - Duration::from_nanos(x as u64);
                                                    //}

                                                    //64 bytes from sea15s12-in-x0e.1e100.net (2607:f8b0:400a:809::200e): icmp_seq=2 ttl=57 time=6.26 ms
                                                    let size = ipv4_packet.payload().len();
                                                    let source = ipv4_packet.get_source();
                                                    let seq = reply.get_sequence_number();
                                                    let ttl = ipv4_packet.get_ttl();
                                                    //debug!("{:?}", (reply.get_identifier(), seq, x, e));
                                                    let t: f64 = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as f64;
                                                    println!("[{:.6}] {} bytes from {} ({}): icmp_seq={} ttl={} time={:?}", t/1000., size, addr, source, seq, ttl, e);
                                                }
                                                None => {}
                                            }
                                        }
                                        }

                                        // let icmp_packet = IcmpPacket::new(&packet[..num]);
                                        // if let Some(icmp_packet) = icmp_packet {
                                        //     debug!("ICMP {:?}", icmp_packet);
                                        // }

                                    },
                                    Err(_) => {
                                        //debug!("Drained: {:?}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        // We don't expect any events with tokens other than those we provided.
                        _ => unreachable!()
                    }

                }
            }
        }
    }

