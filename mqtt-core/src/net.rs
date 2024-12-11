use std::time::Duration;

use bytes::{Buf, Bytes, BytesMut};
use futures::FutureExt;
use tokio::{
    io::{self, AsyncReadExt, AsyncWrite},
    time::sleep,
};

use crate::{
    err::{self, DecodeError, DecodeErrorKind},
    v3::{decode_packet, FixedHeader, MqttPacket},
};

pub async fn read_packet<
    S: AsyncReadExt + AsyncWrite + Unpin,
    E: From<io::Error> + From<err::DecodeError>,
>(
    stream: &mut S,
) -> Result<Option<MqttPacket>, E> {
    let timeout_dur = Duration::from_millis(10);
    let fut1 = sleep(timeout_dur);

    futures::select! {
        _ = fut1.fuse() => {
            return Err(
                DecodeError::new(
                    DecodeErrorKind::Timeout,
                    format!("Could not decode packet within {} millis", timeout_dur.as_millis().to_string()
                )).into()
            )
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

    let mut buf = BytesMut::with_capacity(f_header.rest_len());

    // this prevents stalling on packets of size 0
    if f_header.rest_len() == 0 {
        return Ok(Some(decode_packet(f_header, &mut Bytes::new())?));
    }

    stream.read_buf(&mut buf).await?;
    match decode_packet(f_header, &mut buf.into()) {
        Ok(packet) => {
            return Ok(Some(packet));
        }
        Err(err) => {
            return Err(err.into());
        }
    }
}

pub async fn read_header<
    S: AsyncReadExt + AsyncWrite + Unpin,
    T: From<err::DecodeError> + From<io::Error>,
>(
    stream: &mut S,
    buf: [u8; 5],
) -> Result<(FixedHeader, Bytes), T> {
    // read the header bytes
    let mut buf = Bytes::from_iter(buf);

    let f_header = FixedHeader::decode(&mut buf)?;

    // Allocate a buf with the provided packet length.
    let mut buf_mut = BytesMut::with_capacity(f_header.header_len() + f_header.rest_len());

    // read the rest of the packet into the buf.
    stream.read_buf(&mut buf_mut).await?;

    // advance the header bytes.
    buf_mut.advance(f_header.header_len());

    return Ok((f_header, buf_mut.into()));
}

// pub trait MqttStream: AsyncRead + AsyncWrite + Unpin {
//     fn mqtt_poll_peek<'a>(&self, buf: &'a mut [u8; 5]) -> impl Future<Output = bool>;
// }

// impl MqttStream for TcpStream {
//     fn mqtt_poll_peek<'a>(&self, buf: &'a mut [u8; 5]) -> impl Future<Output = bool> {
//         let mut buf = ReadBuf::new(buf);

//         // hacky solution to prevent blocking without going into wakers... temperary fix.
//         return poll_fn(move |cx| {
//             if self.poll_peek(cx, &mut buf).is_ready() {
//                 return Poll::Ready(true);
//             } else {
//                 return Poll::Ready(false);
//             }
//         });
//     }
// }

// impl<IO> MqttStream for TlsStream<IO>
// where
//     IO: AsyncRead + AsyncWrite + Unpin,
// {
//     async fn mqtt_poll_peek<'a>(self, buf: &'a mut [u8; 5]) -> bool {
//         self.poll_read(buf);
//         todo!();
//     }
// }
