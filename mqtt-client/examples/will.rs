use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    topics::TopicName,
    v3::{ConnectPacket, Will},
};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);

    let will = Will::new(
        TopicName::from_str("test").unwrap(),
        "RETAIN".to_string(),
        mqtt_core::qos::QosLevel::AtMostOnce,
        true,
    );

    let packet = ConnectPacket::new(false, 10, String::from("pub_id_1"), Some(will), None, None);
    client.connect(packet).await.unwrap();

    // check other clients to see if the will is published / retained.
    panic!();
}
