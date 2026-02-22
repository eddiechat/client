# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.11] - 2026-02-22

### Fixed
- Tolerant IMAP response parsing — messages with unparseable BODYSTRUCTURE (e.g., non-ASCII literal filenames) are now skipped instead of failing the entire sync batch

## [0.2.10] - 2026-02-22

### Added
- Add fallback for RFC 6154 folder types

### Changed
- Update trust network
- Increase size of back button
- Improve logging
- Cleanup

### Fixed
- Fallback for missing sent folder
- Fix overflow

## [0.2.9] - 2026-02-22

### Fixed
- Fix Sentry tracing not sending events in production builds by switching transport from native-tls to rustls

## [0.2.8] - 2026-02-22

### Fixed
- Move Sentry initialization inside the setup function

## [0.2.7] - 2026-02-22

### Changed
- Batched ingestion of sent messages for building the trust network

## [0.2.6] - 2026-02-22

### Fixed
- Fix Sentry logging not working on mobile (tracing init moved to shared entry point)

## [0.2.5] - 2026-02-22

### Changed
- Improved logging across sync engine tasks (historical fetch, connection history, incremental sync)
- Format elapsed time in log statements as whole milliseconds
- Sentry integration improvements

## [0.2.4] - 2026-02-21

### Changed
- Increased text and avatar sizes across the app for better phone readability
- Notification toaster now only shows in dev mode

## [0.2.3] - 2026-02-21

### Added
- Gravatar support for contact avatars

### Changed
- Remove entity feature

### Fixed
- Android build fix

## [0.2.2] - 2026-02-21

### Added
- Dark mode support
- Version display in the app

### Changed
- Applied safe area insets in skills views

### Fixed
- Android build issue
- Removed exposed API key

## [0.2.1] - 2026-02-17

### Fixed
- Rollback TLS Android fix

## [0.2.0] - 2026-02-17

### Changed
- Migrate to v0.2 architecture
- Swap async-native-tls with tokio-rustls for TLS connections
- Safe area support

### Fixed
- Fix bottom margin in chat view

## [0.1.8] - 2026-02-09

### Added
- Resizable sidebar on desktop
- Database diagram documentation
- Sync engine specification and requirements

### Fixed
- Duplicated ingestion spinner

## [0.1.7] - 2026-02-06

### Changed
- Documentation updates

### Fixed
- Build error in iOS

## [0.1.6] - 2026-02-06

### Added
- Read-only mode (enabled by default)
- Notification when attempting to send messages in read-only mode
- Onboarding messages during initial email ingestion

## [0.1.5] - 2026-02-04

### Fixed
- Fix version header on mobile
- Add missing macOS application identifier

## [0.1.4] - 2026-02-04

### Changed
- Show version in title
- Clear verbose logging

## [0.1.3] - 2026-02-04

### Fixed
- Add mobile fallback for missing hardware device id
- Fix loading of conversation when using compose button

## [0.1.2] - 2026-02-04

### Fixed
- Fix aliased sender bubbles
- Fix gravatars
- Fix build script
- Remove background from gravatars

### Added
- Add support for aliases
- Filter connections/others on is_connection
- Add entities table for participant tracking and recipient autocomplete

## [0.1.1] - 2026-02-03

### Fixed
- Fix Android build
- Fix sync start after onboarding
- Fix initial poll
- Fix header and title rendering
- Fix iOS TestFlight build

### Added
- Conversation classification and filtering
- Participant rendering improvements
- Improve fetching mechanism
- Simplify avatar filtering and increase limit

### Changed
- Re-enable disabled jobs in CI
- Update iOS initialize and build jobs
- Update app name
- Truncate chat message content to a max of 20 lines
- Use default xcode runner in CI
