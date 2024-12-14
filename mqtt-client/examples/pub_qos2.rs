use std::time::Duration;

use bytes::Bytes;
use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    topic::TopicName,
    v3::{ConnectPacket, MqttPacket, PublishPacket, Will},
};
use tokio::{net::TcpStream, time::sleep};

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);
    let topic_name = TopicName::from_str("qos2").unwrap();

    let will = Will::new(
        topic_name.clone(),
        "RETAIN".to_string(),
        mqtt_core::qos::QosLevel::ExactlyOnce,
        true,
    );

    let packet = ConnectPacket::new(false, 10, String::from("pub_id2"), Some(will), None, None);
    client.connect(packet).await.unwrap();

    let mut should_send = true;

    let mut i = 0;

    loop {
        if should_send {
            sleep(Duration::from_millis(10)).await;
            let id = client.next_packet_id().unwrap();
            let mut packet = PublishPacket::new(
                &topic_name,
                Bytes::copy_from_slice(&format!("TEST QOS 2, Id: {id}, idx: {i}").as_bytes()),
            );
            packet.set_qos_exactlyonce(id);
            client.publish(packet).await.unwrap();
            println!("idx: {i}");
            i += 1;
        }

        should_send = !should_send;

        loop {
            if let Some(packet) = client.recv_packet().await.unwrap() {
                match packet {
                    MqttPacket::PubRec(packet) => {
                        client.rel(packet.id()).await.unwrap();
                        break;
                    }
                    MqttPacket::PubComp(_) => {
                        break;
                    }
                    _ => {
                        panic!("ERR: recieved packet other than PUBREC or PUBCOMP")
                    }
                }
            }
        }
    }
}
