use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{header::References, Mailbox, MessageBuilder},
    transport::smtp::authentication::{Credentials, Mechanism},
};
use crate::error::EddieError;
use crate::services::logger;

pub struct SmtpMessage {
    pub from: String,
    pub from_name: Option<String>,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    pub references: Vec<String>,
}

/// Send an email via SMTP and return the raw RFC 5322 message bytes
/// (for IMAP APPEND to Sent folder).
pub async fn send_message(
    smtp_host: &str,
    smtp_port: u16,
    smtp_tls: bool,
    username: &str,
    password: &str,
    message: &SmtpMessage,
) -> Result<Vec<u8>, EddieError> {
    let from_mailbox: Mailbox = if let Some(ref name) = message.from_name {
        format!("{} <{}>", name, message.from)
            .parse()
            .map_err(|e| EddieError::InvalidInput(format!("Invalid from address: {}", e)))?
    } else {
        message.from.parse()
            .map_err(|e| EddieError::InvalidInput(format!("Invalid from address: {}", e)))?
    };

    let mut builder: MessageBuilder = lettre::Message::builder()
        .from(from_mailbox)
        .subject(&message.subject);

    for to_addr in &message.to {
        let mailbox: Mailbox = to_addr.parse()
            .map_err(|e| EddieError::InvalidInput(format!("Invalid to address '{}': {}", to_addr, e)))?;
        builder = builder.to(mailbox);
    }

    for cc_addr in &message.cc {
        let mailbox: Mailbox = cc_addr.parse()
            .map_err(|e| EddieError::InvalidInput(format!("Invalid cc address '{}': {}", cc_addr, e)))?;
        builder = builder.cc(mailbox);
    }

    if let Some(ref reply_to) = message.in_reply_to {
        builder = builder.in_reply_to(reply_to.clone());
    }

    if !message.references.is_empty() {
        let refs_str = message.references.join(" ");
        builder = builder.header(References::from(refs_str));
    }

    let email = builder
        .body(message.body.clone())
        .map_err(|e| EddieError::Backend(format!("Failed to build email: {}", e)))?;

    let creds = Credentials::new(username.to_string(), password.to_string());

    // Save the raw bytes before sending (for IMAP APPEND)
    let raw_message = email.formatted();

    // Allow PLAIN, LOGIN, and XOAUTH2 mechanisms — many servers only advertise
    // PLAIN/LOGIN after TLS upgrade, so we list them explicitly.
    let mechanisms = &[Mechanism::Plain, Mechanism::Login, Mechanism::Xoauth2];

    let transport = if smtp_tls && smtp_port == 465 {
        // Implicit TLS (port 465)
        AsyncSmtpTransport::<Tokio1Executor>::relay(smtp_host)
            .map_err(|e| EddieError::Backend(format!("SMTP relay failed: {}", e)))?
            .port(smtp_port)
            .credentials(creds)
            .authentication(mechanisms.to_vec())
            .build()
    } else if smtp_tls {
        // STARTTLS (port 587 typically)
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(smtp_host)
            .map_err(|e| EddieError::Backend(format!("SMTP STARTTLS relay failed: {}", e)))?
            .port(smtp_port)
            .credentials(creds)
            .authentication(mechanisms.to_vec())
            .build()
    } else {
        // No TLS — use builder_dangerous and allow LOGIN for servers that support it
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(smtp_host)
            .port(smtp_port)
            .credentials(creds)
            .authentication(mechanisms.to_vec())
            .build()
    };

    transport.send(email).await
        .map_err(|e| EddieError::Backend(format!("SMTP send failed: {}", e)))?;

    logger::debug(&format!("Email sent via SMTP to {:?}", message.to));

    Ok(raw_message)
}
