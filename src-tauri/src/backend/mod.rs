//! Email backend service using email-lib
//!
//! This module provides the core email functionality using IMAP for receiving
//! and SMTP for sending emails.

use std::process::Command;
use std::sync::Arc;

use email::account::config::{passwd::PasswordConfig, AccountConfig as EmailLibAccountConfig};
use email::backend::BackendBuilder;
use email::envelope::Id;
use email::flag::{add::AddFlags, remove::RemoveFlags, set::SetFlags, Flag, Flags};
use email::folder::{
    add::AddFolder, delete::DeleteFolder, expunge::ExpungeFolder, list::ListFolders, INBOX,
};
use email::imap::config::{ImapAuthConfig, ImapConfig as EmailImapConfig};
use email::message::{
    add::AddMessage, copy::CopyMessages, delete::DeleteMessages, get::GetMessages,
    peek::PeekMessages, r#move::MoveMessages, send::SendMessage,
};
use email::smtp::config::{SmtpAuthConfig, SmtpConfig as EmailSmtpConfig};
use email::tls::{Encryption, Tls};
use secret::Secret;
use std::path::PathBuf;
use tracing::{debug, info, warn};

use crate::config::{EmailAccountConfig, AuthConfig, ImapConfig, PasswordSource, SmtpConfig};
use crate::encryption::DeviceEncryption;
use crate::sync::db::{get_active_connection_config, get_connection_config, init_config_db};
use crate::types::error::EddieError;
use crate::types::{Attachment, Envelope, Folder, ChatMessage};

/// Result of sending a message - contains the UID, Message-ID header, and sent folder name
#[derive(Debug, Clone, serde::Serialize)]
pub struct SendMessageResult {
    pub uid: String,           // IMAP UID of the saved message
    pub message_id: String,    // Message-ID email header (for deduplication)
    pub sent_folder: String,
}

/// Backend service for email operations
pub struct EmailBackend {
    /// Account configuration from our config
    account_config: EmailAccountConfig,
    /// email-lib account configuration
    email_account_config: Arc<EmailLibAccountConfig>,
}

impl EmailBackend {
    /// Create a new email backend for an account
    pub async fn new(account_name: &str) -> Result<Self, EddieError> {
        // Initialize database if needed
        init_config_db()?;

        // Load account from database
        let db_config = get_connection_config(account_name)?
            .ok_or_else(|| EddieError::AccountNotFound(account_name.to_string()))?;

        // Deserialize IMAP and SMTP configs from JSON
        let imap_config = db_config
            .imap_config
            .and_then(|json| serde_json::from_str::<ImapConfig>(&json).ok());

        let smtp_config = db_config
            .smtp_config
            .and_then(|json| serde_json::from_str::<SmtpConfig>(&json).ok());

        let account_config = EmailAccountConfig {
            name: db_config.display_name.clone(),
            default: db_config.active,
            email: db_config.email.clone(),
            display_name: db_config.display_name.clone(),
            imap: imap_config,
            smtp: smtp_config,
        };

        // Build email-lib account config
        let email_account_config = Arc::new(EmailLibAccountConfig {
            name: db_config.display_name.clone().unwrap_or_else(|| account_name.to_string()),
            email: account_config.email.clone(),
            display_name: account_config.display_name.clone(),
            ..Default::default()
        });

        Ok(Self {
            account_config,
            email_account_config,
        })
    }

    /// Get the email address for this account
    pub fn get_email(&self) -> String {
        self.account_config.email.clone()
    }

    /// Create backend for default account
    pub async fn default() -> Result<Self, EddieError> {
        // Initialize database if needed
        init_config_db()?;

        // Load active account from database
        let db_config = get_active_connection_config()?
            .ok_or_else(|| EddieError::Config("No active account configured".to_string()))?;

        Self::new(&db_config.account_id).await
    }

