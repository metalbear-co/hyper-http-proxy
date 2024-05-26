use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use hyper::rt::{Read, ReadBufCursor, Write};
use hyper_util::client::legacy::connect::{Connected, Connection};

#[cfg(feature = "__tls")]
use hyper_util::rt::TokioIo;

#[cfg(feature = "__rustls")]
use tokio_rustls::client::TlsStream as RustlsStream;

#[cfg(all(not(feature = "__rustls"), feature = "native-tls"))]
use tokio_native_tls::TlsStream as TokioNativeTlsStream;

#[cfg(feature = "__rustls")]
pub type TlsStream<R> = TokioIo<RustlsStream<TokioIo<R>>>;

#[cfg(all(not(feature = "__rustls"), feature = "native-tls"))]
pub type TlsStream<R> = TokioIo<TokioNativeTlsStream<TokioIo<R>>>;

/// A Proxy Stream wrapper
pub enum ProxyStream<R> {
    NoProxy(R),
    Regular(R),
    #[cfg(feature = "__tls")]
    Secured(Box<TlsStream<R>>),
}

macro_rules! match_fn_pinned {
    ($self:expr, $fn:ident, $ctx:expr, $buf:expr) => {
        match $self.get_mut() {
            ProxyStream::NoProxy(s) => Pin::new(s).$fn($ctx, $buf),
            ProxyStream::Regular(s) => Pin::new(s).$fn($ctx, $buf),
            #[cfg(feature = "__tls")]
            ProxyStream::Secured(s) => Pin::new(s).$fn($ctx, $buf),
        }
    };

    ($self:expr, $fn:ident, $ctx:expr) => {
        match $self.get_mut() {
            ProxyStream::NoProxy(s) => Pin::new(s).$fn($ctx),
            ProxyStream::Regular(s) => Pin::new(s).$fn($ctx),
            #[cfg(feature = "__tls")]
            ProxyStream::Secured(s) => Pin::new(s).$fn($ctx),
        }
    };
}

impl<R: Read + Write + Unpin> Read for ProxyStream<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: ReadBufCursor<'_>,
    ) -> Poll<io::Result<()>> {
        match_fn_pinned!(self, poll_read, cx, buf)
    }
}

impl<R: Read + Write + Unpin> Write for ProxyStream<R> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match_fn_pinned!(self, poll_write, cx, buf)
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<Result<usize, io::Error>> {
        match_fn_pinned!(self, poll_write_vectored, cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            ProxyStream::NoProxy(s) => s.is_write_vectored(),
            ProxyStream::Regular(s) => s.is_write_vectored(),
            #[cfg(feature = "__tls")]
            ProxyStream::Secured(s) => s.is_write_vectored(),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match_fn_pinned!(self, poll_flush, cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match_fn_pinned!(self, poll_shutdown, cx)
    }
}

impl<R: Read + Write + Connection + Unpin> Connection for ProxyStream<R> {
    fn connected(&self) -> Connected {
        match self {
            ProxyStream::NoProxy(s) => s.connected(),

            ProxyStream::Regular(s) => s.connected().proxy(true),
            #[cfg(all(not(feature = "__rustls"), feature = "native-tls"))]
            ProxyStream::Secured(s) => s
                .inner()
                .get_ref()
                .get_ref()
                .get_ref()
                .inner()
                .connected()
                .proxy(true),

            #[cfg(feature = "__rustls")]
            ProxyStream::Secured(s) => s.inner().get_ref().0.inner().connected().proxy(true),
        }
    }
}
