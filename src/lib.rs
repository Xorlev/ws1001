use async_stream::try_stream;
use bytes::Bytes;
use failure::Error;
use futures::{pin_mut, SinkExt, Stream, StreamExt};
use log::{debug, info};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::{BytesCodec, Framed};

mod data;
pub use data::*;

type Port = u16;

const BROADCAST_PORT: Port = 6000;
const LISTEN_ADDRESS: &'static str = "0.0.0.0:6500";

pub struct WeatherRecordStream {
    time_between_queries: Duration,
    socket: Framed<TcpStream, BytesCodec>,
}

impl WeatherRecordStream {
    pub async fn new(time_between_queries: Duration) -> Result<WeatherRecordStream, Error> {
        // Setup listener.
        let mut listener = TcpListener::bind(LISTEN_ADDRESS).await?;
        let socket = async move {
            info!("Waiting for requests on {}", LISTEN_ADDRESS);
            let (socket, addr) = listener.accept().await?;
            info!("Incoming request from {}", addr);
            return Ok(socket);
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
        let command = Command::search();
        let message = command.to_bytes()?;
        udp_socket.set_broadcast(true)?;
        udp_socket.set_multicast_loop_v4(false)?;
        udp_socket
            .send_to(
                &message,
                format!("255.255.255.255:{}", BROADCAST_PORT),
            )
            .await?;

        debug!("Sent broadcast packet.");

        Ok(())
    }

    async fn query(&mut self) -> Result<(), Error> {
        debug!("Querying..");
        let command = Command::query();
        let message = command.to_bytes()?;
        self.socket.send(Bytes::from(message)).await?;

        Ok(())
    }
}
