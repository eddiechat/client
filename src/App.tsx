import { useState, useCallback, useEffect } from "react";
import {
  AccountConfigModal,
  AccountSetupWizard,
  SidebarHeader,
  useAccounts,
  type AccountEditData,
} from "./features/accounts";
import {
  ChatMessages,
  ConversationView,
  InitialSyncLoader,
  useConversations,
  useConversationMessages,
} from "./features/conversations";
import {
  saveAccount,
  removeAccount,
  getAccountDetails,
  markConversationRead,
  sendMessageWithAttachments,
  syncNow,
  initSyncEngine,
  getReadOnlyMode,
} from "./tauri";
import type { Conversation, SaveEmailAccountRequest, ComposeAttachment } from "./tauri";
import { extractEmail, useResizableSidebar, ResizeHandle } from "./shared";
import "./App.css";

function App() {
  // Conversation selection state
  const [selectedConversation, setSelectedConversation] =
    useState<Conversation | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<'connections' | 'all' | 'others'>('connections');

  // Compose mode state (messenger-style compose in chat view)
  const [isComposing, setIsComposing] = useState(false);
  const [composeParticipants, setComposeParticipants] = useState<string[]>([]);

  // Account config modal state
  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [accountEditData, setAccountEditData] =
    useState<AccountEditData | null>(null);

  // Account setup wizard state
  const [setupWizardOpen, setSetupWizardOpen] = useState(false);

  // Current account aliases state
  const [currentAccountAliases, setCurrentAccountAliases] = useState<string[]>([]);

  // Hooks for data fetching
  const {
    accounts,
    currentAccount,
    setCurrentAccount,
    loading: accountsLoading,
    refresh: refreshAccounts,
  } = useAccounts();

  const { sidebarWidth, isDesktop, isDragging, handleMouseDown: handleResizeMouseDown } = useResizableSidebar();

  // Get current account email for determining message direction
  const currentAccountEmail = currentAccount || undefined;

  // Fetch account details (including aliases) when current account changes
  useEffect(() => {
    if (!currentAccount) {
      setCurrentAccountAliases([]);
      return;
    }

    getAccountDetails(currentAccount)
      .then((details) => {
        // Parse comma-separated aliases string into array
        const aliases = details.aliases
          ? details.aliases.split(',').map(a => a.trim()).filter(a => a.length > 0)
          : [];
        setCurrentAccountAliases(aliases);
      })
      .catch((err) => {
        console.error("Failed to fetch account aliases:", err);
        setCurrentAccountAliases([]);
      });
  }, [currentAccount]);

  // Show setup wizard when no accounts are configured
  const showSetupWizard = !accountsLoading && accounts.length === 0;

  // Conversations hook with tab filtering
  const {
    conversations,
    loading: conversationsLoading,
    syncing,
    syncStatus,
    refresh: refreshConversations,
  } = useConversations(currentAccount || undefined, activeFilter);

  // Show initial loader when: we have an account, no conversations, and either
  // in initial sync, loading, syncing, or never synced before
  const isInitialSync = syncStatus?.state === "syncing" || syncStatus?.state === "pending";
  const neverSynced = !!currentAccount && !syncStatus?.last_sync;
  const noConversations = conversations.length === 0;
  const showInitialLoader = noConversations && (isInitialSync || conversationsLoading || syncing || neverSynced);

  // Messages for selected conversation
  const {
    messages,
    loading: messagesLoading,
    error: messagesError,
    refresh: refreshMessages,
  } = useConversationMessages(selectedConversation, currentAccount || undefined);

  // Handlers
  const handleConversationSelect = useCallback(
    (conversation: Conversation) => {
      setSelectedConversation(conversation);
      setIsComposing(false);
      setComposeParticipants([]);

      // Mark messages as read when opening conversation
      if (conversation.unread_count > 0) {
        markConversationRead(conversation.id, currentAccount || undefined).catch(
          (err) => {
            console.error("Failed to mark conversation as read:", err);
          }
        );
      }
    },
    [currentAccount]
  );

  const handleCompose = useCallback(() => {
    setSelectedConversation(null);
    setIsComposing(true);
    setComposeParticipants([]);
  }, []);

  // Handle when participants are confirmed in compose mode
  const handleComposeParticipantsConfirm = useCallback(
    (participants: string[]) => {
      // Try to find existing conversation with these participants
      const normalizedParticipants = participants
        .map((p) => extractEmail(p).toLowerCase())
        .sort();

      const existingConversation = conversations.find((conv) => {
        // Filter out current user from conversation participants before comparing
        // Backend includes all participants including the current user,
        // but when composing users only enter the recipients
        const currentUserEmail = currentAccount?.toLowerCase();
        const convParticipants = conv.participants
          .map((p) => extractEmail(p).toLowerCase())
          .filter((p) => p !== currentUserEmail)
          .sort();

        // Check if participants match
        return (
          JSON.stringify(normalizedParticipants) ===
          JSON.stringify(convParticipants)
        );
      });

      if (existingConversation) {
        // Found existing conversation - switch to it
        setSelectedConversation(existingConversation);
        setIsComposing(false);
        setComposeParticipants([]);
      } else {
        // No existing conversation - stay in compose mode with participants set
        setComposeParticipants(participants);
      }
    },
    [conversations, currentAccount]
  );

  // Handle sending a new message in compose mode (no existing conversation)
  const handleSendNewMessage = useCallback(
    async (
      text: string,
      participants: string[],
      attachments?: ComposeAttachment[]
    ) => {
      if (
        (!text.trim() && (!attachments || attachments.length === 0)) ||
        participants.length === 0
      )
        return;

      // Check if readonly mode is enabled
      const isReadOnly = await getReadOnlyMode();
      if (isReadOnly) {
        alert(
          "Cannot send message: Eddie is currently in read-only mode.\n\n" +
          "To send messages, disable read-only mode in account settings."
        );
        return;
      }

      // Extract first line as subject
      const lines = text.split("\n");
      const subject = lines[0].trim() || "(No subject)";
      const body =
        lines.length > 1 ? lines.slice(1).join("\n").trim() || lines[0] : text;

      // Use the new API if we have attachments, otherwise use the legacy API for compatibility
      const result = await sendMessageWithAttachments(
        currentAccount || "user@example.com",
        participants,
        subject,
        body,
        attachments || [],
        undefined,
        currentAccount || undefined
      );

      // Trigger sync to pull the sent message into local database
      if (result?.sent_folder) {
        await syncNow();
      }

      // Exit compose mode and refresh
      setIsComposing(false);
      setComposeParticipants([]);
      await refreshConversations();

      // Try to select the newly created conversation
      const normalizedParticipants = participants
        .map((p) => extractEmail(p).toLowerCase())
        .sort();
      setTimeout(() => {
        const newConversation = conversations.find((conv) => {
          const convParticipants = conv.participants
            .map((p) => extractEmail(p).toLowerCase())
            .sort();
          return (
            JSON.stringify(normalizedParticipants) ===
            JSON.stringify(convParticipants)
          );
        });
        if (newConversation) {
          setSelectedConversation(newConversation);
        }
      }, 500);
    },
    [currentAccount, refreshConversations, conversations]
  );

  const handleSendFromConversation = useCallback(
    async (text: string, attachments?: ComposeAttachment[]) => {
      if (
        !selectedConversation ||
        (!text.trim() && (!attachments || attachments.length === 0))
      )
        return;

      // Check if readonly mode is enabled
      const isReadOnly = await getReadOnlyMode();
      if (isReadOnly) {
        alert(
          "Cannot send message: Eddie is currently in read-only mode.\n\n" +
          "To send messages, disable read-only mode in account settings."
        );
        return;
      }

      // Get all participants as recipients
      const to = selectedConversation.participants;

      // Extract first line as subject for new message style
      const lines = text.split("\n");
      const firstLine = lines[0].trim();
      const subject =
        firstLine || `Re: ${selectedConversation.last_message_preview || ""}`;
      const body =
        lines.length > 1 ? lines.slice(1).join("\n").trim() || text : text;

      // Use the new API with attachments support
      const result = await sendMessageWithAttachments(
        currentAccount || "user@example.com",
        to,
        subject,
        body,
        attachments || [],
        undefined,
        currentAccount || undefined
      );

      // Trigger sync to pull the sent message into local database
      if (result?.sent_folder) {
        await syncNow();
      }
      refreshConversations();
      refreshMessages();
    },
    [selectedConversation, currentAccount, refreshConversations, refreshMessages]
  );

  const handleEditAccount = useCallback(async () => {
    // Don't open if wizard is still open
    if (showSetupWizard || setupWizardOpen) {
      return;
    }

    if (!currentAccount) {
      // No active account - open the setup wizard
      setSetupWizardOpen(true);
      return;
    }

    try {
      const details = await getAccountDetails(currentAccount);

      // Ensure wizard is closed before opening config modal
      setSetupWizardOpen(false);
      setAccountEditData(details);
      setAccountModalOpen(true);
    } catch (err) {
      console.error("Failed to get account details:", err);
      const errorMessage = typeof err === 'object' && err !== null && 'message' in err
        ? String(err.message)
        : err instanceof Error
        ? err.message
        : String(err);
      alert(`Failed to load account details: ${errorMessage}`);
    }
  }, [currentAccount, showSetupWizard, setupWizardOpen]);

  const handleSaveAccount = async (data: SaveEmailAccountRequest) => {
    await saveAccount(data);
    await refreshAccounts();
    refreshConversations();
  };

  const handleDeleteAccount = async (accountName: string) => {
    await removeAccount(accountName);
    setSelectedConversation(null);
    setCurrentAccount(null);
    await refreshAccounts();
  };

  const handleCloseAccountModal = useCallback(() => {
    setAccountModalOpen(false);
    setAccountEditData(null);
  }, []);

  const handleCloseSetupWizard = useCallback(() => {
    setSetupWizardOpen(false);
  }, []);

  const handleSetupSuccess = useCallback(async () => {
    await refreshAccounts();
    // Start IMAP sync for the newly created account
    // Pass undefined to use the default account (the one we just created)
    await initSyncEngine(undefined);
    await refreshConversations();
  }, [refreshAccounts, refreshConversations]);

  const handleBack = useCallback(() => {
    setSelectedConversation(null);
    setIsComposing(false);
    setComposeParticipants([]);
  }, []);

  // Determine if sidebar should be hidden on mobile (when conversation is selected)
  const sidebarHidden = selectedConversation || isComposing;

  return (
    <main className="flex h-dvh max-h-dvh overflow-hidden">
      {/* Sidebar with chat list */}
      <aside
        className={`
          w-full bg-bg-secondary
          flex flex-col overflow-hidden
          absolute md:relative inset-0 z-50 md:z-auto
          ${isDragging ? "" : "transition-transform duration-250 ease-out"}
          h-full min-h-0
          ${sidebarHidden ? "-translate-x-full md:translate-x-0" : "translate-x-0"}
        `}
        style={isDesktop ? { width: sidebarWidth, minWidth: sidebarWidth } : undefined}
      >
        <SidebarHeader
          accounts={accounts}
          currentAccount={currentAccount}
          onEditAccount={handleEditAccount}
          onCompose={handleCompose}
        />

        {showInitialLoader ? (
          <InitialSyncLoader syncStatus={syncStatus} />
        ) : (
          <ChatMessages
            conversations={conversations}
            selectedId={selectedConversation?.id || null}
            onSelect={handleConversationSelect}
            loading={conversationsLoading}
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            currentAccountEmail={currentAccountEmail}
            activeFilter={activeFilter}
            onFilterChange={setActiveFilter}
          />
        )}
      </aside>

      <ResizeHandle onMouseDown={handleResizeMouseDown} isDragging={isDragging} />

      {/* Main conversation view */}
      <section className="flex-1 flex flex-col bg-bg-primary overflow-hidden h-full min-h-0">
        <ConversationView
          conversation={selectedConversation}
          messages={messages}
          loading={messagesLoading}
          error={messagesError}
          currentAccountEmail={currentAccountEmail}
          currentAccountAliases={currentAccountAliases}
          onSendMessage={handleSendFromConversation}
          onBack={handleBack}
          isComposing={isComposing}
          composeParticipants={composeParticipants}
          onComposeParticipantsConfirm={handleComposeParticipantsConfirm}
          onSendNewMessage={handleSendNewMessage}
        />
      </section>

      {/* Account Setup Wizard */}
      <AccountSetupWizard
        isOpen={showSetupWizard || setupWizardOpen}
        onClose={handleCloseSetupWizard}
        onSuccess={handleSetupSuccess}
      />

      {/* Account Config Modal (for editing existing accounts) */}
      <AccountConfigModal
        isOpen={accountModalOpen}
        onClose={handleCloseAccountModal}
        onSave={handleSaveAccount}
        onDelete={handleDeleteAccount}
        editData={accountEditData}
      />
    </main>
  );
}

export default App;
