use futures::executor::block_on;
use mqtt_core::{
    err::client::{self, ClientError},
    id::{IdGenType, IdGenerator},
    io::read_packet_with_timeout,
    v3::{
        ConnectPacket, DisconnectPacket, MqttPacket, PingReqPacket, PubAckPacket, PubCompPacket,
        PubRecPacket, PubRelPacket, PublishPacket, SubscribePacket, UnsubscribePacket,
    },
    Encode,
};

use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub struct AsyncClient<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    stream: BufReader<T>,
    id_gen: IdGenerator,
}

impl<T> AsyncClient<T>
where
    T: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: T) -> Self {
        return Self {
            stream: BufReader::new(stream),
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
            if let Some(packet) = read_packet_with_timeout::<_, ClientError>(&mut self.stream)
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
        return read_packet_with_timeout(&mut self.stream).await;
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
        let disconnect_packet = DisconnectPacket::new();
        self.stream.write_all(&disconnect_packet.encode()).await?;
        return Ok(());
    }
}

impl<T: AsyncRead + AsyncWrite + Unpin> Drop for AsyncClient<T> {
    fn drop(&mut self) {
        block_on(self.disconnect()).unwrap();
    }
}

// TODO: Create another client struct that handles the ack, rec, rel, comp packets and the associated stores.
// I don't want this to be the only client available because this provides an unnecisary abstraction for specific use cases
// where memory overhead is a concern and the two 64kb store would take up 128kb total...
