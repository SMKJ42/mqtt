use std::{
    env,
    time::{Duration, Instant},
};

use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    qos::QosLevel,
    topics::TopicFilter,
    v3::{ConnectPacket, MqttPacket, PublishPacket, SubscribePacket},
};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);

    let packet = ConnectPacket::new(false, 100, String::from("test_id"), None, None, None);
    client.connect(packet).await.unwrap();

    let topics = args
        .iter()
        .map(|x| (TopicFilter::from_str(&x).unwrap(), QosLevel::ExactlyOnce))
        .collect();

    let packet = SubscribePacket::new(client.next_packet_id().unwrap(), topics);
    client.sub(packet).await.unwrap();

    loop {
        if let Some(packet) = client.recv_packet().await.unwrap() {
            match packet {
                MqttPacket::Publish(packet) => {
                    println!(
                        "id: {:?}, {:?}, payload: {:?}",
                        packet.id(),
                        packet.qos(),
                        packet.payload()
                    )
                }
                MqttPacket::SubAck(_) => {
                    break;
                }
                _ => {
                    println!("{:?}", packet);
                    panic!("Received a packet other than PUBACK or PUBLISH.")
                }
            }
        }
    }

    let mut packets: Vec<PublishPacket> = vec![];

    let timeout = Duration::from_secs(50);
    let mut last_ping = Instant::now();

    loop {
        // prevent successive calls to the stream.
        if Instant::now().duration_since(last_ping) > timeout {
            last_ping = Instant::now();
            client.ping().await.unwrap();
        }
        if let Some(packet) = client.recv_packet().await.unwrap() {
            match packet {
                MqttPacket::Publish(packet) => match packet.qos() {
                    QosLevel::AtMostOnce => {
                        println!(
                            "id: {:?}, {:?}, payload: {:?}",
                            packet.id(),
                            packet.qos(),
                            packet.payload()
                        );
                    }
                    QosLevel::AtLeastOnce => {
                        client.ack(packet.id().unwrap()).await.unwrap();
                        println!(
                            "id: {}, {:?}, payload: {:?}",
                            packet.id().unwrap(),
                            packet.qos(),
                            packet.payload()
                        );
                    }
                    QosLevel::ExactlyOnce => {
                        client.rec(packet.id().unwrap()).await.unwrap();
                        packets.push(packet);
                    }
                },
                MqttPacket::PubRel(packet) => {
                    client.comp(packet.id()).await.unwrap();
                    if let Some(idx) = packets.iter().position(|x| x.id().unwrap() == packet.id()) {
                        let pub_packet = packets[idx].clone();
                        println!(
                            "id: {}, {:?}, payload: {:?}",
                            pub_packet.id().unwrap(),
                            pub_packet.qos(),
                            pub_packet.payload()
                        );
                        packets.remove(idx);
                    }
                }
                _ => {}
            }
        }
    }
}
