use std::time::Duration;

use mqtt_client::r#async::AsyncClient;
use mqtt_core::v3::{ConnectPacket, MqttPacket};
use tokio::{net::TcpStream, time::Instant};

const MAXPING: u32 = 1000000;
#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:1883").await.unwrap();
    let mut client = AsyncClient::new(stream);

    let packet = ConnectPacket::new(false, 10, String::from("test_id"), None, None, None);
    client.connect(packet).await.unwrap();

    let mut dur = Duration::from_secs(0);
    let start = Instant::now();

    for _ in 0..MAXPING {
        client.ping().await.unwrap();
        let curr_start = Instant::now();
        loop {
            if let Some(packet) = client.recv_packet().await.unwrap() {
                match packet {
                    MqttPacket::PingResp(_) => {
                        dur += Instant::now().duration_since(curr_start);
                        break;
                    }
                    _ => {
                        panic!();
                    }
                }
            }
        }
    }

    println!(
        "Average ping response time: {} μs",
        dur.as_micros() / MAXPING as u128
    );
    println!(
        "Total sent: {}, Total Time: {} ms",
        MAXPING,
        Instant::now().duration_since(start).as_millis()
    );
}
