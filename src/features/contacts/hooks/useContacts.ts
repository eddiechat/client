import { useState, useEffect, useCallback } from "react";
import * as commands from "../../../tauri/commands";
import type { Contact, AddressBook, SaveContactRequest } from "../../../tauri/types";

/**
 * Hook for managing contacts from CardDAV.
 */
export function useContacts(account?: string) {
  const [contacts, setContacts] = useState<Contact[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [hasCardDAV, setHasCardDAV] = useState(false);

  const checkCardDAV = useCallback(async () => {
    try {
      const hasConfig = await commands.hasCardDAVConfig(account);
      setHasCardDAV(hasConfig);
      return hasConfig;
    } catch {
      setHasCardDAV(false);
      return false;
    }
  }, [account]);

  const fetchContacts = useCallback(async () => {
    try {
      setLoading(true);
      const hasConfig = await checkCardDAV();
      if (!hasConfig) {
        setContacts([]);
        setError(null);
        return;
      }
      const contactList = await commands.listContacts(account);
      // Sort contacts alphabetically by full name
      contactList.sort((a, b) => a.full_name.localeCompare(b.full_name));
      setContacts(contactList);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [account, checkCardDAV]);

  useEffect(() => {
    fetchContacts();
  }, [fetchContacts]);

  const createContact = useCallback(
    async (contact: Contact) => {
      const request: SaveContactRequest = { account, contact };
      const created = await commands.createContact(request);
      setContacts((prev) =>
        [...prev, created].sort((a, b) => a.full_name.localeCompare(b.full_name))
      );
      return created;
    },
    [account]
  );

  const updateContact = useCallback(
    async (contact: Contact) => {
      const request: SaveContactRequest = { account, contact };
      const updated = await commands.updateContact(request);
      setContacts((prev) =>
        prev
          .map((c) => (c.id === updated.id ? updated : c))
          .sort((a, b) => a.full_name.localeCompare(b.full_name))
      );
      return updated;
    },
    [account]
  );

  const deleteContact = useCallback(
    async (contactId: string, href?: string) => {
      await commands.deleteContact(contactId, href, account);
      setContacts((prev) => prev.filter((c) => c.id !== contactId));
    },
    [account]
  );

  const getContact = useCallback(
    async (contactId: string) => {
      return commands.getContact(contactId, account);
    },
    [account]
  );

  return {
    contacts,
    loading,
    error,
    hasCardDAV,
    refresh: fetchContacts,
    createContact,
    updateContact,
    deleteContact,
    getContact,
  };
}

/**
 * Hook for listing address books.
 */
export function useAddressBooks(account?: string) {
  const [addressBooks, setAddressBooks] = useState<AddressBook[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAddressBooks = useCallback(async () => {
    try {
      setLoading(true);
      const books = await commands.listAddressBooks(account);
      setAddressBooks(books);
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }, [account]);

  useEffect(() => {
    fetchAddressBooks();
  }, [fetchAddressBooks]);

  return {
    addressBooks,
    loading,
    error,
    refresh: fetchAddressBooks,
  };
}
