use futures::TryStreamExt;
use crate::services::logger;

use super::connection::ImapSession;
use crate::error::EddieError;

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub enum FolderPriority {
    High,
    Medium,
    Low,
    Excluded,
    NoSelect,
}

#[derive(Debug, serde::Serialize)]
pub struct FolderInfo {
    pub name: String,
    pub attributes: Vec<String>,
    pub priority: FolderPriority,
}

impl FolderInfo {
    fn compute_priority(name: &str, attributes: &[String]) -> FolderPriority {
        if attributes.iter().any(|a| a.contains("NoSelect")) {
            return FolderPriority::NoSelect;
        }

        for attr in attributes {
            if attr.contains("Junk") || attr.contains("Trash") {
                return FolderPriority::Excluded;
            }
        }

        if name == "INBOX" {
            return FolderPriority::High;
        }

        for attr in attributes {
            if attr.contains("Sent") || attr.contains("Drafts") {
                return FolderPriority::High;
            }
            if attr.contains("All") {
                return FolderPriority::Medium;
            }
        }

        FolderPriority::Low
    }
}

pub async fn list_folders(session: &mut ImapSession) -> Result<Vec<FolderInfo>, EddieError> {
    let folders: Vec<_> = session
        .list(None, Some("*"))
        .await
        .map_err(|e| EddieError::Backend(format!("LIST failed: {}", e)))?
        .try_collect()
        .await
        .map_err(|e| EddieError::Backend(format!("Failed to collect folders: {}", e)))?;

    let folder_infos: Vec<FolderInfo> = folders
        .iter()
        .map(|f| {
            let name = f.name().to_string();
            let attributes: Vec<String> =
                f.attributes().iter().map(|a| format!("{:?}", a)).collect();
            let priority = FolderInfo::compute_priority(&name, &attributes);
            FolderInfo {
                name,
                attributes,
                priority,
            }
        })
        .collect();

    logger::debug(&format!("Listed {} IMAP folders", folder_infos.len()));
    Ok(folder_infos)
}

pub fn find_folder_by_attribute(folders: &[FolderInfo], attribute: &str) -> Option<String> {
    folders.iter()
        .find(|f| f.attributes.iter().any(|a| a.contains(attribute)))
        .map(|f| f.name.clone())
}

const SKIP_ATTRIBUTES: &[&str] = &["Drafts", "Trash", "Junk", "NoSelect", "All", "Flagged"];

pub fn folders_to_sync(folders: &[FolderInfo], is_gmail: bool) -> Vec<&FolderInfo> {
    if is_gmail {
        // Gmail: only sync All Mail (contains every message exactly once)
        folders.iter().filter(|f| {
            f.attributes.iter().any(|a| a.contains("All"))
        }).collect()
    } else {
        // Non-Gmail: skip Drafts, Trash, Junk, NoSelect, All, Flagged
        folders.iter().filter(|f| {
            !f.attributes.iter().any(|attr| SKIP_ATTRIBUTES.contains(&attr.as_str()))
        }).collect()
    }
}