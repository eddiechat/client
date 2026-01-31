# Vendored Dependencies

This directory contains vendored copies of upstream dependencies that require patches or modifications for Eddie.

## pimalaya-core

**Source**: https://github.com/pimalaya/core
**Vendored Commit**: c36dd7c5 (2024-01-31)
**Method**: git subtree

The pimalaya-core repository is a monorepo containing multiple crates:
- `email` - Email protocol library (IMAP, SMTP, etc.)
- `secret` - Secret management
- `mml` - MIME Meta Language
- `process` - Process management
- And others

We vendor the entire workspace to maintain consistency between related crates.

### Updating from Upstream

To pull latest changes from upstream:

```bash
git subtree pull --prefix=src-tauri/vendor/pimalaya-core \
  https://github.com/pimalaya/core.git master --squash
```

After pulling, review and re-apply patches documented in the `patches/` directory.

### Our Modifications

See the `patches/` directory for detailed documentation of each patch:

1. **CC Field Support** (`patches/001-envelope-cc-field.md`)
   - Files: `email/src/email/envelope/mod.rs`, `email/src/email/envelope/imap.rs`
   - Adds CC field extraction to Envelope struct for IMAP and message parsing

### Diffing Against Upstream

To see our changes vs upstream:

```bash
cd src-tauri/vendor/pimalaya-core
git diff c36dd7c5 -- email/src/email/envelope/
```

Or to compare the current vendored version with a specific upstream commit:

```bash
# Fetch the latest from upstream
git fetch https://github.com/pimalaya/core.git master

# Diff against latest upstream
git diff FETCH_HEAD -- email/
```

### Contributing Patches Upstream

If our patches would benefit the upstream project:

1. Fork https://github.com/pimalaya/core
2. Create a branch with our changes
3. Submit a PR to pimalaya/core
4. If merged, we can drop our local patch in future updates
