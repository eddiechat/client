//! Email backend service using email-lib
//!
//! This module provides the core email functionality using IMAP for receiving
//! and SMTP for sending emails.

use std::process::Command;
use std::sync::Arc;

use email::account::config::{passwd::PasswordConfig, AccountConfig as EmailAccountConfig};
use email::backend::BackendBuilder;
use email::envelope::list::{ListEnvelopes, ListEnvelopesOptions};
use email::envelope::Id;
use email::flag::{add::AddFlags, remove::RemoveFlags, set::SetFlags, Flag, Flags};
use email::folder::{
    add::AddFolder, delete::DeleteFolder, expunge::ExpungeFolder, list::ListFolders, INBOX,
};
use email::imap::config::{ImapAuthConfig, ImapConfig as EmailImapConfig};
use email::message::{
    add::AddMessage, copy::CopyMessages, delete::DeleteMessages, get::GetMessages,
    r#move::MoveMessages, peek::PeekMessages, send::SendMessage,
};
use email::smtp::config::{SmtpAuthConfig, SmtpConfig as EmailSmtpConfig};
use email::tls::{Encryption, Tls};
use std::path::PathBuf;
use secret::Secret;
use tracing::info;

use crate::config::{self, AccountConfig, AuthConfig, PasswordSource};
use crate::types::error::HimalayaError;
use crate::types::{Attachment, Envelope, Folder, Message};

/// Backend service for email operations
pub struct EmailBackend {
    /// Account name
    #[allow(dead_code)]
    account_name: String,
    /// Account configuration from our config
    account_config: AccountConfig,
    /// email-lib account configuration
    email_account_config: Arc<EmailAccountConfig>,
}

impl EmailBackend {
    /// Create a new email backend for an account
    pub async fn new(account_name: &str) -> Result<Self, HimalayaError> {
        let config = config::get_config()?;
        let (name, account_config) = config
            .get_account(Some(account_name))
            .ok_or_else(|| HimalayaError::AccountNotFound(account_name.to_string()))?;

        let account_config = account_config.clone();
        let account_name = name.to_string();

        // Build email-lib account config
        let email_account_config = Arc::new(EmailAccountConfig {
            name: account_name.clone(),
            email: account_config.email.clone(),
            display_name: account_config.display_name.clone(),
            ..Default::default()
        });

        Ok(Self {
            account_name,
            account_config,
            email_account_config,
        })
    }

    /// Create backend for default account
    pub async fn default() -> Result<Self, HimalayaError> {
        let config = config::get_config()?;
        let account_name = config
            .default_account_name()
            .ok_or_else(|| HimalayaError::Config("No accounts configured".to_string()))?
            .to_string();

        Self::new(&account_name).await
    }

