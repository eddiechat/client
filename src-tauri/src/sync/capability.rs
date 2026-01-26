//! IMAP Server Capability Detection
//!
//! Detects server capabilities to determine the best sync strategy:
//! - QRESYNC: Full incremental sync with flag changes and deletions in one round-trip
//! - CONDSTORE: Incremental flag changes, separate deletion detection
//! - Bare IMAP: Full flag comparison against cache

use serde::{Deserialize, Serialize};

/// Server capability level for sync strategy selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServerCapability {
    /// QRESYNC available (Dovecot, Cyrus) - best performance
    /// Use SELECT...QRESYNC to get flag changes AND deletions (VANISHED) in one round-trip
    Qresync,

    /// CONDSTORE only (Gmail) - good performance
    /// Use FETCH...CHANGEDSINCE for flag changes, UID SEARCH to detect deletions
    Condstore,

    /// Bare IMAP (Exchange) - baseline performance
    /// Full UID FETCH 1:* FLAGS comparison against cache
    Bare,
}

impl ServerCapability {
    /// Get a human-readable description of this capability level
    pub fn description(&self) -> &'static str {
        match self {
            Self::Qresync => "QRESYNC (optimal sync with VANISHED responses)",
            Self::Condstore => "CONDSTORE (incremental flag sync)",
            Self::Bare => "Basic IMAP (full flag comparison)",
        }
    }
}

/// Capability detection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInfo {
    /// Best sync strategy available
    pub sync_capability: ServerCapability,

    /// Whether IDLE is supported for real-time notifications
    pub supports_idle: bool,

    /// Whether COMPRESS is supported
    pub supports_compress: bool,

    /// Whether MOVE is supported (vs COPY + DELETE)
    pub supports_move: bool,

    /// Whether SPECIAL-USE is supported for folder identification
    pub supports_special_use: bool,

    /// Whether UIDPLUS is supported
    pub supports_uidplus: bool,

    /// Raw capability strings from server
    pub raw_capabilities: Vec<String>,
}

impl Default for CapabilityInfo {
    fn default() -> Self {
        Self {
            sync_capability: ServerCapability::Bare,
            supports_idle: false,
            supports_compress: false,
            supports_move: false,
            supports_special_use: false,
            supports_uidplus: false,
            raw_capabilities: Vec::new(),
        }
    }
}

/// Capability detector for IMAP servers
pub struct CapabilityDetector;

impl CapabilityDetector {
    /// Detect capabilities from a list of capability strings
    ///
    /// Capability strings are typically returned from:
    /// - Initial greeting (untagged CAPABILITY)
    /// - CAPABILITY command response
    /// - OK response with [CAPABILITY ...] code
    pub fn detect(capabilities: &[String]) -> CapabilityInfo {
        let caps_upper: Vec<String> = capabilities.iter().map(|c| c.to_uppercase()).collect();

        let has_qresync = caps_upper.iter().any(|c| c == "QRESYNC");
        let has_condstore = caps_upper.iter().any(|c| c == "CONDSTORE");
        let has_enable = caps_upper.iter().any(|c| c == "ENABLE");

        // QRESYNC requires CONDSTORE and ENABLE
        let sync_capability = if has_qresync && has_condstore && has_enable {
            ServerCapability::Qresync
        } else if has_condstore {
            ServerCapability::Condstore
        } else {
            ServerCapability::Bare
        };

        CapabilityInfo {
            sync_capability,
            supports_idle: caps_upper.iter().any(|c| c == "IDLE"),
            supports_compress: caps_upper.iter().any(|c| c.starts_with("COMPRESS")),
            supports_move: caps_upper.iter().any(|c| c == "MOVE"),
            supports_special_use: caps_upper.iter().any(|c| c == "SPECIAL-USE"),
            supports_uidplus: caps_upper.iter().any(|c| c == "UIDPLUS"),
            raw_capabilities: capabilities.to_vec(),
        }
    }