    /// Get or resolve password from PasswordSource
    async fn resolve_password(source: &PasswordSource) -> Result<String, EddieError> {
        match source {
            PasswordSource::Raw(password) => Ok(password.clone()),
            PasswordSource::Command { command } => {
                info!("Executing password command");
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map_err(|e| {
                        EddieError::Config(format!("Failed to run password command: {}", e))
                    })?;

                if !output.status.success() {
                    return Err(EddieError::Config("Password command failed".to_string()));
                }

                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
        }
    }

    /// Build IMAP configuration for email-lib
    async fn build_imap_config(&self) -> Result<EmailImapConfig, EddieError> {
        let imap = self
            .account_config
            .imap
            .as_ref()
            .ok_or_else(|| EddieError::Config("No IMAP configuration".to_string()))?;

        info!("Building IMAP config for {} ({}:{})", self.account_config.email, imap.host, imap.port);
        let email = &self.account_config.email;
        let (login, auth) = Self::build_auth_config(&imap.auth, email).await.map_err(|e| {
            warn!("Failed to build auth config for {}: {}", email, e);
            e
        })?;

        let tls_config = Tls {
            cert: imap.tls_cert.as_ref().map(PathBuf::from),
            ..Default::default()
        };

        let encryption = if imap.tls {
            Some(Encryption::Tls(tls_config))
        } else {
            Some(Encryption::StartTls(tls_config))
        };

        Ok(EmailImapConfig {
            host: imap.host.clone(),
            port: imap.port,
            encryption,
            login,
            auth,
            ..Default::default()
        })
    }

    /// Build authentication configuration from AuthConfig
    async fn build_auth_config(
        auth_config: &AuthConfig,
        email: &str,
    ) -> Result<(String, ImapAuthConfig), EddieError> {
        match auth_config {
            AuthConfig::Password { user, password } => {
                debug!("Using password authentication for user: {}", user);
                let passwd = Self::resolve_password(password).await?;
                if passwd.is_empty() {
                    return Err(EddieError::Auth(format!(
                        "Empty password returned for user: {}",
                        user
                    )));
                }
                Ok((user.clone(), ImapAuthConfig::Password(PasswordConfig(Secret::new_raw(passwd)))))
            }
            AuthConfig::AppPassword { user } => {
                debug!("Using app password authentication for {}", email);

                // Get encrypted password from database
                init_config_db()?;
                let db_config = get_connection_config(email)?
                    .ok_or_else(|| {
                        EddieError::Auth(format!("No account configuration found for {}", email))
                    })?;

                let encrypted_password = db_config.encrypted_password
                    .ok_or_else(|| {
                        EddieError::Auth(format!(
                            "No password stored for {}. Please re-enter your password.",
                            email
                        ))
                    })?;

                // Decrypt password
                let encryption = DeviceEncryption::new()
                    .map_err(|e| EddieError::Auth(format!("Failed to initialize encryption: {}", e)))?;

                let password = encryption.decrypt(&encrypted_password)
                    .map_err(|e| {
                        warn!("Failed to decrypt password for {}: {}", email, e);
                        EddieError::Auth(format!(
                            "Failed to decrypt password for {}. You may need to re-enter your password.",
                            email
                        ))
                    })?;

                if password.is_empty() {
                    return Err(EddieError::Auth(format!(
                        "Empty password decrypted for {}. Please re-enter your password.",
                        email
                    )));
                }

                Ok((user.clone(), ImapAuthConfig::Password(PasswordConfig(Secret::new_raw(password)))))
            }
        }
    }

    /// Build SMTP configuration for email-lib
    async fn build_smtp_config(&self) -> Result<EmailSmtpConfig, EddieError> {
        let smtp = self
            .account_config
            .smtp
            .as_ref()
            .ok_or_else(|| EddieError::Config("No SMTP configuration".to_string()))?;

        let email = &self.account_config.email;
        let (login, auth) = Self::build_smtp_auth_config(&smtp.auth, email).await?;

        let tls_config = Tls {
            cert: smtp.tls_cert.as_ref().map(PathBuf::from),
            ..Default::default()
        };

        let encryption = if smtp.tls {
            Some(Encryption::Tls(tls_config))
        } else {
            Some(Encryption::StartTls(tls_config))
        };

        Ok(EmailSmtpConfig {
            host: smtp.host.clone(),
            port: smtp.port,
            encryption,
            login,
            auth,
            ..Default::default()
        })
    }

    /// Build SMTP authentication configuration from AuthConfig
    async fn build_smtp_auth_config(
        auth_config: &AuthConfig,
        email: &str,
    ) -> Result<(String, SmtpAuthConfig), EddieError> {
        match auth_config {
            AuthConfig::Password { user, password } => {
                let passwd = Self::resolve_password(password).await?;
                Ok((user.clone(), SmtpAuthConfig::Password(PasswordConfig(Secret::new_raw(passwd)))))
            }
            AuthConfig::AppPassword { user } => {
                // Get encrypted password from database (same as IMAP)
                init_config_db()?;
                let db_config = get_connection_config(email)?
                    .ok_or_else(|| {
                        EddieError::Auth(format!("No account configuration found for {}", email))
                    })?;

                let encrypted_password = db_config.encrypted_password
                    .ok_or_else(|| {
                        EddieError::Auth(format!(
                            "No password stored for {}. Please re-enter your password.",
                            email
                        ))
                    })?;

                // Decrypt password
                let encryption = DeviceEncryption::new()
                    .map_err(|e| EddieError::Auth(format!("Failed to initialize encryption: {}", e)))?;

                let password = encryption.decrypt(&encrypted_password)
                    .map_err(|e| {
                        EddieError::Auth(format!(
                            "Failed to decrypt password for {}: {}",
                            email, e
                        ))
                    })?;

                Ok((user.clone(), SmtpAuthConfig::Password(PasswordConfig(Secret::new_raw(password)))))
            }
        }
    }

    /// Find the Sent folder by checking common folder names
    pub async fn find_sent_folder(&self) -> Result<Option<String>, EddieError> {
        let folders = self.list_folders().await?;

        for folder in &folders {
            let name_lower = folder.name.to_lowercase();
            // Check for common sent folder names in various languages
            if name_lower == "sent"
                || name_lower == "sent mail"
                || name_lower == "sent messages"
                || name_lower.contains("sent")
                || name_lower.contains("envoy")      // French
                || name_lower.contains("gesendet")   // German
                || name_lower.contains("enviados")   // Spanish
                || name_lower.contains("inviati")
            // Italian
            {
                info!("Found sent folder: {}", folder.name);
                return Ok(Some(folder.name.clone()));
            }
        }

        info!("No sent folder found");
        Ok(None)
    }

    /// List all folders
    pub async fn list_folders(&self) -> Result<Vec<Folder>, EddieError> {
        let imap_config = self.build_imap_config().await?;

        info!(
            "Attempting IMAP connection for {}: {}:{} (TLS: {})",
            self.account_config.email,
            self.account_config.imap.as_ref().map(|i| i.host.as_str()).unwrap_or("unknown"),
            self.account_config.imap.as_ref().map(|i| i.port).unwrap_or(0),
            self.account_config.imap.as_ref().map(|i| i.tls).unwrap_or(false)
        );

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let start_time = std::time::Instant::now();
        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                let error_msg = format!(
                    "Failed to build IMAP backend after {:?}: {}. Config: host={}, port={}, encryption={}, error_chain: {:?}",
                    elapsed,
                    e,
                    self.account_config.imap.as_ref().map(|i| i.host.as_str()).unwrap_or("unknown"),
                    self.account_config.imap.as_ref().map(|i| i.port).unwrap_or(0),
                    self.account_config.imap.as_ref().map(|i| i.tls).unwrap_or(false),
                    e.source()
                );
                warn!("{}", error_msg);

                // Classify error type for better diagnostics
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("timeout") || error_str.contains("timed out") {
                    EddieError::Network(format!("Connection timeout: {}", error_msg))
                } else if error_str.contains("connection") || error_str.contains("refused") || error_str.contains("reset") {
                    EddieError::Network(format!("Connection error: {}", error_msg))
                } else if error_str.contains("auth") || error_str.contains("login") || error_str.contains("password") {
                    EddieError::Auth(format!("Authentication error: {}", error_msg))
                } else if error_str.contains("dns") || error_str.contains("resolve") {
                    EddieError::Network(format!("DNS resolution error: {}", error_msg))
                } else if error_str.contains("tls") || error_str.contains("ssl") || error_str.contains("certificate") {
                    EddieError::Network(format!("TLS/SSL error: {}", error_msg))
                } else {
                    EddieError::Backend(error_msg)
                }
            })?;

