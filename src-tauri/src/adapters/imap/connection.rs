use async_imap::Session;
use async_imap::types::Mailbox;
use async_native_tls::TlsStream;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use tracing::info;
use futures::io::{AsyncRead, AsyncWrite};
use std::pin::Pin;
use std::task::{Context, Poll};

use crate::error::EddieError;

/// A stream that can be either TLS-encrypted or plain TCP.
#[derive(Debug)]
pub enum MaybeTlsStream {
    Tls(TlsStream<Compat<TcpStream>>),
    Plain(Compat<TcpStream>),
}

impl AsyncRead for MaybeTlsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_read(cx, buf),
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTlsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_write(cx, buf),
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_flush(cx),
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStream::Tls(s) => Pin::new(s).poll_close(cx),
            MaybeTlsStream::Plain(s) => Pin::new(s).poll_close(cx),
        }
    }
}

pub type ImapSession = Session<MaybeTlsStream>;

pub struct ImapConnection {
    pub session: ImapSession,
    pub has_gmail_ext: bool,
    pub read_only: bool,
}

impl ImapConnection {
    pub async fn select_folder(&mut self, folder: &str) -> Result<Mailbox, EddieError> {
        let mailbox = if self.read_only {
            self.session.examine(folder).await
        } else {
            self.session.select(folder).await
        }
        .map_err(|e| EddieError::Backend(format!("SELECT failed: {}", e)))?;

        Ok(mailbox)
    }
}

pub async fn connect_with_tls(
    host: &str,
    port: u16,
    use_tls: bool,
    username: &str,
    password: &str,
) -> Result<ImapConnection, EddieError> {
    info!(host = %host, port = port, tls = use_tls, "Connecting to IMAP server");

    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| EddieError::Backend(format!("TCP connection failed: {}", e)))?;

    let stream = if use_tls {
        let tcp = tcp.compat();
        let tls = async_native_tls::TlsConnector::new();
        let tls_stream = tls
            .connect(host, tcp)
            .await
            .map_err(|e| EddieError::Backend(format!("TLS handshake failed: {}", e)))?;
        MaybeTlsStream::Tls(tls_stream)
    } else {
        MaybeTlsStream::Plain(tcp.compat())
    };

    let client = async_imap::Client::new(stream);

    let session = client
        .login(username, password)
        .await
        .map_err(|(e, _)| EddieError::Backend(format!("Login failed: {}", e)))?;

    let has_gmail_ext = host.contains("gmail.com")
        || host.contains("googlemail.com");

    Ok(ImapConnection {
        session,
        has_gmail_ext,
        read_only: true,
    })
}
