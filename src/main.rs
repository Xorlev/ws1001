#[macro_use] extern crate failure;
#[macro_use] extern crate failure_derive;

use data::*;
use failure::Error;
use std::env;
use std::io::{self, Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::net::UdpSocket;
use std::process;
use std::thread;
use std::sync::mpsc;

mod data;

type Port = u16;

const BROADCAST_PORT: Port = 6000;
const TCP_PORT: Port = 6500;

// https://www.mail-archive.com/weewx-user@googlegroups.com/msg10441/HP1000-gs.py

// This is an encoded c-struct reverse-engineered from wireshark.
// Offset  Value           Structure       Comment
// 0x00    PC2000          8 byte string   Identifies the calling station
// 0x08    SEARCH          8 byte string   Command
// 0x10    nulls           24 null bytes
const SEARCH_MESSAGE: &[u8] = b"PC2000\x00\x00SEARCH\x00\x00\x00\xcd\xfd\x94,\xfb\xe3\x0b\x0c\xfb\xe3\x0bP\xab\xa5w\x00\x00\x00\x00\x00\xdd\xbfw";
const QUERY_MESSAGE: &[u8] = b"PC2000\x00\x00READ\x00\x00\x00\x00NOWRECORD\x00\x00\x00\x00\x00\x00\x00\xb8\x01\x00\x00\x00\x00\x00\x00";

pub struct WeatherRecordListener {
    tx: mpsc::Sender<Option<NowRecord>>,
    rx: mpsc::Receiver<Option<NowRecord>>
}

impl WeatherRecordListener {
    pub fn new() -> WeatherRecordListener {
        let (tx, rx) = mpsc::channel();
        WeatherRecordListener {
            tx,
            rx
        }
    }

    pub fn start(&self) -> &WeatherRecordListener {
        let tx_clone = self.tx.clone();
        thread::spawn(move || {
            let result: Result<(), Error> = self.run_listener(tx_clone);

            if result.is_err() {
                panic!("WeatherRecordListener failed: {:?}", result.unwrap_err());
            }
        });

        self
    }

    fn run_listener(&self, tx: mpsc::Sender<Option<NowRecord>>) -> Result<(), Error> {
        let tcp_address = SocketAddr::new("0.0.0.0".parse().unwrap(), TCP_PORT);
        let tcp_socket = TcpListener::bind(tcp_address)?;

        self.broadcast_search()?;

        for stream in tcp_socket.incoming() {
            println!("Connection received...");
            let mut tcp_stream = stream?;
            tcp_stream.set_read_timeout(Some(std::time::Duration::from_secs(300)))?;

            loop {
                // Send QUERY message
                tcp_stream.write(QUERY_MESSAGE)?;

                let mut buf = [0; 512];
                match tcp_stream.read(&mut buf) {
                    Ok(received) => {
                        println!("received {} bytes {:?}", received, &buf[..received]);
                        let now_record = NowRecord::parse(&buf[..received])?;
                        tx.send(Some(now_record));
                    }
                    Err(e) => println!("recv function failed: {:?}", e),
                }

                std::thread::sleep_ms(10000);
            }
        }

        Ok(())
    }

    fn broadcast_search(&self) -> Result<(), Error> {
        let udp_address = SocketAddr::new("0.0.0.0".parse().unwrap(), BROADCAST_PORT);
        let udp_broadcast_address = SocketAddr::new("255.255.255.255".parse().unwrap(), BROADCAST_PORT);
        let udp_socket = UdpSocket::bind(udp_address)?;
        udp_socket.set_broadcast(true)?;
        udp_socket.set_multicast_loop_v4(false)?;
        udp_socket.send_to(SEARCH_MESSAGE, udp_broadcast_address)?;

        Ok(())
    }
}

impl Iterator for WeatherRecordListener {
    type Item = NowRecord;

    fn next(&mut self) -> Option<NowRecord> {
        let data = self.rx.recv();

        if data.is_ok() {
            data.unwrap()
        }
        else {
            None
        }
    }
}


fn main() {
    WeatherRecordListener::new()
        .start()
        .for_each(|record| println!("Record: {:?}", record));
}
