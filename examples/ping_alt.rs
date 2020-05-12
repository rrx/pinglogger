extern crate pnet;

use log::*;
use std::error;
use std::process;
use pnet::packet::icmp::{IcmpTypes, echo_request, echo_reply, IcmpPacket};
use pnet::packet::icmpv6::{Icmpv6Types, MutableIcmpv6Packet};
use pnet::packet::ip::IpNextHeaderProtocols;
use pnet::packet::{Packet, PacketSize};
use pnet::transport::{
    ipv4_packet_iter,
    icmpv6_packet_iter,
    transport_channel, 
    TransportChannelType::{
        Layer3, 
        //Ipv6Layer3, 
        Layer4
    },
    TransportProtocol::{Ipv4, Ipv6},
    TransportSender
};
use pnet::util;
use pinglogger::cli;
use std::net::{IpAddr, SocketAddr};
use std::thread;
use std::thread::sleep;
use std::time::Duration;
use itertools::Itertools;

/// Checksum method for ipv6.
/// @param packet is a reference to a `MutableIcmpv6Packet`.
/// @return a `u16` representation of a checksum.
fn icmp_checksum(packet: &echo_request::MutableEchoRequestPacket) -> u16 {
    return util::checksum(packet.packet(),1);
}


/// Checksum method for ipv6.
/// @param packet is a reference to a `MutableIcmpv6Packet`.
/// @return a `u16` representation of a checksum.
fn icmpv6_checksum(packet: &MutableIcmpv6Packet) -> u16 {
    return util::checksum(packet.packet(),1);
}

const SEND_BUFFER_SIZE: usize = 64;
fn send(sender: &mut TransportSender, address: IpAddr, seq: u16, ident: u16) -> Result<(), Box<dyn error::Error>> {
    let mut vec : Vec<u8> = vec![0; SEND_BUFFER_SIZE];

    if address.is_ipv4() {
        let mut packet = echo_request::MutableEchoRequestPacket::new(&mut vec[..]).unwrap();
        packet.set_sequence_number(seq);
        packet.set_identifier(ident);
        packet.set_icmp_type(IcmpTypes::EchoRequest);
        packet.set_checksum(icmp_checksum(&packet));
        sender.send_to(packet, address)?;
    } else {
        let mut packet = MutableIcmpv6Packet::new(&mut vec[..]).unwrap();
        packet.set_icmpv6_type(Icmpv6Types::EchoRequest);
        let mut buffer = [0u8;4];
        buffer[0] = (ident >> 8) as u8;
        buffer[1] = ident as u8;
        buffer[2] = (seq >> 8) as u8;
        buffer[3] = seq as u8;
        packet.set_payload(&buffer);
        packet.set_checksum(icmpv6_checksum(&packet));
        sender.send_to(packet, address)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let (targets, _) = cli::init();

    // bail if we don't have anything
    if targets.output.len() == 0 {
        return Ok(());
    }

    let channel_size = 1024;
    let ident = process::id() as u16;

    let (_, mut rx_v4) = transport_channel(channel_size, Layer3(IpNextHeaderProtocols::Icmp))
        .map_err(|err| format!("Error opening the channel: {}", err))?;

    let (mut tx_v4_2, _) = transport_channel(channel_size, Layer4(Ipv4(IpNextHeaderProtocols::Icmp)))
        .map_err(|err| format!("Error opening the channel: {}", err))?;

    // Work around the fact that pnet does not have a ipv6 packet iterator
    // The best we can do right now is to get an icmpv6_packet iterator, which will not return
    // the hop limit.  It only gives us the icmpv6 fields.
    //let (mut tx_v6, mut rx_v6) = transport_channel(channel_size, Ipv6Layer3(IpNextHeaderProtocols::Icmpv6))
        //.map_err(|err| format!("Error opening the channel: {}", err))?;

    let (mut tx_v6_2, mut rx_v6_2) = transport_channel(channel_size, Layer4(Ipv6(IpNextHeaderProtocols::Icmpv6)))
        .map_err(|err| format!("Error opening the channel: {}", err))?;


    thread::spawn(move || {
        let mut seq_cnt = 1;
        loop {
            targets.output.iter().for_each(|site| {
                let result = match site.sock_addr {
                    SocketAddr::V4(_) => {
                        send(&mut tx_v4_2, site.sock_addr.ip(), seq_cnt, ident)
                    },
                    SocketAddr::V6(_) => {
                        send(&mut tx_v6_2, site.sock_addr.ip(), seq_cnt, ident)
                    }
                };
                if result.is_err() {
                    error!("Error: {:?}", result);
                }
            });
            seq_cnt += 1;
            sleep(Duration::from_secs(1));
        }

    });


    let v6_handler = thread::spawn(move || {
        let mut rx = icmpv6_packet_iter(&mut rx_v6_2);
        loop {
            match rx.next() {
                Ok((packet, addr)) => {
                    let echo_packet = echo_reply::EchoReplyPacket::new(&packet.packet()).unwrap();
                    if packet.get_icmpv6_type() == Icmpv6Types::EchoReply && echo_packet.get_identifier() == ident {
                        debug!("ICMPV6: {:?} {:?}", packet, addr.to_string());
                        debug!("X: {:02x}", packet.packet().iter().format(" "));
                        println!("{} bytes from {}", packet.packet_size(), addr);
                    }},
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    });

    let v4_handler = thread::spawn(move || {
        let mut rx = ipv4_packet_iter(&mut rx_v4);
        loop {
            match rx.next() {
                Ok((packet, addr)) => {
                    let icmp_packet = IcmpPacket::new(&packet.payload()).unwrap();

                    let echo_packet = echo_reply::EchoReplyPacket::new(&packet.payload()).unwrap();
                    if echo_packet.get_identifier() == ident {
                        debug!("ICMP: {:?}", icmp_packet);
                        debug!("ECHO: {:?}", echo_packet);
                        debug!("IPV4: {:?}", packet);
                        debug!("X: {:02x}", icmp_packet.packet().iter().format(" "));
                        println!("{} bytes from {}, ttl={}", packet.get_total_length(), addr, packet.get_ttl());
                    }
                },
                Err(e) => {
                    println!("Error: {:?}", e);
                }
            }
        }
    });

    v6_handler.join().unwrap();
    v4_handler.join().unwrap();
    Ok(())
}

