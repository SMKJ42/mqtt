use tokio::time::Duration;

use futures::FutureExt;

use tokio::io::{AsyncBufRead, AsyncBufReadExt};
use tokio::time::sleep;

mod util;

pub(crate) use util::*;

use crate::{
    err,
    v4::{decode_mqtt_packet, MqttPacket},
};

pub async fn read_packet_with_timeout<S, E>(
    stream: &mut S,
    timeout_us: u64,
) -> Result<Option<MqttPacket>, E>
where
    S: AsyncBufRead + Unpin,
    E: From<err::DecodeError>,
{
    futures::select! {
        _ = sleep(Duration::from_micros(timeout_us)).fuse() => {
            return Ok(None);
        }
        packet = read_packet::<S, E>(stream).fuse() => {
            return packet
        }
    }
}

pub async fn read_packet<S, E>(stream: &mut S) -> Result<Option<MqttPacket>, E>
where
    S: AsyncBufRead + Unpin,
    E: From<err::DecodeError>,
{
    let packet_tuple = decode_mqtt_packet::<S, E>(stream).await?;
    if let Some((f_header, packet)) = packet_tuple {
        stream.consume(f_header.header_len() + f_header.rest_len());
        return Ok(Some(packet));
    } else {
        return Ok(None);
    }
}
