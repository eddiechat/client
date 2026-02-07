# Sync Engine Requirements

## Context
The sync engine is at the heart of a messaging app similar to FB Messenger, WhatsApp, Signal etc., but running on top of email protocols.

The main differences to traditional email clients are that:
- Messages are grouped by participants, rather than subject
- Messages are rendered as short-form chat messages, with the core message extracted and the ability to click the chat message to view the full message

A core principle is to be 100% privacy-centric and not add any servers. Data should only ever be touched by the user's devices and email server.

Most users are expected to run the app on several devices, which means we either have to reprocess data on every device, store data by adding metadata to the actual messages on the email server through IMAP, or store data as a draft message on the email server, which can then be shared between devices.

## The Sync Engine
The app has a sync engine on top of a SQLite database that pulls data using IMAP and persists it locally.
The database is considered a cache, and the email server is considered the ultimate single source of truth.

Besides storing copies of the messages, the messages are:
- Classified with a string label, indicating whether they are from what seems to be an actual person (label = "chat") or some automated sender (newsletters, promotions, updates, transactional, etc.)
- Classified as important, based on either a metadata field or a classifier
- Distilled into a short chat message, usually consisting of the top of the message, after the initial hello and before a signature and the included message history

A trust network of conversation participants (entities) is also stored, with a classification of:
- user, the actual email address of the user
- alias, alternative email addresses of the user
- contact, information pulled using CardDAV (optional)
- connection, if the user has sent at least one message to that participant in the past, from either the actual email address or an alias

If a participant is not in this list, they are considered an outsider, someone the user does not know.

Based on the classification and trust network, messages are grouped into conversations that can be filtered by a derived conversation classification:
- "Connections": conversations where at least one message is classified as "chat" and at least one message is from someone in the trust network
- "Others": all the other messages that are classified as "chat", mostly unsolicited outreach
- "Important": messages classified as important
- "All": unfiltered

## Ingestion
- During onboarding, the full message history is fetched from all folders except spam, based on which the trust network is built in the database.
- Messages from the last 12 months are stored in the database, with all metadata as well as generated data.
- Each message is assigned a participant key, which is a sanitized, sorted list of all participant email addresses, except the user's email or aliases. This key is additionally hashed into a conversation ID.
- Each conversation is stored in the database and extended with a derived classification, and those that are classified as "Connections" are expanded with all messages from that participant group since the beginning of time.

## Challenges
The user should experience as little latency as possible.

This implies:
- Rendering a preliminary list of conversations as early as possible, and rearranging it as more data are being fetched and processed
- Enriching message metadata on the email server with processing outcomes when possible, to avoid reprocessing on multiple devices
- Running expensive processing using AI primarily on desktop devices
- Caching what cannot be meaningfully stored as metadata on messages in a draft system message
- Queuing up client actions for background processing and applying them optimistically for instant appearance. If they fail when applied to the server, the optimistic update should be modified to reflect this