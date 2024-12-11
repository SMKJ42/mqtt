use futures::executor::block_on;
use mqtt_core::net::read_packet;
use mqtt_core::v3::{
    ConnectPacket, DisconnectPacket, MqttPacket, PingReqPacket, PubAckPacket, PubCompPacket,
    PubRecPacket, PubRelPacket, PublishPacket, SubscribePacket, UnsubscribePacket,
};
use mqtt_core::{
    err::client::{self, ClientError},
    id::{IdGenType, IdGenerator},
};

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::{io::AsyncWriteExt, net::TcpStream};
use tokio_rustls::client::TlsStream;

pub trait Disconnect {
    #[allow(async_fn_in_trait)]
    /// Use this function when a client is intending to shutdown.
    async fn disconnect(&mut self) -> Result<(), ClientError>;
}

impl Disconnect for TcpStream {
    async fn disconnect(&mut self) -> Result<(), ClientError> {
        let disconnect_packet = DisconnectPacket::new();
        self.write_all(&disconnect_packet.encode()).await?;
        return Ok(());
    }
}

impl Disconnect for TlsStream<TcpStream> {
    async fn disconnect(&mut self) -> Result<(), ClientError> {
        let disconnect_packet = DisconnectPacket::new();
        self.write_all(&disconnect_packet.encode()).await?;
        return Ok(());
    }
}

pub struct AsyncClient<T>
where
    T: AsyncRead + AsyncWriteExt + Unpin + Disconnect,
{
    stream: T,
    id_gen: IdGenerator,
}

impl<T> AsyncClient<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Disconnect,
{
    pub fn new(stream: T) -> Self {
        return Self {
            stream,
            id_gen: IdGenerator::new(IdGenType::Client),
        };
    }

    pub fn next_packet_id(&mut self) -> Option<u16> {
        return self.id_gen.next_id();
    }

    pub async fn connect(&mut self, packet: ConnectPacket) -> Result<(), ClientError> {
        self.stream.write_all(&mut packet.encode().unwrap()).await?;
        self.stream.flush().await?;
        loop {
            if let Some(packet) = read_packet::<_, ClientError>(&mut self.stream)
                .await
                .unwrap()
            {
                match packet {
                    MqttPacket::ConnAck(..) => return Ok(()),
                    _ => {
                        println!("packet: {:?}", packet);
                        return Err(ClientError::new(
                            client::ErrorKind::ProtocolError,
                            String::from(
                                "First packet received from broker was not a CONNACK packet.",
                            ),
                        ));
                    }
                }
            }
        }
    }

    pub async fn recv_packet(&mut self) -> Result<Option<MqttPacket>, ClientError> {
        return read_packet(&mut self.stream).await;
    }

    pub async fn ping(&mut self) -> Result<(), ClientError> {
        self.stream
            .write_all(&PingReqPacket::new().encode())
            .await?;
        return Ok(());
    }

    pub async fn publish(&mut self, packet: PublishPacket) -> Result<(), ClientError> {
        self.stream.write_all(&packet.encode()?).await?;
        return Ok(());
    }

    pub async fn ack(&mut self, packet_id: u16) -> Result<(), ClientError> {
        self.stream
            .write_all(&PubAckPacket::new(packet_id).encode())
            .await?;
        return Ok(());
    }

    pub async fn rec(&mut self, packet_id: u16) -> Result<(), ClientError> {
        self.stream
            .write_all(&PubRecPacket::new(packet_id).encode())
            .await?;
        return Ok(());
    }

    pub async fn rel(&mut self, packet_id: u16) -> Result<(), ClientError> {
        self.stream
            .write_all(&PubRelPacket::new(packet_id).encode())
            .await?;
        return Ok(());
    }

    pub async fn comp(&mut self, packet_id: u16) -> Result<(), ClientError> {
        self.stream
            .write_all(&PubCompPacket::new(packet_id).encode())
            .await?;
        return Ok(());
    }

    pub async fn sub(&mut self, packet: SubscribePacket) -> Result<(), ClientError> {
        self.stream.write_all(&packet.encode()?).await?;
        return Ok(());
    }

    pub async fn unsub(&mut self, packet: UnsubscribePacket) -> Result<(), ClientError> {
        self.stream.write_all(&packet.encode()?).await?;
        return Ok(());
    }

    pub async fn disconnect(&mut self) -> Result<(), ClientError> {
        self.stream.disconnect().await
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin + Disconnect> Drop for AsyncClient<T> {
    fn drop(&mut self) {
        block_on(self.stream.disconnect()).unwrap();
    }
}
