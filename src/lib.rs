use async_stream::try_stream;
use bytes::{BytesMut, BufMut};
use failure::Error;
use futures::{pin_mut, SinkExt, Stream, StreamExt};
use log::{debug, info};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::net::{TcpListener, TcpStream};
use tokio_util::codec::Framed;

mod data;
pub use data::*;

type Port = u16;

const BROADCAST_PORT: Port = 6000;
const LISTEN_ADDRESS: &'static str = "0.0.0.0:6500";

struct Ws1001Codec;

impl tokio_util::codec::Decoder for Ws1001Codec {
    type Item = Response;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        Ok(Some(Response::from_bytes(src.as_ref())?))
    }
}

impl tokio_util::codec::Encoder for Ws1001Codec {
    type Item = Command;
    type Error = Error;

    fn encode(&mut self, item: Self::Item, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let bytes = item.to_bytes()?;
        dst.put_slice(&bytes);

        Ok(())
    }
}


pub struct WeatherRecordStream {
    time_between_queries: Duration,
    socket: Framed<TcpStream, Ws1001Codec>,
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
        let socket = Framed::new(socket, Ws1001Codec{});

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
        if let Some(Ok(response)) = self.socket.next().await {
            match response {
                Response::WeatherRecord(record) => Ok(Some(record)),
                _ => todo!(),
            }
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
        self.socket.send(command).await?;

        Ok(())
    }
}
