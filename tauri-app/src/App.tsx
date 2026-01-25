import { useState, useCallback } from "react";
import {
  AccountConfigModal,
  AccountSelector,
  ComposeModal,
  EnvelopeList,
  FolderList,
  MessageView,
} from "./components";
import type { AccountEditData } from "./components";
import {
  useAccounts,
  useFolders,
  useEnvelopes,
  useMessage,
  useEmailActions,
} from "./hooks/useEmail";
import * as api from "./lib/api";
import type { Envelope, ComposeMessageData, SaveAccountRequest } from "./types";
import "./App.css";

function App() {
  const [selectedEnvelopeId, setSelectedEnvelopeId] = useState<string | null>(null);
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

  // Show config modal when no accounts are configured
  const showConfigModal = !accountsLoading && accounts.length === 0;

  const {
    folders,
    currentFolder,
    setCurrentFolder,
    loading: foldersLoading,
    error: foldersError,
    refresh: refreshFolders,
  } = useFolders(currentAccount || undefined);

  const {
    envelopes,
    loading: envelopesLoading,
    error: envelopesError,
    refresh: refreshEnvelopes,
    query,
    setQuery,
    page,
    setPage,
  } = useEnvelopes(currentAccount || undefined, currentFolder);

  const {
    message,
    loading: messageLoading,
    error: messageError,
  } = useMessage(selectedEnvelopeId, currentAccount || undefined, currentFolder);

  const actions = useEmailActions(
    currentAccount || undefined,
    currentFolder,
    refreshEnvelopes
  );

  // Reset UI state
  const resetUIState = useCallback(() => {
    setSelectedEnvelopeId(null);
    setQuery("");
    setPage(1);
  }, [setQuery, setPage]);

  // Handlers
  const handleEnvelopeSelect = useCallback((envelope: Envelope) => {
    setSelectedEnvelopeId(envelope.id);
  }, []);

  const handleCloseMessage = useCallback(() => {
    setSelectedEnvelopeId(null);
  }, []);

  const handleToggleFlag = useCallback(
    (id: string, isFlagged: boolean) => {
      actions.toggleFlagged(id, isFlagged);
    },
    [actions]
  );

  const handleDelete = useCallback(async () => {
    if (selectedEnvelopeId) {
      await actions.deleteMessages([selectedEnvelopeId]);
      setSelectedEnvelopeId(null);
    }
  }, [selectedEnvelopeId, actions]);

  const handleReply = useCallback(() => {
    if (message) {
      setComposeMode("reply");
      setComposeInitialData({
        to: [message.envelope.from],
        subject: `Re: ${message.envelope.subject}`,
        body: `\n\n--- Original Message ---\n${message.text_body || ""}`,
        in_reply_to: message.envelope.message_id,
      });
      setComposeOpen(true);
    }
  }, [message]);

  const handleForward = useCallback(() => {
    if (message) {
      setComposeMode("forward");
      setComposeInitialData({
        subject: `Fwd: ${message.envelope.subject}`,
        body: `\n\n--- Forwarded Message ---\n${message.text_body || ""}`,
      });
      setComposeOpen(true);
    }
  }, [message]);

  const handleCompose = useCallback(() => {
    setComposeMode("new");
    setComposeInitialData({});
    setComposeOpen(true);
  }, []);

  const handleSendMessage = async (data: ComposeMessageData) => {
    // Build RFC 822 message (simplified)
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
    // Reset UI and refresh data
    resetUIState();
    await refreshAccounts();
    await refreshFolders();
    await refreshEnvelopes();
  };

  const handleDeleteAccount = async (accountName: string) => {
    await api.removeAccount(accountName);
    // Reset UI and refresh accounts
    resetUIState();
    setCurrentAccount(null);
    await refreshAccounts();
  };

  const handleCloseAccountModal = useCallback(() => {
    setAccountModalOpen(false);
    setAccountEditData(null);
  }, []);

  return (
    <main className="app">
      <header className="app-header">
        <h1>Himalaya</h1>
        <AccountSelector
          accounts={accounts}
          currentAccount={currentAccount}
          onAccountChange={setCurrentAccount}
          onEditAccount={handleEditAccount}
          loading={accountsLoading}
        />
        <button className="compose-btn" onClick={handleCompose}>
          Compose
        </button>
      </header>

      <div className="app-content">
        <aside className="sidebar">
          {foldersError && (
            <div className="error-banner">{foldersError}</div>
          )}
          <FolderList
            folders={folders}
            currentFolder={currentFolder}
            onFolderSelect={setCurrentFolder}
            loading={foldersLoading}
          />
        </aside>

        <section className="main-panel">
          <div className="toolbar">
            <input
              type="search"
              placeholder="Search emails..."
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="search-input"
            />
            <div className="pagination">
              <button onClick={() => setPage(Math.max(1, page - 1))} disabled={page <= 1}>
                Prev
              </button>
              <span>Page {page}</span>
              <button onClick={() => setPage(page + 1)}>Next</button>
            </div>
          </div>

          <div className="content-area">
            {envelopesError && (
              <div className="error-banner">{envelopesError}</div>
            )}
            {selectedEnvelopeId ? (
              <MessageView
                message={message}
                loading={messageLoading}
                error={messageError}
                onClose={handleCloseMessage}
                onDelete={handleDelete}
                onReply={handleReply}
                onForward={handleForward}
                onDownloadAttachments={() =>
                  selectedEnvelopeId && actions.downloadAttachments(selectedEnvelopeId)
                }
              />
            ) : (
              <EnvelopeList
                envelopes={envelopes}
                selectedId={selectedEnvelopeId}
                onSelect={handleEnvelopeSelect}
                onToggleFlag={handleToggleFlag}
                loading={envelopesLoading}
              />
            )}
          </div>
        </section>
      </div>

      <ComposeModal
        isOpen={composeOpen}
        onClose={() => setComposeOpen(false)}
        onSend={handleSendMessage}
        onSaveDraft={handleSaveDraft}
        initialData={composeInitialData}
        mode={composeMode}
      />

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
