
use std::str::FromStr;
use std::collections::BTreeMap;
use std::cmp;
use std::os::unix::io::AsRawFd;
use smoltcp::time::{Duration, Instant};
use smoltcp::phy::Device;
use smoltcp::phy::wait as phy_wait;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr,
Ipv6Address, Icmpv6Repr, Icmpv6Packet,
Ipv4Address, Icmpv4Repr, Icmpv4Packet};
use smoltcp::iface::{NeighborCache, EthernetInterfaceBuilder, Routes};
use smoltcp::socket::{SocketSet, IcmpSocket, IcmpSocketBuffer, IcmpPacketMetadata, IcmpEndpoint};
use std::collections::HashMap;

macro_rules! send_icmp_ping {
    ( $repr_type:ident, $packet_type:ident, $ident:expr, $seq_no:expr,
      $echo_payload:expr, $socket:expr, $remote_addr:expr ) => {{
        let icmp_repr = $repr_type::EchoRequest {
            ident: $ident,
            seq_no: $seq_no,
            data: &$echo_payload,
        };

        let icmp_payload = $socket
            .send(icmp_repr.buffer_len(), $remote_addr)
            .unwrap();

          let icmp_packet = $packet_type::new_unchecked(icmp_payload);
          (icmp_repr, icmp_packet)
    }}
}

macro_rules! get_icmp_pong {
    ( $repr_type:ident, $repr:expr, $payload:expr, $waiting_queue:expr, $remote_addr:expr,
      $timestamp:expr, $received:expr ) => {{
        if let $repr_type::EchoReply { seq_no, data, .. } = $repr {
            if let Some(_) = $waiting_queue.get(&seq_no) {
                let packet_timestamp_ms = NetworkEndian::read_i64(data);
                println!("{} bytes from {}: icmp_seq={}, time={}ms",
                    data.len(), $remote_addr, seq_no,
                    $timestamp.total_millis() - packet_timestamp_ms);
                $waiting_queue.remove(&seq_no);
                $received += 1;
            }
        }
    }}
}
fn main() {
    //thread::spawn(move || {
    //let localhost_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    //let ping = icmp::IcmpSocket::connect(localhost_v4).unwrap();
    //let localhost_v6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    //let ping_v6 = icmp::IcmpSocket::connect(localhost_v6).unwrap();
    //let mut b = [0u8; 1024];

    //loop {
    //let num = ping_v6.recv(&mut b).unwrap();
    //println!("{:?} {:02x}", num, b[..num].iter().format(" "));
    //targets2.handle_icmpv6(&b, num, &s2);
    //}
    //});

    use pnet::datalink::{self, NetworkInterface};
    use smoltcp::phy::wait as phy_wait;
    use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr,
    Ipv6Address, Icmpv6Repr, Icmpv6Packet,
    Ipv4Address, Icmpv4Repr, Icmpv4Packet};
    use smoltcp::iface::{NeighborCache, EthernetInterfaceBuilder, Routes};
    use smoltcp::socket::{SocketSet, IcmpSocket, IcmpSocketBuffer, IcmpPacketMetadata, IcmpEndpoint};

    let icmp_rx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
    let icmp_tx_buffer = IcmpSocketBuffer::new(vec![IcmpPacketMetadata::EMPTY], vec![0; 256]);
    let mut icmp_socket = IcmpSocket::new(icmp_rx_buffer, icmp_tx_buffer);
    //let mut sockets = SocketSet::new(vec![]);
    //let icmp_handle = sockets.add(icmp_socket);

    let mut ping_buffer = [0u8; 2048];

    // Bind to ICMP messages with the ICMP identifier 0x1234
    icmp_socket.bind(IcmpEndpoint::Ident(0)).unwrap();

    //let mut socket = sockets.get::<IcmpSocket>(icmp_handle);
    let ident = 0;
    let mut seq_no = 0;
    let mut echo_payload = [0xffu8; 40];

    loop {
        let request = pinglogger::icmp::EchoRequest {
            ident,
            seq_cnt: seq_no,
            payload: &[]
        };
        seq_no += 1;

        request.encode::<pinglogger::icmp::IcmpV6>(&mut ping_buffer).unwrap();
        targets2.output.iter().for_each(|site| {
            let remote_addr: IpAddress = site.sock_addr.ip().into();
            if icmp_socket.can_send() {
                match remote_addr {
                    IpAddress::Ipv4(_) => {
                        let (icmp_repr, mut icmp_packet) = send_icmp_ping!(
                            Icmpv4Repr, Icmpv4Packet, ident, seq_no,
                            echo_payload, icmp_socket, remote_addr);
                        icmp_repr.emit(&mut icmp_packet, &device_caps.checksum);
                    },
                    IpAddress::Ipv6(_) => {
                        let (icmp_repr, mut icmp_packet) = send_icmp_ping!(
                            Icmpv6Repr, Icmpv6Packet, ident, seq_no,
                            echo_payload, icmp_socket, remote_addr);
                        icmp_repr.emit(&src_ipv6, &remote_addr,
                            &mut icmp_packet, &device_caps.checksum);
                    },
                    _ => unimplemented!()
                }
                println!("Send {}", remote_addr);
                icmp_socket.send_slice(&ping_buffer[..64], a).unwrap();
            }});
        println!("wait");
        if icmp_socket.can_recv() {
            let (payload, _) = icmp_socket.recv().unwrap();
            println!("{:?}", payload);
        } else {
            sleep(Duration::from_millis(1000));
        }
    }


}
