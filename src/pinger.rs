use std::collections::HashMap;
use std::error::Error;
use std::process;
use std::collections::HashSet;

use pnet::packet::icmp::echo_reply;
use std::os::unix::io::{AsRawFd};

use mio::{Events, Interest, Poll, Token};
use socket2::{Domain, Protocol, Type};
use mio::unix::SourceFd;
use pnet::packet::icmpv6::{Icmpv6Packet,Icmpv6Type};
use pnet::packet::ipv4::Ipv4Packet;
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;
use itertools::Itertools;

use std::time::Instant;
use std::net::SocketAddr;
use dns_lookup::lookup_host;
use log::*;
use crossbeam_channel::Sender;

// Some tokens to allow us to identify which event is for which socket.
const PING: Token = Token(2);
const PING_V6: Token = Token(3);

const TOKEN_SIZE: usize = 24;
const ICMP_HEADER_SIZE: usize = 8;
const ECHO_REQUEST_BUFFER_SIZE: usize = ICMP_HEADER_SIZE + TOKEN_SIZE + 32;

#[derive(Clone)]
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

#[derive(PartialEq, Debug)]
pub enum UniPacket {
    SendPacket {
        host: String,
        addr: String,
        seq: u16,
        ident: u16,
        t: u128
    },
    RecvPacket {
        seq: u16,
        ident: u16,
        t: u128,
        ttl: u8,
        size: usize
    }
}

pub struct PingTargets {
    pub output: Vec<Site>,
    pub ping_output: Vec<Site>,
    pub sources: HashMap<u16,String>,
    pub addrs: HashSet<std::net::IpAddr>,
    pub ping: crate::icmp::Socket,
    pub ping_v6: crate::icmp::Socket,
    pub start_instant: Instant,
}

impl Default for PingTargets {
    fn default() -> Self {
        PingTargets {
            output: vec![],
            ping_output: vec![],
            sources: HashMap::new(),
            addrs: HashSet::new(),
            ping: crate::icmp::Socket::new(Domain::ipv4(), Type::raw(), Protocol::icmpv4()).unwrap(),
            ping_v6: crate::icmp::Socket::new(Domain::ipv6(), Type::raw(), Protocol::icmpv6()).unwrap(),
            start_instant: Instant::now(),
        }
    }
}

impl PingTargets {
    pub fn start(&mut self) {
        self.start_instant = Instant::now();
    }

    pub fn ping(&self, count: u16, s: &Sender<UniPacket>) {
        self.output.iter().for_each(|site| self.ping_site(&site, count, s));
    }

    pub fn ping_site(&self, site: &Site, count: u16, s: &Sender<UniPacket>) {
        let mut ping_buffer = [0u8; ECHO_REQUEST_BUFFER_SIZE];
        let now: u128 = self.start_instant.elapsed().as_nanos();
        let seq = count;

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
            },
            SocketAddr::V6(_) => {
                request.encode::<crate::icmp::IcmpV6>(&mut ping_buffer).unwrap();
                self.ping_v6.send_to(&ping_buffer[..64], &target.clone().into()).unwrap();
            }
        }
        s.send(UniPacket::SendPacket { 
            host: site.host.clone(),
            addr: site.sock_addr.ip().to_string(),
            seq,
            ident: site.ident,
            t: now
        }).unwrap();
    }

    pub fn handle_icmpv6(&self, packet: &[u8], num: usize, s: &Sender<UniPacket>) {
        if let Some(ipv6) = Ipv6Packet::new(&packet[..num]) {
            debug!("IPV6 {:?} {:02x}", ipv6, ipv6.payload().iter().format(" "));
        }

        if let Some(icmpv6) = Icmpv6Packet::new(&packet[..num]) {
            if icmpv6.get_icmpv6_type() == Icmpv6Type(129) {
                debug!("ICMPV6 Reply {:?} {:02x}", icmpv6, packet[..num].iter().format(" "));

                if let Some(reply) = echo_reply::EchoReplyPacket::new(&packet[..num]) {
                    debug!("ECHO {:?} {:02x}", reply, reply.payload().iter().format(" "));
                    match self.sources.get(&reply.get_identifier()) {
                        Some(_) => {
                            let now: u128 = self.start_instant.elapsed().as_nanos();
                            let seq = reply.get_sequence_number();
                            let ttl = 0;//reply.get_hop_limit();
                            s.send(UniPacket::RecvPacket { 
                                seq,
                                ident: reply.get_identifier(),
                                t: now,
                                ttl,
                                size: num
                            }).unwrap();
                        }
                        _ => {}
                    }
                }

            } else {
                return;
            }
        }

        if let Some(ipv6_packet) = Ipv6Packet::new(&packet[..num]) {
            debug!("IPV6-payload {:02x}", ipv6_packet.payload().iter().format(" "));
            debug!("IPV6 {:?}", ipv6_packet);
            if self.addrs.contains(&ipv6_packet.get_source().into()) {
                if let Some(icmpv6) = Icmpv6Packet::new(&packet[..num]) {
                    if icmpv6.get_icmpv6_type() == Icmpv6Type(129) {
                    } else {
                        return;
                    }
                }

                if let Some(reply) = echo_reply::EchoReplyPacket::new(&packet[..num]) {
                    debug!("ECHO {:?} {:02x}", reply, reply.payload().iter().format(" "));
                }
            } else {
                return;
            }
        }

    }

    pub fn handle_icmpv4(&self, packet: &[u8], num: usize, s: &Sender<UniPacket>) {
        if let Some(ipv4_packet) = Ipv4Packet::new(&packet[..num]) {
            if let Some(reply) = echo_reply::EchoReplyPacket::new(ipv4_packet.payload()) { //&packet[..num]) {
                match self.sources.get(&reply.get_identifier()) {
                    Some(_) => {
                        let size = ipv4_packet.payload().len();
                        let seq = reply.get_sequence_number();
                        let ttl = ipv4_packet.get_ttl();
                        let now: u128 = self.start_instant.elapsed().as_nanos();
                        s.send(UniPacket::RecvPacket { 
                            seq,
                            ident: reply.get_identifier(),
                            t: now,
                            ttl,
                            size
                        }).unwrap();
                    }
                    None => {}
                }
            }
            }
        }

        pub fn poll(&self, s: &Sender<UniPacket>) -> Result<(), Box<dyn Error>> {

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
                                    Ok((num, addr)) => {
                                        debug!("Addr {:?}", addr);
                                        self.handle_icmpv6(&packet, num, &s);
                                    }
                                    Err(_) => {
                                        break;
                                    }
                                }
                            }
                        }
                        PING => {
                            loop {
                                let mut packet = [0u8;2048]; 
                                match self.ping.recv(&mut packet) {
                                    Ok((num, addr)) => {
                                        debug!("Addr {:?}", addr);
                                        self.handle_icmpv4(&packet, num, &s);
                                    },
                                    Err(_) => {
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
                        sock_addr
                    };
                    result.sources.insert(site.ident.clone(), site.host.to_string());
                    result.output.push(site.clone());
                    result.ping_output.push(site);
                }
                SocketAddr::V6(_) if both || versions.contains(&SelectVersion::V6) => {
                    result.addrs.insert(x.clone());
                    let site = Site {
                        host: host.to_string(),
                        ident: process::id() as u16 + i as u16,
                        sock_addr
                    };
                    result.sources.insert(site.ident.clone(), site.host.to_string());
                    result.output.push(site.clone());
                    result.ping_output.push(site);
                }
                // default skip
                _ => {}
            }
        });
        Ok(result)
    }

