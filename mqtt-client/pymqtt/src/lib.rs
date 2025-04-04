use bytes::Bytes;
use pyo3::{
    prelude::*,
    types::{PyBytes, PyString, PyStringMethods},
};

/// Formats the sum of two numbers as string.
#[pyfunction]
fn sum_as_string(a: usize, b: usize) -> PyResult<String> {
    Ok((a + b).to_string())
}

/// A Python module implemented in Rust.
#[pymodule]
fn pymqtt(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(sum_as_string, m)?)?;
    m.add_function(wrap_pyfunction!(mqtt_client, m)?)?;
    Ok(())
}

use mqtt_core::{
    err::client::{self, ClientError},
    id::{IdGenType, IdGenerator},
    io::read_packet_with_timeout,
    topic::TopicName,
    v4::{
        ConnectPacket, DisconnectPacket, MqttPacket, PingReqPacket, PubAckPacket, PubCompPacket,
        PubRecPacket, PubRelPacket, PublishPacket, SubscribePacket, UnsubscribePacket,
    },
    Encode,
};

use tokio::{
    io::{AsyncWriteExt, BufReader},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    runtime::Runtime,
    sync::mpsc::{self, Receiver, Sender},
};

const MAX_MESSAGES: usize = 128;
const POLL_EVENT_INTERVAL: u32 = 61;

#[pyfunction]
pub fn mqtt_client(py: Python<'_>, addr: String) -> MqttClient {
    pyo3::prepare_freethreaded_python();
    let mut builder = tokio::runtime::Builder::new_multi_thread();

    builder
        .enable_io()
        .event_interval(POLL_EVENT_INTERVAL)
        .enable_time()
        .max_blocking_threads(2);

    let rt = builder.build().unwrap();

    let (packet_tx, packet_rx) = mpsc::channel(MAX_MESSAGES);
    let (bytes_tx, bytes_rx) = mpsc::channel(MAX_MESSAGES);

    let (r, w) = connect(&rt, &addr);

    rt.spawn(reader_runtime(r, packet_tx.clone(), bytes_tx));
    rt.spawn(writer_runtime(w, packet_rx));

    return MqttClient::new(packet_tx.clone(), bytes_rx);
}

fn connect(rt: &Runtime, addr: &str) -> (OwnedReadHalf, OwnedWriteHalf) {
    let stream = rt.block_on(async { TcpStream::connect(addr).await.unwrap() });

    let (r, w) = stream.into_split();
    return (r, w);
}
// fn spawn_reader(r: OwnedReadHalf, packet_tx: Sender<MqttPacket>, bytes_tx: Sender<Vec<u8>>) {
//     tokio::spawn(reader_runtime(r, packet_tx, bytes_tx));
// }

// Reader is responsible for reading from the stream and forwarding packet payloads on to the client.
// It is also responsible for managing the messaging queue and forwarding messages that have not been
// acknowledge onto the writer_runtime
async fn reader_runtime(
    r: OwnedReadHalf,
    packet_tx: Sender<MqttPacket>,
    bytes_tx: Sender<Vec<u8>>,
) {
    let mut read_buf = BufReader::new(r);
    loop {
        if let Some(packet) = read_packet_with_timeout::<_, ClientError>(&mut read_buf)
            .await
            .unwrap()
        {
            match packet {
                MqttPacket::Connect(_) => {
                    unimplemented!(
                        "Client received a connect packet, this packet is not supported."
                    )
                }
                MqttPacket::ConnAck(_) => {
                    unimplemented!("Client received a Connack packet. While the client suports this packet type, it was not expected.")
                }
                MqttPacket::Subscribe(_) => {
                    unimplemented!(
                        "Client received a Subscribe packet, this packet is not supported."
                    )
                }
                MqttPacket::SubAck(_) => {
                    unimplemented!()
                }
                MqttPacket::Unsubscribe(_) => {
                    unimplemented!(
                        "Client received an Unsubscribe packet, this packet is not supported."
                    )
                }
                MqttPacket::UnsubAck(_) => {
                    unimplemented!()
                }
                MqttPacket::PingReq(_) => {
                    unimplemented!(
                        "Client received a PingReq packet, this packet is not supported."
                    )
                }
                MqttPacket::PingResp(_) => {
                    unimplemented!()
                }
                MqttPacket::Publish(_) => {
                    unimplemented!()
                }
                MqttPacket::PubAck(_) => {
                    unimplemented!()
                }
                MqttPacket::PubRec(_) => {
                    unimplemented!()
                }
                MqttPacket::PubRel(_) => {
                    unimplemented!()
                }
                MqttPacket::PubComp(_) => {
                    unimplemented!()
                }
                MqttPacket::Disconnect(_) => {
                    unimplemented!(
                        "Client received a Disconnect packet, this packet is not supported."
                    )
                }
            }
        }
    }
}

async fn writer_runtime(mut w: OwnedWriteHalf, mut rx: Receiver<MqttPacket>) {
    while let Some(packet) = rx.recv().await {
        // TODO: unwraps...
        w.write_all(&packet.encode().unwrap()).await.unwrap();
    }
}

#[pyclass]
pub struct MqttClient {
    c_rx: Receiver<Vec<u8>>,
    rt_tx: Sender<MqttPacket>,
    id_gen: IdGenerator,
}

impl MqttClient {
    fn new(rt_tx: Sender<MqttPacket>, c_rx: Receiver<Vec<u8>>) -> Self {
        return Self {
            rt_tx,
            c_rx,
            id_gen: IdGenerator::new(IdGenType::Client),
        };
    }
}

#[pymethods]
impl MqttClient {
    #[pyo3(signature = (topic, payload))]
    fn publish(&self, py: Python, topic: Bound<'_, PyString>, payload: Bound<'_, PyBytes>) {
        let topic = topic.to_str().unwrap();
        let topic = TopicName::from_str(topic).unwrap();

        let payload = payload.as_bytes().to_owned();
        let packet = PublishPacket::new(&topic, Bytes::from(payload));
        self.rt_tx
            .blocking_send(MqttPacket::Publish(packet))
            .unwrap();
    }
}

// impl MqttStream {
//     pub async fn disconnect(&mut self) -> Result<(), ClientError> {
//         let disconnect_packet = DisconnectPacket::new();
//         self.w_stream.write_all(&disconnect_packet.encode()).await?;
//         return Ok(());
//     }
// }

// impl Drop for MqttClient {
//     fn drop(&mut self) {
//         let _ = block_on(self.disconnect());
//     }
// }

// TODO: Create another client struct that handles the ack, rec, rel, comp packets and the associated stores.
// I don't want this to be the only client available because this provides an unnecisary abstraction for specific use cases
// where memory overhead is a concern and the two 64kb store would take up 128kb total...
