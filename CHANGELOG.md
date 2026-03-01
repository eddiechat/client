# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.3] - 2026-03-01

### Changed
- Restyle login screen with new boxed logo and light mode default
- Use image icons in chat messages
- Use link badges instead of full links

## [0.3.2] - 2026-03-01

### Added
- Search across all points

### Changed
- Updated filter icons
- Locked screen orientation
- Decreased onboarding duration
- Added safe area to login and launcher screens

### Fixed
- Fixed splash screen
- Fixed icons
- Removed borders from launcher icon
- Fixed compose button spacing

## [0.3.1] - 2026-02-27

### Changed
- New app icon
- Increase item sizes by 20%
- Increase spacing on home screen by 10%
- Clean up settings

## [0.3.0] - 2026-02-27

### Added
- Last message prefix in conversation list
- Sunshine background to additional screens
- Settings accessible from toaster
- Groups menu item

### Changed
- Major UI facelift with new color palette
- Improved chat layout
- Improved avatar rendering and display
- Render first line of message instead of subject
- Short names in conversation list
- New pill logo in header
- New background styling
- Boxed groups layout
- Updated menu design
- Limit number of chats shown
- Removed background from search

### Fixed
- Onboarding background
- Avatar display in chat view
- Bottom safe area spacing

## [0.2.15] - 2026-02-24

### Added
- Show full message content with HTML rendering
- Skills reintroduced

## [0.2.14] - 2026-02-23

### Added
- Spec for skills processing

### Changed
- Cluster lines by sender instead of domain

## [0.2.13] - 2026-02-23

### Changed
- Run incremental sync on every tick so new mail arrives during onboarding
- Disable the connection history task
- Update docs

### Fixed
- Fix release skill to bump version in all config files

## [0.2.12] - 2026-02-22

### Fixed
- Error screen now shows "Go back" and "Go home" buttons so users are never stuck on a blank screen

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
