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

    // TODO: Cleanup after verifying Martins inbox
    // logger::debug(&format!("Listed {} IMAP folders:", folder_infos.len()));
    // for f in &folder_infos {
    //     logger::debug(&format!(
    //         "  folder: {:30} | attrs: [{}] | priority: {:?}",
    //         f.name,
    //         f.attributes.join(", "),
    //         f.priority,
    //     ));
    // }
    Ok(folder_infos)
}

pub fn find_folder_by_attribute(folders: &[FolderInfo], attribute: &str) -> Option<String> {
    folders.iter()
        .find(|f| f.attributes.iter().any(|a| a.contains(attribute)))
        .map(|f| f.name.clone())
}

/// Known Sent folder names across languages and email providers.
/// Used as a fallback when the server doesn't advertise the \Sent attribute (RFC 6154).
/// Each entry is matched as a substring against the last segment of the folder name
/// (after splitting by hierarchy delimiters like '.' or '/').
const SENT_FOLDER_NAMES: &[&str] = &[
    // English (Thunderbird, Apple Mail, Outlook, generic)
    "sent",             // most common
    "sent items",       // Outlook/Exchange
    "sent messages",    // Apple Mail
    "sent mail",        // some providers
    // German
    "gesendet",         // Thunderbird/generic
    "gesendete objekte",// Outlook
    "gesendete elemente",
    // French
    "envoyés",          // Thunderbird
    "éléments envoyés", // Outlook
    "messages envoyés",
    // Spanish
    "enviados",         // Thunderbird
    "elementos enviados",// Outlook
    // Portuguese
    "enviadas",
    "itens enviados",   // Outlook
    // Italian
    "inviata",          // Posta inviata
    "inviati",
    // Dutch
    "verzonden",        // Thunderbird
    "verzonden items",  // Outlook
    // Swedish
    "skickat",          // Thunderbird
    "skickade",         // Outlook
    // Danish / Norwegian
    "sendt",
    "sendte",
    "sendte elementer",
    // Finnish
    "lähetetyt",
    // Polish
    "wysłane",
    // Czech
    "odeslané",
    "odeslaná pošta",
    // Hungarian
    "elküldött",
    "elküldött elemek",
    // Romanian
    "trimise",
    "mesaje trimise",
    // Russian
    "отправленные",
    // Turkish
    "gönderilenler",
    // Greek
    "απεσταλμένα",
];

/// Find the Sent folder using a 3-tier strategy:
/// 1. IMAP attribute match (\Sent from RFC 6154) — most reliable
/// 2. Gmail labels (not needed here — Gmail uses All Mail for sync)
/// 3. Name-based fallback against known Sent folder names across languages
pub fn find_sent_folder(folders: &[FolderInfo]) -> Option<String> {
    // Tier 1: attribute match
    if let Some(name) = find_folder_by_attribute(folders, "Sent") {
        logger::debug(&format!("Sent folder found by attribute: {}", name));
        return Some(name);
    }

    // Tier 3: name-based fallback (tier 2 = Gmail labels, not applicable here)
    for folder in folders {
        if folder.priority == FolderPriority::NoSelect || folder.priority == FolderPriority::Excluded {
            continue;
        }
        // Extract the last segment after hierarchy delimiters (e.g., "INBOX.Sent" → "Sent")
        let leaf = folder.name
            .rsplit_once('.')
            .or_else(|| folder.name.rsplit_once('/'))
            .map(|(_, leaf)| leaf)
            .unwrap_or(&folder.name);
        let leaf_lower = leaf.to_lowercase();

        for known in SENT_FOLDER_NAMES {
            if leaf_lower == *known {
                logger::debug(&format!("Sent folder found by name fallback: {} (matched '{}')", folder.name, known));
                return Some(folder.name.clone());
            }
        }
    }

    logger::debug("No Sent folder found by attribute or name");
    None
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