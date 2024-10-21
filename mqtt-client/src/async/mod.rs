use std::{future::poll_fn, task::Poll};

use bytes::{Buf, Bytes, BytesMut};
use futures::executor::block_on;
use mqtt_core::v3::{
    ConnectPacket, DisconnectPacket, FixedHeader, MqttPacket, PingReqPacket, PubAckPacket,
    PubCompPacket, PubRecPacket, PubRelPacket, PublishPacket, SubscribePacket, UnsubscribePacket,
};
use mqtt_core::{
    err::client::{self, ClientError},
    id::{IdGenType, IdGenerator},
};
use tokio::task::block_in_place;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadBuf},
    net::TcpStream,
};

pub struct AsyncClient {
    stream: TcpStream,
    id_gen: IdGenerator,
}

impl AsyncClient {
    pub fn new(stream: TcpStream) -> Self {
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
        loop {
            if let Some(packet) = read_packet(&mut self.stream).await.unwrap() {
                match packet {
                    MqttPacket::ConnAck(..) => return Ok(()),
                    _ => {
                        return Err(ClientError::new(
                            client::ErrorKind::ProtocolError,
                            String::from(
                                "First packet received from broker was not a CONNACK packet.",
                            ),
                        ))
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
        let disconnect_packet = DisconnectPacket::new();
        self.stream.write_all(&disconnect_packet.encode()).await?;
        return Ok(());
    }
}

async fn read_packet(stream: &mut TcpStream) -> Result<Option<MqttPacket>, ClientError> {
    if let Some((f_header, buf)) = read_header(stream).await? {
        let packet = MqttPacket::decode(f_header, &mut buf.into())?;
        return Ok(Some(packet));
    } else {
        return Ok(None);
    }
}

async fn read_header(stream: &mut TcpStream) -> Result<Option<(FixedHeader, Bytes)>, ClientError> {
    let mut buf: [u8; 5] = [0; 5];
    let mut buf = ReadBuf::new(&mut buf);

    let ready_state = poll_fn(|cx| {
        if stream.poll_peek(cx, &mut buf).is_ready() {
            Poll::Ready(Some(()))
        } else {
            Poll::Ready(None)
        }
    })
    .await;

    if let Some(()) = ready_state {
        // read the header bytes
        let mut buf = Bytes::from_iter(buf.filled().iter().cloned());
        let f_header = FixedHeader::decode(&mut buf)?;

        // create a new space allocated for
        let mut buf_mut = BytesMut::with_capacity(f_header.header_len() + f_header.rest_len());

        // read the rest of the packet into the buf.
        stream.read_buf(&mut buf_mut).await?;

        // advance the header bytes.
        buf_mut.advance(f_header.header_len());

        return Ok(Some((f_header, buf_mut.into())));
    } else {
        return Ok(None);
    }
}

impl Drop for AsyncClient {
    fn drop(&mut self) {
        block_on(self.disconnect()).unwrap();
    }
}
