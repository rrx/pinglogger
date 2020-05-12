extern crate pnet;
use pnet::datalink::Channel::Ethernet;
use pnet::datalink::{self, NetworkInterface, interfaces};
use itertools::Itertools;
use pnet::packet::ethernet::{EtherTypes, EthernetPacket};

use pnet::packet::icmpv6::Icmpv6Packet;
use pnet::packet::ip::{IpNextHeaderProtocol, IpNextHeaderProtocols};
use pnet::packet::ipv6::Ipv6Packet;
use pnet::packet::Packet;

use std::net::IpAddr;

fn handle_icmpv6_packet(interface_name: &str, source: IpAddr, destination: IpAddr, packet: &[u8]) {
    let icmpv6_packet = Icmpv6Packet::new(packet);
    if let Some(icmpv6_packet) = icmpv6_packet {
        println!(
            "[{}]: ICMPv6 packet {} -> {} (type={:?})",
            interface_name,
            source,
            destination,
            icmpv6_packet.get_icmpv6_type()
        )
    } else {
        println!("[{}]: Malformed ICMPv6 Packet", interface_name);
    }
}

fn handle_transport_protocol(
    interface_name: &str,
    source: IpAddr,
    destination: IpAddr,
    protocol: IpNextHeaderProtocol,
    packet: &[u8],
) {
    match protocol {
        IpNextHeaderProtocols::Icmpv6 => {
            handle_icmpv6_packet(interface_name, source, destination, packet)
        }
        _ => {}

    }
}

fn handle_ipv6_packet(interface_name: &str, ethernet: &EthernetPacket) {
    let header = Ipv6Packet::new(ethernet.payload());
    if let Some(header) = header {
        handle_transport_protocol(
            interface_name,
            IpAddr::V6(header.get_source()),
            IpAddr::V6(header.get_destination()),
            header.get_next_header(),
            header.payload(),
        );
    } else {
        println!("[{}]: Malformed IPv6 Packet", interface_name);
    }
}

fn handle_ethernet_frame(interface: &NetworkInterface, ethernet: &EthernetPacket) {
    let interface_name = &interface.name[..];
    match ethernet.get_ethertype() {
        EtherTypes::Ipv6 => handle_ipv6_packet(interface_name, ethernet),
        _ => {}
    }
}

fn main() {
    let addrs = nix::ifaddrs::getifaddrs().unwrap();
    for ifaddr in addrs {
        match ifaddr.address {
            Some(address) => {
                println!("interface {} address {}",
                    ifaddr.interface_name, address);
            },
            None => {
                println!("interface {} with unsupported address family",
                    ifaddr.interface_name);
            }
        }
    }
    // Get a vector with all network interfaces found
    let all_interfaces = interfaces();

    // Search for the default interface - the one that is
    // up, not loopback and has an IP.
    let default_interface = all_interfaces
        .iter()
        .filter(|e| e.is_up() && !e.is_loopback() && e.ips.len() > 0)
        .next();

    match default_interface {
        Some(interface) => {
            println!("Found default interface with [{}].", interface.name);  
            let (_, mut rx) = match datalink::channel(&interface, Default::default()) {
                Ok(Ethernet(tx, rx)) => (tx, rx),
                Ok(_) => panic!("packetdump: unhandled channel type: {}"),
                Err(e) => panic!("packetdump: unable to create channel: {}", e),
            };
            loop {
                let result = rx.next();
                if result.is_err() {
                    break;
                }
                if let Ok(packet) = result {
                    //println!("{:02x}", packet.iter().format(" "));
                    handle_ethernet_frame(&interface, &EthernetPacket::new(packet).unwrap());
                }
            }
        }
        None => println!("Error while finding the default interface."),
    }
}