        let elapsed = start_time.elapsed();
        info!("IMAP connection established successfully in {:?}", elapsed);

        let folders = backend
            .list_folders()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(folders
            .into_iter()
            .map(|f| Folder {
                name: f.name.clone(),
                desc: if f.desc.is_empty() {
                    None
                } else {
                    Some(f.desc.clone())
                },
            })
            .collect())
    }

    /// List envelopes in a folder
    pub async fn list_envelopes(
        &self,
        folder: Option<&str>,
        page: usize,
        page_size: usize,
    ) -> Result<Vec<Envelope>, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);

        debug!(
            "Listing envelopes for {}: folder={}, page={}, page_size={}",
            self.account_config.email, folder, page, page_size
        );

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let start_time = std::time::Instant::now();
        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| {
                let elapsed = start_time.elapsed();
                let error_msg = format!(
                    "Failed to build IMAP backend after {:?} while listing envelopes: {}",
                    elapsed, e
                );
                warn!("{}", error_msg);

                // Classify error type
                let error_str = e.to_string().to_lowercase();
                if error_str.contains("timeout") || error_str.contains("timed out") {
                    EddieError::Network(format!("Connection timeout: {}", error_msg))
                } else if error_str.contains("connection") {
                    EddieError::Network(format!("Connection error: {}", error_msg))
                } else {
                    EddieError::Backend(error_msg)
                }
            })?;

        use email::envelope::list::{ListEnvelopes, ListEnvelopesOptions};

        let opts = ListEnvelopesOptions {
            page,
            page_size,
            query: None,
        };

        let envelopes = backend
            .list_envelopes(folder, opts)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let result: Vec<Envelope> = envelopes
            .into_iter()
            .map(|e| {
                // info!(
                //     "Fetched envelope: [{}] {} >> {}: {}",
                //     e.date.to_rfc3339(),
                //     e.from.to_string(),
                //     e.to.to_string(),
                //     e.subject
                // );
                Envelope {
                    id: e.id.clone(),
                    message_id: if e.message_id.is_empty() {
                        None
                    } else {
                        Some(e.message_id.clone())
                    },
                    in_reply_to: e.in_reply_to.clone(),
                    from: e.from.to_string(),
                    to: vec![e.to.to_string()],
                    cc: e.cc.iter().map(|addr| addr.to_string()).collect(),
                    subject: e.subject.clone(),
                    date: e.date.to_rfc3339(),
                    flags: e.flags.iter().map(|f| f.to_string()).collect(),
                    has_attachment: e.has_attachment,
                }
            })
            .collect();

        Ok(result)
    }

    /// Get a message by ID
    pub async fn get_message(
        &self,
        folder: Option<&str>,
        id: &str,
        peek: bool,
    ) -> Result<ChatMessage, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_id = Id::single(id);

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = if peek {
            backend
                .peek_messages(folder, &msg_id)
                .await
                .map_err(|e| EddieError::Backend(e.to_string()))?
        } else {
            backend
                .get_messages(folder, &msg_id)
                .await
                .map_err(|e| EddieError::Backend(e.to_string()))?
        };

        let msg = messages
            .first()
            .ok_or_else(|| EddieError::MessageNotFound(id.to_string()))?;

        // Parse the message to extract content
        let parsed = msg
            .parsed()
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        // Extract text and html bodies from parsed message
        let text_body = parsed.body_text(0).map(|s| s.to_string());
        let html_body = parsed.body_html(0).map(|s| s.to_string());

        // Extract attachments info
        let attachments: Vec<Attachment> = msg
            .attachments()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .into_iter()
            .map(|a| Attachment {
                filename: a.filename,
                mime_type: a.mime.to_string(),
                size: a.body.len(),
            })
            .collect();

        // Get headers from parsed message
        let from = parsed
            .from()
            .and_then(|a| a.first())
            .map(|a| {
                if let Some(name) = a.name() {
                    format!("{} <{}>", name, a.address().unwrap_or(""))
                } else {
                    a.address().unwrap_or("").to_string()
                }
            })
            .unwrap_or_default();

        let to: Vec<String> = parsed
            .to()
            .map(|list| {
                list.iter()
                    .map(|a| {
                        if let Some(name) = a.name() {
                            format!("{} <{}>", name, a.address().unwrap_or(""))
                        } else {
                            a.address().unwrap_or("").to_string()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cc: Vec<String> = parsed
            .cc()
            .map(|list| {
                list.iter()
                    .map(|a| {
                        if let Some(name) = a.name() {
                            format!("{} <{}>", name, a.address().unwrap_or(""))
                        } else {
                            a.address().unwrap_or("").to_string()
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        let subject = parsed.subject().map(|s| s.to_string()).unwrap_or_default();
        let date = parsed.date().map(|d| d.to_rfc3339()).unwrap_or_default();
        let message_id = parsed.message_id().map(|s| s.to_string());
        let in_reply_to = parsed.in_reply_to().as_text().map(|s| s.to_string());

        // info!(
        //     "Fetched message: [{}] {} >> {:?}: {}",
        //     date, from, to, subject
        // );

        Ok(ChatMessage {
            id: id.to_string(),
            envelope: Envelope {
                id: id.to_string(),
                message_id,
                in_reply_to,
                from,
                to,
                cc,
                subject,
                date,
                flags: vec![],
                has_attachment: !attachments.is_empty(),
            },
            headers: parsed
                .headers()
                .iter()
                .map(|h| {
                    (
                        h.name().to_string(),
                        h.value().as_text().unwrap_or("").to_string(),
                    )
                })
                .collect(),
            text_body,
            html_body,
            attachments,
        })
    }

    /// Get attachment info for a message without content
    pub async fn get_attachment_info(
        &self,
        folder: Option<&str>,
        id: &str,
    ) -> Result<Vec<Attachment>, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_id = Id::single(id);

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = backend
            .peek_messages(folder, &msg_id)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let msg = messages
            .first()
            .ok_or_else(|| EddieError::MessageNotFound(id.to_string()))?;

        let attachments: Vec<Attachment> = msg
            .attachments()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .into_iter()
            .map(|a| Attachment {
                filename: a.filename,
                mime_type: a.mime.to_string(),
                size: a.body.len(),
            })
            .collect();

        Ok(attachments)
    }

    /// Download a specific attachment and save to disk
    pub async fn download_attachment(
        &self,
        folder: Option<&str>,
        id: &str,
        attachment_index: usize,
        download_dir: &std::path::Path,
    ) -> Result<PathBuf, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_id = Id::single(id);

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = backend
            .peek_messages(folder, &msg_id)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let msg = messages
            .first()
            .ok_or_else(|| EddieError::MessageNotFound(id.to_string()))?;

        let attachments: Vec<_> = msg
            .attachments()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .into_iter()
            .collect();

        let attachment = attachments
            .get(attachment_index)
            .ok_or_else(|| EddieError::Backend(format!("Attachment index {} not found", attachment_index)))?;

        let filename = attachment
            .filename
            .clone()
            .unwrap_or_else(|| format!("attachment_{}", attachment_index));

        // Sanitize filename to prevent path traversal
        let safe_filename = std::path::Path::new(&filename)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("attachment_{}", attachment_index));

        let file_path = download_dir.join(&safe_filename);

        // Write the attachment content to disk
        std::fs::write(&file_path, &attachment.body)
            .map_err(|e| EddieError::Backend(format!("Failed to write attachment: {}", e)))?;

        info!("Downloaded attachment: {}", file_path.display());
        Ok(file_path)
    }

    /// Download all attachments and save to disk
    pub async fn download_all_attachments(
        &self,
        folder: Option<&str>,
        id: &str,
        download_dir: &std::path::Path,
    ) -> Result<Vec<PathBuf>, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_id = Id::single(id);

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let messages = backend
            .peek_messages(folder, &msg_id)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let msg = messages
            .first()
            .ok_or_else(|| EddieError::MessageNotFound(id.to_string()))?;

        let attachments: Vec<_> = msg
            .attachments()
            .map_err(|e| EddieError::Backend(e.to_string()))?
            .into_iter()
            .collect();

        let mut saved_files = Vec::new();

        for (index, attachment) in attachments.iter().enumerate() {
            let filename = attachment
                .filename
                .clone()
                .unwrap_or_else(|| format!("attachment_{}", index));

            // Sanitize filename to prevent path traversal
            let safe_filename = std::path::Path::new(&filename)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| format!("attachment_{}", index));

            let file_path = download_dir.join(&safe_filename);

            // Write the attachment content to disk
            std::fs::write(&file_path, &attachment.body)
                .map_err(|e| EddieError::Backend(format!("Failed to write attachment: {}", e)))?;

            info!("Downloaded attachment: {}", file_path.display());
            saved_files.push(file_path);
        }

        Ok(saved_files)
    }

    /// Add flags to messages
    pub async fn add_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .add_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Remove flags from messages
    pub async fn remove_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .remove_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Set flags on messages (replace)
    pub async fn set_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .set_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Delete messages
    pub async fn delete_messages(
        &self,
        folder: Option<&str>,
        ids: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .delete_messages(folder, &msg_ids)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Copy messages to another folder
    pub async fn copy_messages(
        &self,
        source_folder: Option<&str>,
        target_folder: &str,
        ids: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let source = source_folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .copy_messages(source, target_folder, &msg_ids)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Move messages to another folder
    pub async fn move_messages(
        &self,
        source_folder: Option<&str>,
        target_folder: &str,
        ids: &[&str],
    ) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;
        let source = source_folder.unwrap_or(INBOX);
        let msg_ids = Id::multiple(ids.iter().map(|s| s.to_string()).collect::<Vec<_>>());

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .move_messages(source, target_folder, &msg_ids)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Create a folder
    pub async fn create_folder(&self, name: &str) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .add_folder(name)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Delete a folder
    pub async fn delete_folder(&self, name: &str) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .delete_folder(name)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Expunge folder (permanently remove deleted messages)
    pub async fn expunge_folder(&self, name: &str) -> Result<(), EddieError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .expunge_folder(name)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))
    }

    /// Extract Message-ID header from raw message bytes
    /// Used for deduplication tracking
    fn extract_message_id(raw_message: &[u8]) -> Option<String> {
        let message_str = String::from_utf8_lossy(raw_message);

        // Find the Message-ID header (case-insensitive)
        for line in message_str.lines() {
            if line.is_empty() {
                // Reached end of headers
                break;
            }
            if line.to_lowercase().starts_with("message-id:") {
                // Extract the Message-ID value (after "Message-ID: ")
                let msg_id = line[11..].trim().to_string();
                debug!("Extracted Message-ID from raw message: {}", msg_id);
                return Some(msg_id);
            }
        }

        warn!("Could not extract Message-ID from raw message");
        None
    }

    /// Send a message via SMTP and save to Sent folder
    /// Returns the UID, Message-ID header, and sent folder name, or None if no Sent folder was found
    pub async fn send_message(
        &self,
        raw_message: &[u8],
    ) -> Result<Option<SendMessageResult>, EddieError> {
        // Extract Message-ID before sending for deduplication tracking
        let message_id = Self::extract_message_id(raw_message)
            .unwrap_or_else(|| "<unknown>".to_string());

        info!("=== SEND MESSAGE START ===");
        info!("Message-ID: {}", message_id);
        debug!("Raw message size: {} bytes", raw_message.len());

        // First, send via SMTP
        let smtp_config = self.build_smtp_config().await?;

        let ctx = email::smtp::SmtpContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(smtp_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        backend
            .send_message(raw_message)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        info!("Message sent via SMTP successfully");

        // Now save to Sent folder via IMAP
        let sent_folder = self.find_sent_folder().await?;

        if let Some(folder) = sent_folder {
            info!("Saving sent message to folder: {}", folder);
            let uid = self.save_message(Some(&folder), raw_message).await?;
            info!("Message saved to Sent folder with UID: {}", uid);
            info!("=== SEND MESSAGE END ===");
            Ok(Some(SendMessageResult {
                uid,
                message_id,
                sent_folder: folder,
            }))
        } else {
            info!("No Sent folder found, message not saved to IMAP");
            info!("=== SEND MESSAGE END ===");
            Ok(None)
        }
    }

    /// Save a message to a folder
    pub async fn save_message(
        &self,
        folder: Option<&str>,
        raw_message: &[u8],
    ) -> Result<String, EddieError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or("Drafts");

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        let id = backend
            .add_message(folder, raw_message)
            .await
            .map_err(|e| EddieError::Backend(e.to_string()))?;

        Ok(id.to_string())
    }
}

/// Get backend for account (or default)
pub async fn get_backend(account: Option<&str>) -> Result<EmailBackend, EddieError> {
    match account {
        Some(name) => EmailBackend::new(name).await,
        None => EmailBackend::default().await,
    }
}