    /// Parse capabilities from a CAPABILITY response line
    ///
    /// Example: "* CAPABILITY IMAP4rev1 UNSELECT IDLE NAMESPACE QUOTA ID..."
    pub fn parse_capability_line(line: &str) -> Vec<String> {
        let line = line.trim();

        // Handle untagged response format: "* CAPABILITY ..."
        let caps_str = if line.starts_with("* CAPABILITY ") {
            &line[13..]
        } else if line.starts_with("CAPABILITY ") {
            &line[11..]
        } else if line.contains("[CAPABILITY ") {
            // Handle OK response: "... [CAPABILITY ...] ..."
            if let Some(start) = line.find("[CAPABILITY ") {
                let after_cap = &line[start + 12..];
                if let Some(end) = after_cap.find(']') {
                    &after_cap[..end]
                } else {
                    return Vec::new();
                }
            } else {
                return Vec::new();
            }
        } else {
            line
        };

        caps_str.split_whitespace().map(|s| s.to_string()).collect()
    }
}

/// Known capability profiles for common providers
pub mod profiles {
    use super::*;

    /// Gmail capability profile
    pub fn gmail() -> CapabilityInfo {
        CapabilityInfo {
            sync_capability: ServerCapability::Condstore,
            supports_idle: true,
            supports_compress: true,
            supports_move: true,
            supports_special_use: true,
            supports_uidplus: true,
            raw_capabilities: vec![
                "IMAP4rev1".to_string(),
                "CONDSTORE".to_string(),
                "IDLE".to_string(),
                "COMPRESS=DEFLATE".to_string(),
                "MOVE".to_string(),
                "SPECIAL-USE".to_string(),
                "UIDPLUS".to_string(),
            ],
        }
    }

    /// Dovecot capability profile (typical)
    pub fn dovecot() -> CapabilityInfo {
        CapabilityInfo {
            sync_capability: ServerCapability::Qresync,
            supports_idle: true,
            supports_compress: true,
            supports_move: true,
            supports_special_use: true,
            supports_uidplus: true,
            raw_capabilities: vec![
                "IMAP4rev1".to_string(),
                "QRESYNC".to_string(),
                "CONDSTORE".to_string(),
                "ENABLE".to_string(),
                "IDLE".to_string(),
                "MOVE".to_string(),
                "SPECIAL-USE".to_string(),
                "UIDPLUS".to_string(),
            ],
        }
    }

    /// Exchange/Office365 capability profile (conservative)
    pub fn exchange() -> CapabilityInfo {
        CapabilityInfo {
            sync_capability: ServerCapability::Bare,
            supports_idle: true,
            supports_compress: false,
            supports_move: true,
            supports_special_use: true,
            supports_uidplus: true,
            raw_capabilities: vec![
                "IMAP4rev1".to_string(),
                "IDLE".to_string(),
                "MOVE".to_string(),
                "SPECIAL-USE".to_string(),
                "UIDPLUS".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_qresync() {
        let caps = vec![
            "IMAP4rev1".to_string(),
            "QRESYNC".to_string(),
            "CONDSTORE".to_string(),
            "ENABLE".to_string(),
            "IDLE".to_string(),
        ];

        let info = CapabilityDetector::detect(&caps);
        assert_eq!(info.sync_capability, ServerCapability::Qresync);
        assert!(info.supports_idle);
    }

    #[test]
    fn test_detect_condstore() {
        let caps = vec![
            "IMAP4rev1".to_string(),
            "CONDSTORE".to_string(),
            "IDLE".to_string(),
        ];

        let info = CapabilityDetector::detect(&caps);
        assert_eq!(info.sync_capability, ServerCapability::Condstore);
    }

    #[test]
    fn test_detect_bare() {
        let caps = vec!["IMAP4rev1".to_string(), "IDLE".to_string()];

        let info = CapabilityDetector::detect(&caps);
        assert_eq!(info.sync_capability, ServerCapability::Bare);
    }

    #[test]
    fn test_parse_capability_line() {
        let line = "* CAPABILITY IMAP4rev1 UNSELECT IDLE CONDSTORE";
        let caps = CapabilityDetector::parse_capability_line(line);
        assert_eq!(caps, vec!["IMAP4rev1", "UNSELECT", "IDLE", "CONDSTORE"]);
    }

    #[test]
    fn test_parse_capability_from_ok() {
        let line = "A001 OK [CAPABILITY IMAP4rev1 IDLE CONDSTORE] Logged in";
        let caps = CapabilityDetector::parse_capability_line(line);
        assert_eq!(caps, vec!["IMAP4rev1", "IDLE", "CONDSTORE"]);
    }
}
