use bytes::Bytes;
use mqtt_client::r#async::AsyncClient;
use mqtt_core::{
    topic::TopicName,
    v4::{ConnectPacket, PublishPacket},
};
use tokio::net::TcpStream;

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();

    let mut client = AsyncClient::new(stream);

    let packet = ConnectPacket::new(false, 10, String::from("test_id"), None, None, None);
    client.connect(packet).await.unwrap();

    let mut packet = PublishPacket::new(&TopicName::from_str("test").unwrap(), Bytes::new());
    packet.set_retain(true);

    client.publish(packet).await.unwrap();

    client.disconnect().await.unwrap();
}
