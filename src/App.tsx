import { useState, useCallback } from "react";
import {
  AccountConfigModal,
  ChatList,
  ComposeModal,
  ConversationView,
} from "./components";
import type { AccountEditData } from "./components";
import { useAccounts } from "./hooks/useEmail";
import {
  useConversations,
  useConversationMessages,
} from "./hooks/useConversations";
import * as api from "./lib/api";
import type { Conversation, ComposeMessageData, SaveAccountRequest } from "./types";
import "./App.css";

function App() {
  // Conversation selection state
  const [selectedConversation, setSelectedConversation] = useState<Conversation | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  // Compose modal state
  const [composeOpen, setComposeOpen] = useState(false);
  const [composeMode, setComposeMode] = useState<"new" | "reply" | "forward">("new");
  const [composeInitialData, setComposeInitialData] = useState<Partial<ComposeMessageData>>({});

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
  } = useConversationMessages(
    selectedConversation?.message_ids || [],
    currentAccount || undefined
  );

  // Handlers
  const handleConversationSelect = useCallback((conversation: Conversation) => {
    setSelectedConversation(conversation);
  }, []);

  const handleCompose = useCallback(() => {
    setComposeMode("new");
    setComposeInitialData({});
    setComposeOpen(true);
  }, []);

  const handleSendMessage = async (data: ComposeMessageData) => {
    const headers = [
      `From: ${data.from || currentAccount || "user@example.com"}`,
      `To: ${data.to.join(", ")}`,
      data.cc?.length ? `Cc: ${data.cc.join(", ")}` : "",
      `Subject: ${data.subject}`,
      `Date: ${new Date().toUTCString()}`,
      data.in_reply_to ? `In-Reply-To: ${data.in_reply_to}` : "",
      "MIME-Version: 1.0",
      "Content-Type: text/plain; charset=utf-8",
    ]
      .filter(Boolean)
      .join("\r\n");

    const rawMessage = `${headers}\r\n\r\n${data.body}`;
    await api.sendMessage(rawMessage, currentAccount || undefined);
    refreshConversations();
  };

  const handleSaveDraft = async (data: ComposeMessageData) => {
    const headers = [
      `From: ${data.from || currentAccount || "user@example.com"}`,
      `To: ${data.to.join(", ")}`,
      data.cc?.length ? `Cc: ${data.cc.join(", ")}` : "",
      `Subject: ${data.subject}`,
      "MIME-Version: 1.0",
      "Content-Type: text/plain; charset=utf-8",
    ]
      .filter(Boolean)
      .join("\r\n");

    const rawMessage = `${headers}\r\n\r\n${data.body}`;
    await api.saveMessage(rawMessage, "Drafts", currentAccount || undefined);
  };

  const handleSendFromConversation = useCallback(
    async (text: string) => {
      if (!selectedConversation || !text.trim()) return;

      // Get the recipient (first participant that's not the current user)
      const recipient =
        selectedConversation.participants.find(
          (p) => !p.includes(currentAccount || "")
        ) || selectedConversation.participants[0];

      const subject = `Re: ${selectedConversation.last_message_preview}`;

      const headers = [
        `From: ${currentAccount || "user@example.com"}`,
        `To: ${recipient}`,
        `Subject: ${subject}`,
        `Date: ${new Date().toUTCString()}`,
        "MIME-Version: 1.0",
        "Content-Type: text/plain; charset=utf-8",
      ].join("\r\n");

      const rawMessage = `${headers}\r\n\r\n${text}`;
      await api.sendMessage(rawMessage, currentAccount || undefined);
      refreshConversations();
    },
    [selectedConversation, currentAccount, refreshConversations]
  );

  const handleEditAccount = useCallback(async () => {
    if (!currentAccount) return;

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
  }, []);

  return (
    <main className="app">
      {/* Sidebar with chat list */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="sidebar-title">
            <h1>eddie</h1>
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
        />
      </section>

      {/* Compose Modal */}
      <ComposeModal
        isOpen={composeOpen}
        onClose={() => setComposeOpen(false)}
        onSend={handleSendMessage}
        onSaveDraft={handleSaveDraft}
        initialData={composeInitialData}
        mode={composeMode}
      />

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
