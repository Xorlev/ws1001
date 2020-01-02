use async_stream::try_stream;
use bytes::Bytes;
use data::*;
use failure::Error;
use futures::{pin_mut, Stream, StreamExt, SinkExt};
use log::{debug, info};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{BytesCodec, Framed};

mod data;

type Port = u16;

const BROADCAST_PORT: Port = 6000;
const LISTEN_ADDRESS: &'static str = "0.0.0.0:6500";

// https://www.mail-archive.com/weewx-user@googlegroups.com/msg10441/HP1000-gs.py

// This is an encoded c-struct reverse-engineered from wireshark.
// Offset  Value           Structure       Comment
// 0x00    PC2000          8 byte string   Identifies the calling station
// 0x08    SEARCH          8 byte string   Command
// 0x10    nulls           24 null bytes
// TODO: construct these using Command.
const SEARCH_MESSAGE: &[u8] = b"PC2000\x00\x00SEARCH\x00\x00\x00\xcd\xfd\x94,\xfb\xe3\x0b\x0c\xfb\xe3\x0bP\xab\xa5w\x00\x00\x00\x00\x00\xdd\xbfw";
const QUERY_MESSAGE: &[u8] = b"PC2000\x00\x00READ\x00\x00\x00\x00NOWRECORD\x00\x00\x00\x00\x00\x00\x00\xb8\x01\x00\x00\x00\x00\x00\x00";

struct WeatherRecordStream {
    time_between_queries: Duration,
    socket: Framed<TcpStream, BytesCodec>,
}

impl WeatherRecordStream {
    pub async fn new(time_between_queries: Duration) -> Result<WeatherRecordStream, Error> {
        // Setup listener.
        let mut listener = TcpListener::bind(LISTEN_ADDRESS).await?;
        let socket = async move {
            loop {
                info!("Waiting for requests on {}", LISTEN_ADDRESS);
                let (socket, addr) = listener.accept().await?;
                info!("Incoming request from {}", addr);
                return Ok(socket);
            }
        };
        WeatherRecordStream::broadcast_search().await?;
        let socket: Result<TcpStream, Error> = socket.await;
        let socket = socket?;
        let socket = Framed::new(socket, BytesCodec::new());

        Ok(WeatherRecordStream {
            time_between_queries,
            socket,
        })
    }

    pub fn start(mut self) -> impl Stream<Item = Result<Option<WeatherRecord>, Error>> {
        let timer_stream = tokio::time::interval(self.time_between_queries);

        try_stream! {
                    pin_mut!(timer_stream);
                    while let Some(_) = timer_stream.next().await {
        //                let x = Box::pin(self.get_next_record());
                        let record = self.get_next_record().await?;
                        yield record
                    }
                }
    }

    async fn get_next_record(&mut self) -> Result<Option<WeatherRecord>, Error> {
        self.query().await?;
        if let Some(Ok(bytes)) = self.socket.next().await {
            let now_record = WeatherRecord::parse(&bytes)?;
            Ok(Some(now_record))
        } else {
            Ok(None)
        }
    }

    async fn broadcast_search() -> Result<(), Error> {
        let mut udp_socket = UdpSocket::bind(format!("0.0.0.0:{}", BROADCAST_PORT)).await?;
        udp_socket.set_broadcast(true)?;
        udp_socket.set_multicast_loop_v4(false)?;
        udp_socket
            .send_to(
                SEARCH_MESSAGE,
                format!("255.255.255.255:{}", BROADCAST_PORT),
            )
            .await?;

        debug!("Sent broadcast packet.");

        Ok(())
    }

    async fn query(&mut self) -> Result<(), Error> {
        debug!("Querying..");
        self.socket.send(Bytes::from(QUERY_MESSAGE)).await?;

        Ok(())
    }
}
#[tokio::main]
async fn main() -> Result<(), Error> {
    let stream = WeatherRecordStream::new(Duration::from_secs(10))
        .await?
        .start();
    pin_mut!(stream);
    while let Some(record) = stream.next().await {
        println!("{:#?}", record);
    }

    Ok(())
}
