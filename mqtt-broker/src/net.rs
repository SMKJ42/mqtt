use std::{
    future::{poll_fn, Future},
    task::Poll,
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
use tokio_rustls::server::TlsStream;

pub trait MqttStream: AsyncRead + AsyncWrite + Unpin {
    fn mqtt_poll_peek<'a>(&self, buf: &'a mut [u8; 5]) -> impl Future<Output = bool>;
}

impl MqttStream for TcpStream {
    fn mqtt_poll_peek<'a>(&self, buf: &'a mut [u8; 5]) -> impl Future<Output = bool> {
        let mut buf = ReadBuf::new(buf);

        // hacky solution to prevent blocking without going into wakers... temperary fix.
        return poll_fn(move |cx| {
            if self.poll_peek(cx, &mut buf).is_ready() {
                return Poll::Ready(true);
            } else {
                return Poll::Ready(false);
            }
        });
    }
}

impl MqttStream for TlsStream<TcpStream> {
    fn mqtt_poll_peek<'a>(&self, buf: &'a mut [u8; 5]) -> impl Future<Output = bool> {
        return self.get_ref().0.mqtt_poll_peek(buf);
    }
}
