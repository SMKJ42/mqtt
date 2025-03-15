use std::{sync::Arc, time::Duration};

use tokio_rustls::{
    rustls::pki_types::{pem::PemObject, ServerName},
    TlsConnector,
};

use mqtt_client::r#async::AsyncClient;
use mqtt_core::v3::{ConnectPacket, MqttPacket};
use tokio::{net::TcpStream, time::Instant};
use tokio_rustls::rustls::{self, pki_types::CertificateDer};

const MAXPING: u32 = 10000;
#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:8883").await.unwrap();

    let domain = "test.mqtt.com";

    let mut root_cert_store = rustls::RootCertStore::empty();
    for cert in CertificateDer::pem_file_iter("../mqtt-broker/tls/cert.pem").unwrap() {
        root_cert_store.add(cert.unwrap()).unwrap();
    }

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();

    let connector = TlsConnector::from(Arc::new(config));
    let domain = ServerName::try_from(domain).unwrap().to_owned();

    let stream = connector.connect(domain, stream).await.unwrap();

    let mut client = AsyncClient::new(stream);

    let packet = ConnectPacket::new(false, 10, String::from("test_id"), None, None, None);
    client.connect(packet).await.unwrap();

    let mut dur = Duration::from_secs(0);
    let start = Instant::now();

    for _ in 0..MAXPING {
        client.ping().await.unwrap();
        let start = Instant::now();
        loop {
            if let Some(packet) = client.recv_packet().await.unwrap() {
                match packet {
                    MqttPacket::PingResp(_) => {
                        dur += Instant::now().duration_since(start);
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
