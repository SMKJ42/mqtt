use std::time::{Duration, Instant};

use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    qos::QosLevel,
    topic::TopicFilter,
    v3::{ConnectPacket, MqttPacket, PublishPacket, SubscribePacket, UnsubscribePacket},
};
use tokio::{net::TcpStream, time::sleep};

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);

    let packet = ConnectPacket::new(false, 100, String::from("test_id"), None, None, None);
    client.connect(packet).await.unwrap();

    let topics = vec![(TopicFilter::from_str("test").unwrap(), QosLevel::AtMostOnce)];

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
                    panic!("Should have received a PUBACK.")
                }
            }
        }
    }

    let mut packets: Vec<PublishPacket> = vec![];

    let mut start = Some(Instant::now());
    let dur = Duration::from_secs(5);

    loop {
        if let Some(time) = start {
            if Instant::now().duration_since(time) > dur {
                let packet =
                    UnsubscribePacket::new(6821, vec![TopicFilter::from_str("test").unwrap()]);
                start = None;
                client.unsub(packet).await.unwrap();
                println!("Unsubscribing.")
            }
        }

        sleep(Duration::from_millis(10)).await;
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
                MqttPacket::UnsubAck(_) => {
                    println!("Unsubscribe Success. Shutting down...");
                    return;
                }
                _ => {}
            }
        }
    }
}
