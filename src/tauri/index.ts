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
  // Core types
  Envelope,
  Message,
  Attachment,

  // Account types
  Account,
  AccountDetails,
  SaveAccountRequest,
  SaveDiscoveredAccountRequest,

  // Conversation types
  Conversation,

  // Sync types
  SyncStatus,
  CachedConversation,
  CachedMessage,
  SyncActionType,
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
