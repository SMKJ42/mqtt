use std::time::Duration;

use bytes::Bytes;
use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    topic::TopicName,
    v3::{ConnectPacket, PublishPacket, Will},
};
use tokio::{net::TcpStream, time::sleep};

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);
    let topic_name = TopicName::from_str("qos0").unwrap();

    let will = Will::new(
        topic_name.clone(),
        "RETAIN".to_string(),
        mqtt_core::qos::QosLevel::AtMostOnce,
        true,
    );

    let packet = ConnectPacket::new(false, 10, String::from("pub_id_1"), Some(will), None, None);
    client.connect(packet).await.unwrap();

    let mut idx = 0;

    loop {
        sleep(Duration::from_millis(10)).await;
        let packet = PublishPacket::new(
            &topic_name,
            Bytes::copy_from_slice(&format!("TEST QOS 0, idx: {idx}").as_bytes()),
        );
        client.publish(packet).await.unwrap();

        println!("idx: {idx}");
        idx += 1;
    }
}
