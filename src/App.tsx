import { useState, useCallback } from "react";
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
  useConversations,
  useConversationMessages,
} from "./features/conversations";
import { ContactsList, useContacts } from "./features/contacts";
import {
  saveAccount,
  removeAccount,
  getAccountDetails,
  markConversationRead,
  sendMessageWithAttachments,
  syncFolder,
} from "./tauri";
import type { Conversation, SaveEmailAccountRequest, ComposeAttachment, Contact } from "./tauri";
import { extractEmail } from "./shared";
import "./App.css";

type SidebarTab = "messages" | "contacts";

function App() {
  // Sidebar tab state
  const [activeTab, setActiveTab] = useState<SidebarTab>("messages");

  // Conversation selection state
  const [selectedConversation, setSelectedConversation] =
    useState<Conversation | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  // Contact selection state (for future contact detail view)
  const [, setSelectedContact] = useState<Contact | null>(null);

  // Compose mode state (messenger-style compose in chat view)
  const [isComposing, setIsComposing] = useState(false);
  const [composeParticipants, setComposeParticipants] = useState<string[]>([]);

  // Account config modal state
  const [accountModalOpen, setAccountModalOpen] = useState(false);
  const [accountEditData, setAccountEditData] =
    useState<AccountEditData | null>(null);

  // Account setup wizard state
  const [setupWizardOpen, setSetupWizardOpen] = useState(false);

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

  // Show setup wizard when no accounts are configured
  const showSetupWizard = !accountsLoading && accounts.length === 0;

  // Conversations hook
  const {
    conversations,
    loading: conversationsLoading,
    refresh: refreshConversations,
  } = useConversations(currentAccount || undefined);

  // Contacts hook
  const {
    contacts,
    loading: contactsLoading,
    error: contactsError,
    hasCardDAV,
    refresh: refreshContacts,
  } = useContacts(currentAccount || undefined);

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
      const cachedId = conversation._cached_id;
      if (cachedId !== undefined && conversation.unread_count > 0) {
        markConversationRead(cachedId, currentAccount || undefined).catch(
          (err) => {
            console.error("Failed to mark conversation as read:", err);
          }
        );
      }
    },
    [currentAccount]
  );

  const handleCompose = useCallback((initialRecipients?: string[]) => {
    setSelectedConversation(null);
    setIsComposing(true);
    setComposeParticipants(initialRecipients || []);
    setActiveTab("messages"); // Switch to messages tab when composing
  }, []);

  // Handle when participants are confirmed in compose mode
  const handleComposeParticipantsConfirm = useCallback(
    (participants: string[]) => {
      // Try to find existing conversation with these participants
      const normalizedParticipants = participants
        .map((p) => extractEmail(p).toLowerCase())
        .sort();

      const existingConversation = conversations.find((conv) => {
        const convParticipants = conv.participants
          .map((p) => extractEmail(p).toLowerCase())
          .sort();

        // Check if participants match (excluding current user)
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
    [conversations]
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

      // Sync the sent folder to pull the message into local database
      if (result?.sent_folder) {
        await syncFolder(result.sent_folder, currentAccount || undefined);
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

      // Get all recipients (all participants except current user)
      const recipients = selectedConversation.participants.filter(
        (p) =>
          !extractEmail(p)
            .toLowerCase()
            .includes((currentAccount || "").toLowerCase())
      );

      // If no recipients found, use first participant
      const to =
        recipients.length > 0
          ? recipients
          : [selectedConversation.participants[0]];

      // Extract first line as subject for new message style
      const lines = text.split("\n");
      const firstLine = lines[0].trim();
      const subject =
        firstLine || `Re: ${selectedConversation.last_message_preview}`;
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

      // Sync the sent folder to pull the message into local database
      if (result?.sent_folder) {
        await syncFolder(result.sent_folder, currentAccount || undefined);
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
      alert(`Failed to load account details: ${err}`);
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
    await refreshConversations();
  }, [refreshAccounts, refreshConversations]);

  const handleBack = useCallback(() => {
    setSelectedConversation(null);
    setIsComposing(false);
    setComposeParticipants([]);
  }, []);

  // Contact handlers
  const handleContactSelect = useCallback((contact: Contact) => {
    setSelectedContact(contact);
  }, []);

  const handleStartEmailToContact = useCallback((email: string) => {
    handleCompose([email]);
  }, [handleCompose]);

  // Determine if sidebar should be hidden on mobile (when conversation is selected)
  const sidebarHidden = selectedConversation || isComposing;

  return (
    <main className="flex h-dvh max-h-dvh overflow-hidden">
      {/* Sidebar with chat list or contacts */}
      <aside
        className={`
          w-full md:w-80 md:min-w-80 bg-bg-secondary border-r border-divider
          flex flex-col overflow-hidden
          absolute md:relative inset-0 z-50 md:z-auto
          transition-transform duration-250 ease-out
          h-full min-h-0
          ${sidebarHidden ? "-translate-x-full md:translate-x-0" : "translate-x-0"}
        `}
      >
        <SidebarHeader
          accounts={accounts}
          currentAccount={currentAccount}
          onEditAccount={handleEditAccount}
          onCompose={() => handleCompose()}
        />

        {/* Tab buttons */}
        <div className="flex border-b border-divider">
          <button
            className={`flex-1 flex items-center justify-center gap-2 py-2 px-4 text-sm font-medium transition-colors ${
              activeTab === "messages"
                ? "text-accent border-b-2 border-accent"
                : "text-text-secondary hover:text-text-primary"
            }`}
            onClick={() => setActiveTab("messages")}
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
            </svg>
            Messages
          </button>
          <button
            className={`flex-1 flex items-center justify-center gap-2 py-2 px-4 text-sm font-medium transition-colors ${
              activeTab === "contacts"
                ? "text-accent border-b-2 border-accent"
                : "text-text-secondary hover:text-text-primary"
            }`}
            onClick={() => setActiveTab("contacts")}
          >
            <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M17 21v-2a4 4 0 0 0-4-4H5a4 4 0 0 0-4 4v2" />
              <circle cx="9" cy="7" r="4" />
              <path d="M23 21v-2a4 4 0 0 0-3-3.87" />
              <path d="M16 3.13a4 4 0 0 1 0 7.75" />
            </svg>
            Contacts
          </button>
        </div>

        {/* Conditionally render based on active tab */}
        {activeTab === "messages" ? (
          <ChatMessages
            conversations={conversations}
            selectedId={selectedConversation?.id || null}
            onSelect={handleConversationSelect}
            loading={conversationsLoading}
            searchQuery={searchQuery}
            onSearchChange={setSearchQuery}
            currentAccountEmail={currentAccountEmail}
          />
        ) : (
          <ContactsList
            contacts={contacts}
            loading={contactsLoading}
            error={contactsError}
            hasCardDAV={hasCardDAV}
            onSelectContact={handleContactSelect}
            onStartEmail={handleStartEmailToContact}
            onRefresh={refreshContacts}
          />
        )}
      </aside>

      {/* Main conversation view */}
      <section className="flex-1 flex flex-col bg-bg-primary overflow-hidden h-full min-h-0">
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
