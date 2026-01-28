import { useState, useCallback } from "react";
import {
  AccountConfigModal,
  ChatList,
  ConversationView,
} from "./components";
import type { AccountEditData } from "./components";
import { useAccounts } from "./hooks/useEmail";
import {
  useConversations,
  useConversationMessages,
} from "./hooks/useConversations";
import * as api from "./lib/api";
import type { Conversation, SaveAccountRequest, ComposeAttachment } from "./types";
import { extractEmail } from "./lib/utils";
import "./App.css";

function App() {
  // Conversation selection state
  const [selectedConversation, setSelectedConversation] = useState<Conversation | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  // Compose mode state (messenger-style compose in chat view)
  const [isComposing, setIsComposing] = useState(false);
  const [composeParticipants, setComposeParticipants] = useState<string[]>([]);

  // Account config modal state
  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [accountEditData, setAccountEditData] = useState<AccountEditData | null>(null);

  // Hooks for data fetching
  const {
    accounts,
    currentAccount,
    setCurrentAccount,
    loading: accountsLoading,
    refresh: refreshAccounts,
  } = useAccounts();

  // Get current account email for determining message direction
  const currentAccountEmail = currentAccount || undefined;

  // Show config modal when no accounts are configured
  const showConfigModal = !accountsLoading && accounts.length === 0;

  // Conversations hook
  const {
    conversations,
    loading: conversationsLoading,
    refresh: refreshConversations,
  } = useConversations(currentAccount || undefined);

  // Messages for selected conversation
  const {
    messages,
    loading: messagesLoading,
    error: messagesError,
    refresh: refreshMessages,
  } = useConversationMessages(
    selectedConversation,
    currentAccount || undefined
  );

  // Handlers
  const handleConversationSelect = useCallback((conversation: Conversation) => {
    setSelectedConversation(conversation);
    setIsComposing(false);
    setComposeParticipants([]);

    // Mark messages as read when opening conversation
    const cachedId = (conversation as { _cached_id?: number })._cached_id;
    if (cachedId !== undefined && conversation.unread_count > 0) {
      api.markConversationRead(cachedId, currentAccount || undefined).catch((err) => {
        console.error("Failed to mark conversation as read:", err);
      });
    }
  }, [currentAccount]);

  const handleCompose = useCallback(() => {
    setSelectedConversation(null);
    setIsComposing(true);
    setComposeParticipants([]);
  }, []);

  // Handle when participants are confirmed in compose mode
  const handleComposeParticipantsConfirm = useCallback((participants: string[]) => {
    // Try to find existing conversation with these participants
    const normalizedParticipants = participants.map(p => extractEmail(p).toLowerCase()).sort();

    const existingConversation = conversations.find(conv => {
      const convParticipants = conv.participants
        .map(p => extractEmail(p).toLowerCase())
        .sort();

      // Check if participants match (excluding current user)
      return JSON.stringify(normalizedParticipants) === JSON.stringify(convParticipants);
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
  }, [conversations]);

  // Handle sending a new message in compose mode (no existing conversation)
  const handleSendNewMessage = useCallback(
    async (text: string, participants: string[], attachments?: ComposeAttachment[]) => {
      if ((!text.trim() && (!attachments || attachments.length === 0)) || participants.length === 0) return;

      // Extract first line as subject
      const lines = text.split('\n');
      const subject = lines[0].trim() || '(No subject)';
      const body = lines.length > 1 ? lines.slice(1).join('\n').trim() || lines[0] : text;

      // Use the new API if we have attachments, otherwise use the legacy API for compatibility
      const result = await api.sendMessageWithAttachments(
        currentAccount || "user@example.com",
        participants,
        subject,
        body,
        attachments || [],
        undefined,
        currentAccount || undefined
      );

      // Sync the sent folder to pull the message into local database
      if (result?.sent_folder) {
        await api.syncFolder(result.sent_folder, currentAccount || undefined);
      }

      // Exit compose mode and refresh
      setIsComposing(false);
      setComposeParticipants([]);
      await refreshConversations();

      // Try to select the newly created conversation
      const normalizedParticipants = participants.map(p => extractEmail(p).toLowerCase()).sort();
      setTimeout(() => {
        const newConversation = conversations.find(conv => {
          const convParticipants = conv.participants
            .map(p => extractEmail(p).toLowerCase())
            .sort();
          return JSON.stringify(normalizedParticipants) === JSON.stringify(convParticipants);
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
      if (!selectedConversation || (!text.trim() && (!attachments || attachments.length === 0))) return;

      // Get all recipients (all participants except current user)
      const recipients = selectedConversation.participants.filter(
        (p) => !extractEmail(p).toLowerCase().includes((currentAccount || "").toLowerCase())
      );

      // If no recipients found, use first participant
      const to = recipients.length > 0 ? recipients : [selectedConversation.participants[0]];

      // Extract first line as subject for new message style
      const lines = text.split('\n');
      const firstLine = lines[0].trim();
      const subject = firstLine || `Re: ${selectedConversation.last_message_preview}`;
      const body = lines.length > 1 ? lines.slice(1).join('\n').trim() || text : text;

      // Use the new API with attachments support
      const result = await api.sendMessageWithAttachments(
        currentAccount || "user@example.com",
        to,
        subject,
        body,
        attachments || [],
        undefined,
        currentAccount || undefined
      );

      // Sync the sent folder to pull the message into local database
      if (result?.sent_folder) {
        await api.syncFolder(result.sent_folder, currentAccount || undefined);
      }
      refreshConversations();
      refreshMessages();
    },
    [selectedConversation, currentAccount, refreshConversations, refreshMessages]
  );

  const handleEditAccount = useCallback(async () => {
    if (!currentAccount) {
      // No active account - just open the modal to allow account configuration
      setAccountEditData(null);
      setAccountModalOpen(true);
      return;
    }

    try {
      const details = await api.getAccountDetails(currentAccount);
      setAccountEditData(details);
      setAccountModalOpen(true);
    } catch (err) {
      console.error("Failed to get account details:", err);
      alert(`Failed to load account details: ${err}`);
    }
  }, [currentAccount]);

  const handleSaveAccount = async (data: SaveAccountRequest) => {
    await api.saveAccount(data);
    await refreshAccounts();
    refreshConversations();
  };

  const handleDeleteAccount = async (accountName: string) => {
    await api.removeAccount(accountName);
    setSelectedConversation(null);
    setCurrentAccount(null);
    await refreshAccounts();
  };

  const handleCloseAccountModal = useCallback(() => {
    setAccountModalOpen(false);
    setAccountEditData(null);
  }, []);

  const handleBack = useCallback(() => {
    setSelectedConversation(null);
    setIsComposing(false);
    setComposeParticipants([]);
  }, []);

  return (
    <main className="app">
      {/* Sidebar with chat list */}
      <aside className={`sidebar${selectedConversation ? ' hidden' : ''}`}>
        <div className="sidebar-header">
          <div className="sidebar-title">
            <div className="sidebar-brand">
              <img src="/eddie-swirl-green.svg" alt="Eddie logo" className="sidebar-logo" />
              <h1>eddie</h1>
            </div>
            {accounts.length > 0 && (
              <span className="account-badge" onClick={handleEditAccount}>
                {currentAccount || "No account"}
              </span>
            )}
          </div>
          <button className="new-message-btn" onClick={handleCompose} title="New message">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
              <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
            </svg>
          </button>
        </div>

        <ChatList
          conversations={conversations}
          selectedId={selectedConversation?.id || null}
          onSelect={handleConversationSelect}
          loading={conversationsLoading}
          searchQuery={searchQuery}
          onSearchChange={setSearchQuery}
          currentAccountEmail={currentAccountEmail}
        />
      </aside>

      {/* Main conversation view */}
      <section className="main-panel">
        <ConversationView
          conversation={selectedConversation}
          messages={messages}
          loading={messagesLoading}
          error={messagesError}
          currentAccountEmail={currentAccountEmail}
          onSendMessage={handleSendFromConversation}
          onBack={handleBack}
          isComposing={isComposing}
          composeParticipants={composeParticipants}
          onComposeParticipantsConfirm={handleComposeParticipantsConfirm}
          onSendNewMessage={handleSendNewMessage}
        />
      </section>

      {/* Account Config Modal */}
      <AccountConfigModal
        isOpen={showConfigModal || accountModalOpen}
        onClose={handleCloseAccountModal}
        onSave={handleSaveAccount}
        onDelete={handleDeleteAccount}
        editData={accountEditData}
      />
    </main>
  );
}

export default App;