    /// Get or resolve password from PasswordSource
    async fn resolve_password(source: &PasswordSource) -> Result<String, HimalayaError> {
        match source {
            PasswordSource::Raw(password) => Ok(password.clone()),
            PasswordSource::Command { command } => {
                info!("Executing password command");
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .output()
                    .map_err(|e| {
                        HimalayaError::Config(format!("Failed to run password command: {}", e))
                    })?;

                if !output.status.success() {
                    return Err(HimalayaError::Config("Password command failed".to_string()));
                }

                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
        }
    }

    /// Build IMAP configuration for email-lib
    async fn build_imap_config(&self) -> Result<EmailImapConfig, HimalayaError> {
        let imap = self
            .account_config
            .imap
            .as_ref()
            .ok_or_else(|| HimalayaError::Config("No IMAP configuration".to_string()))?;

        let auth = match &imap.auth {
            AuthConfig::Password { user: _, password } => {
                let passwd = Self::resolve_password(password).await?;
                ImapAuthConfig::Password(PasswordConfig(Secret::new_raw(passwd)))
            }
            AuthConfig::OAuth2 { .. } => {
                return Err(HimalayaError::Config("OAuth2 not yet supported".to_string()));
            }
        };

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
            login: match &imap.auth {
                AuthConfig::Password { user, .. } => user.clone(),
                AuthConfig::OAuth2 { .. } => self.account_config.email.clone(),
            },
            auth,
            ..Default::default()
        })
    }

    /// Build SMTP configuration for email-lib
    async fn build_smtp_config(&self) -> Result<EmailSmtpConfig, HimalayaError> {
        let smtp = self
            .account_config
            .smtp
            .as_ref()
            .ok_or_else(|| HimalayaError::Config("No SMTP configuration".to_string()))?;

        let auth = match &smtp.auth {
            AuthConfig::Password { user: _, password } => {
                let passwd = Self::resolve_password(password).await?;
                SmtpAuthConfig::Password(PasswordConfig(Secret::new_raw(passwd)))
            }
            AuthConfig::OAuth2 { .. } => {
                return Err(HimalayaError::Config("OAuth2 not yet supported".to_string()));
            }
        };

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
            login: match &smtp.auth {
                AuthConfig::Password { user, .. } => user.clone(),
                AuthConfig::OAuth2 { .. } => self.account_config.email.clone(),
            },
            auth,
            ..Default::default()
        })
    }

    /// List all folders
    pub async fn list_folders(&self) -> Result<Vec<Folder>, HimalayaError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let folders = backend
            .list_folders()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

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
    ) -> Result<Vec<Envelope>, HimalayaError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or(INBOX);

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let opts = ListEnvelopesOptions {
            page,
            page_size,
            query: None,
        };

        let envelopes = backend
            .list_envelopes(folder, opts)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(envelopes
            .into_iter()
            .map(|e| Envelope {
                id: e.id.clone(),
                message_id: if e.message_id.is_empty() {
                    None
                } else {
                    Some(e.message_id.clone())
                },
                in_reply_to: e.in_reply_to.clone(),
                from: e.from.to_string(),
                to: vec![e.to.to_string()],
                subject: e.subject.clone(),
                date: e.date.to_rfc3339(),
                flags: e.flags.iter().map(|f| f.to_string()).collect(),
                has_attachment: e.has_attachment,
            })
            .collect())
    }

    /// Get a message by ID
    pub async fn get_message(
        &self,
        folder: Option<&str>,
        id: &str,
        peek: bool,
    ) -> Result<Message, HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let messages = if peek {
            backend
                .peek_messages(folder, &msg_id)
                .await
                .map_err(|e| HimalayaError::Backend(e.to_string()))?
        } else {
            backend
                .get_messages(folder, &msg_id)
                .await
                .map_err(|e| HimalayaError::Backend(e.to_string()))?
        };

        let msg = messages
            .first()
            .ok_or_else(|| HimalayaError::MessageNotFound(id.to_string()))?;

        // Parse the message to extract content
        let parsed = msg
            .parsed()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        // Extract text and html bodies from parsed message
        let text_body = parsed.body_text(0).map(|s| s.to_string());
        let html_body = parsed.body_html(0).map(|s| s.to_string());

        // Extract attachments info
        let attachments: Vec<Attachment> = msg
            .attachments()
            .map_err(|e| HimalayaError::Backend(e.to_string()))?
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

        let subject = parsed.subject().map(|s| s.to_string()).unwrap_or_default();
        let date = parsed
            .date()
            .map(|d| d.to_rfc3339())
            .unwrap_or_default();
        let message_id = parsed.message_id().map(|s| s.to_string());
        let in_reply_to = parsed.in_reply_to().as_text().map(|s| s.to_string());

        Ok(Message {
            id: id.to_string(),
            envelope: Envelope {
                id: id.to_string(),
                message_id,
                in_reply_to,
                from,
                to,
                subject,
                date,
                flags: vec![],
                has_attachment: !attachments.is_empty(),
            },
            headers: parsed
                .headers()
                .iter()
                .map(|h| (h.name().to_string(), h.value().as_text().unwrap_or("").to_string()))
                .collect(),
            text_body,
            html_body,
            attachments,
        })
    }

    /// Add flags to messages
    pub async fn add_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .add_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Remove flags from messages
    pub async fn remove_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .remove_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Set flags on messages (replace)
    pub async fn set_flags(
        &self,
        folder: Option<&str>,
        ids: &[&str],
        flags: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let flags: Flags = flags.iter().map(|f| Flag::from(*f)).collect();

        backend
            .set_flags(folder, &msg_ids, &flags)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Delete messages
    pub async fn delete_messages(
        &self,
        folder: Option<&str>,
        ids: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .delete_messages(folder, &msg_ids)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Copy messages to another folder
    pub async fn copy_messages(
        &self,
        source_folder: Option<&str>,
        target_folder: &str,
        ids: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .copy_messages(source, target_folder, &msg_ids)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Move messages to another folder
    pub async fn move_messages(
        &self,
        source_folder: Option<&str>,
        target_folder: &str,
        ids: &[&str],
    ) -> Result<(), HimalayaError> {
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
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .move_messages(source, target_folder, &msg_ids)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Create a folder
    pub async fn create_folder(&self, name: &str) -> Result<(), HimalayaError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .add_folder(name)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Delete a folder
    pub async fn delete_folder(&self, name: &str) -> Result<(), HimalayaError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .delete_folder(name)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Expunge folder (permanently remove deleted messages)
    pub async fn expunge_folder(&self, name: &str) -> Result<(), HimalayaError> {
        let imap_config = self.build_imap_config().await?;

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .expunge_folder(name)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Send a message via SMTP
    pub async fn send_message(&self, raw_message: &[u8]) -> Result<(), HimalayaError> {
        let smtp_config = self.build_smtp_config().await?;

        let ctx = email::smtp::SmtpContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(smtp_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        backend
            .send_message(raw_message)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))
    }

    /// Save a message to a folder
    pub async fn save_message(
        &self,
        folder: Option<&str>,
        raw_message: &[u8],
    ) -> Result<String, HimalayaError> {
        let imap_config = self.build_imap_config().await?;
        let folder = folder.unwrap_or("Drafts");

        let ctx = email::imap::ImapContextBuilder::new(
            self.email_account_config.clone(),
            Arc::new(imap_config),
        );

        let backend = BackendBuilder::new(self.email_account_config.clone(), ctx)
            .build()
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        let id = backend
            .add_message(folder, raw_message)
            .await
            .map_err(|e| HimalayaError::Backend(e.to_string()))?;

        Ok(id.to_string())
    }
}

/// Get backend for account (or default)
pub async fn get_backend(account: Option<&str>) -> Result<EmailBackend, HimalayaError> {
    match account {
        Some(name) => EmailBackend::new(name).await,
        None => EmailBackend::default().await,
    }
}
