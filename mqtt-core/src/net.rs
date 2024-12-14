use std::time::Duration;

use bytes::{Bytes, BytesMut};
use futures::FutureExt;
use tokio::{
    io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    time::sleep,
};

use crate::{
    err,
    v3::{decode_packet, FixedHeader, MqttPacket},
};

pub async fn read_packet<
    S: AsyncReadExt + AsyncRead + AsyncWriteExt + Unpin,
    E: From<io::Error> + From<err::DecodeError>,
>(
    stream: &mut S,
) -> Result<Option<MqttPacket>, E> {
    let timeout_dur = Duration::from_micros(100);
    let fut1 = sleep(timeout_dur);

    // This is a little hackey, however it does allow us to escape the event loop without having a direct access to a poll function.
    // Primarily useful for TLS stream types where the stream does not have a poll function.
    futures::select! {
        _ = fut1.fuse() => {
            return Ok(None);
        }
        out = unfused_read_packet(stream).fuse() => {
            return out;
        }
    }
}

pub async fn unfused_read_packet<
    S: AsyncReadExt + AsyncWrite + Unpin,
    E: From<io::Error> + From<err::DecodeError>,
>(
    stream: &mut S,
) -> Result<Option<MqttPacket>, E> {
    // read in the packet type and the encoded length.
    let mut header_buf = [0; 5];
    let f_header: FixedHeader;

    // read in packet type.
    header_buf[0] = stream.read_u8().await?;

    // read in encoded packet length.
    let mut i = 1;
    while i < 6 {
        let byte = stream.read_u8().await?;
        header_buf[i] = byte;
        if byte < 128 {
            break;
        }

        i += 1;
    }

    let mut header_buf = Bytes::copy_from_slice(&header_buf[0..i + 1]);
    f_header = FixedHeader::decode(&mut header_buf)?;

    let mut buf = BytesMut::new();
    buf.resize(f_header.rest_len(), 0);

    // extract the variable header and payload then parse the variable header.
    stream.read_exact(&mut buf).await?;
    match decode_packet(f_header, &mut buf.into()) {
        Ok(packet) => {
            return Ok(Some(packet));
        }
        Err(err) => {
            return Err(err.into());
        }
    }
}
