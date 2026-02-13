/**
 * Tauri integration layer.
 *
 * All Tauri communication should go through this module.
 * Components should never call invoke() directly.
 */

// Export all commands
export * from "./commands";

// Export all events
export * from "./events";

// Export all types
export type {
  // Display types (frontend rendering)
  Envelope,
  ChatMessage,
  Attachment,

  // Account types
  EmailAccount,
  EmailAccountDetails,
  SaveEmailAccountRequest,
  SaveDiscoveredEmailAccountRequest,

  // Conversation types
  Conversation,

  // Sync types
  SyncStatus,
  Message,
  SyncEventPayload,

  // Compose types
  ComposeAttachment,
  ComposeMessageData,
  SendMessageResult,

  // Attachment types
  AttachmentInfo,

  // Discovery types
  DiscoveryResult,
} from "./types";
