use async_imap::Session;
use async_imap::types::Mailbox;
use async_native_tls::TlsStream;
use tokio::net::TcpStream;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use tracing::{info, debug};

use crate::types::error::EddieError;

// This type alias saves us from writing this monster type everywhere.
// An IMAP session is generic over the stream type — in our case,
// it's TLS-encrypted TCP wrapped in a tokio compat layer.
pub type ImapSession = Session<TlsStream<Compat<TcpStream>>>;

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

pub async fn connect(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<ImapConnection, EddieError> {
    info!(host = %host, port = port, "Connecting to IMAP server");

    let tcp = TcpStream::connect((host, port))
        .await
        .map_err(|e| EddieError::Backend(format!("TCP connection failed: {}", e)))?;

    let tcp = tcp.compat();
    let tls = async_native_tls::TlsConnector::new();
    let tls_stream = tls
        .connect(host, tcp)
        .await
        .map_err(|e| EddieError::Backend(format!("TLS handshake failed: {}", e)))?;

    let client = async_imap::Client::new(tls_stream);

    let session = client
        .login(username, password)
        .await
        .map_err(|(e, _)| EddieError::Backend(format!("Login failed: {}", e)))?;

    // Detect Gmail by hostname — servers also advertise X-GM-EXT-1
    // in capabilities, but the host check is simpler and reliable
    let has_gmail_ext = host.contains("gmail.com")
        || host.contains("googlemail.com");

    Ok(ImapConnection {
        session,
        has_gmail_ext,
        read_only: true,
    })
}
