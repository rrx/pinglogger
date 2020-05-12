use std::io;
//use std::convert::TryInto;

use std::os::unix::io::{AsRawFd, RawFd};

use socket2::{Domain, Protocol, SockAddr, Socket as Socket2, Type};

use std::io::Write;

// Some tokens to allow us to identify which event is for which socket.

//const ECHO_REQUEST_BUFFER_SIZE: usize = ICMP_HEADER_SIZE + TOKEN_SIZE + 32;

#[derive(Debug)]
pub enum PacketError {
    InvalidSize,
    InvalidPacket,
}

pub struct IcmpV4;
pub struct IcmpV6;

pub trait Proto {
    const ECHO_REQUEST_TYPE: u8;
    const ECHO_REQUEST_CODE: u8;
    const ECHO_REPLY_TYPE: u8;
    const ECHO_REPLY_CODE: u8;
}

impl Proto for IcmpV4 {
    const ECHO_REQUEST_TYPE: u8 = 8;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 0;
    const ECHO_REPLY_CODE: u8 = 0;
}

impl Proto for IcmpV6 {
    const ECHO_REQUEST_TYPE: u8 = 128;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 129;
    const ECHO_REPLY_CODE: u8 = 0;
}

pub struct EchoRequest<'a> {
    pub ident: u16,
    pub seq_cnt: u16,
    pub payload: &'a [u8],
}

fn write_checksum(buffer: &mut [u8]) {
    let mut sum = 0u32;
    for word in buffer.chunks(2) {
        let mut part = u16::from(word[0]) << 8;
        if word.len() > 1 {
            part += u16::from(word[1]);
        }
        sum = sum.wrapping_add(u32::from(part));
    }

    while (sum >> 16) > 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }

    let sum = !sum as u16;

    buffer[2] = (sum >> 8) as u8;
    buffer[3] = (sum & 0xff) as u8;
}

impl<'a> EchoRequest<'a> {
    pub fn encode<P: Proto>(&self, buffer: &mut [u8]) -> Result<(), PacketError> {
        buffer[0] = P::ECHO_REQUEST_TYPE;
        buffer[1] = P::ECHO_REQUEST_CODE;

        buffer[4] = (self.ident >> 8) as u8;
        buffer[5] = self.ident as u8;
        buffer[6] = (self.seq_cnt >> 8) as u8;
        buffer[7] = self.seq_cnt as u8;

        if let Err(_) = (&mut buffer[8..]).write(self.payload) {
            return Err(PacketError::InvalidSize)
        }

        write_checksum(buffer);
        Ok(())
    }
}

pub struct Socket {
    pub socket: Socket2,
}

impl Socket {
    pub fn new(domain: Domain, type_: Type, protocol: Protocol) -> io::Result<Self> {
        let socket = Socket2::new(domain, type_, Some(protocol))?;
        socket.set_nonblocking(true)?;

        Ok(Self { socket: socket })
    }

    pub fn send_to(&self, buf: &[u8], target: &SockAddr) -> io::Result<usize> {
        self.socket.send_to(buf, target)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.socket.flush()
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<(usize, SockAddr)> {
        self.socket.recv_from(buf)
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> RawFd {
        self.socket.as_raw_fd()
    }
}
