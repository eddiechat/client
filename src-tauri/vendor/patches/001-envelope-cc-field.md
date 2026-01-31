# Patch 001: Add CC Field Support to Envelope

## Summary

Adds CC (carbon copy) field extraction to the Envelope struct for both full message parsing and IMAP envelope parsing.

## Upstream Status

Not submitted upstream. Consider submitting as enhancement PR to https://github.com/pimalaya/core.

## Files Modified

- `email/src/email/envelope/mod.rs`:
  - Add `cc: Vec<Address>` field to Envelope struct
  - Initialize field in `from_msg` function
  - Add CC parsing logic after TO parsing

- `email/src/email/envelope/imap.rs`:
  - Extract CC addresses from IMAP ENVELOPE response
  - Format CC header similar to FROM/TO handling

## Rationale

Eddie requires CC field information for proper conversation display and threading. The IMAP ENVELOPE response includes CC data according to RFC 3501, but email-lib was not extracting it.

Previously, the eddie-client backend had:
```rust
cc: vec![], // TODO: email-lib doesn't expose CC in envelope list
```

With this patch, CC recipients are now properly extracted from both:
1. Full message parsing (via mail_parser's `msg.cc()` method)
2. IMAP envelope responses (via IMAP envelope's `cc` field)

## Backwards Compatibility

This is a backwards-compatible addition. The new `cc` field is:
- Initialized as empty `Vec::new()`
- Populated automatically when CC data is available
- Existing code continues to work without changes

## Technical Details

### Envelope Struct Change

```rust
pub struct Envelope {
    // ... existing fields ...
    pub from: Address,
    pub to: Address,
    pub cc: Vec<Address>,  // NEW
    pub subject: String,
    // ... rest ...
}
```

### Message Parsing (mod.rs)

Extracts CC from parsed message headers using mail_parser:

```rust
match msg.cc() {
    Some(mail_parser::Address::List(addrs)) => { /* extract */ }
    Some(mail_parser::Address::Group(groups)) => { /* extract */ }
    _ => { /* log and skip */ }
}
```

### IMAP Envelope Parsing (imap.rs)

Constructs CC header from IMAP envelope data:

```rust
let cc = envelope
    .cc
    .iter()
    .filter_map(|imap_addr| {
        // Format: "Name" <email@host>
    })
    .fold(b"Cc: ".to_vec(), |mut addrs, addr| {
        // Comma-separated list
    });
```

## Testing

After applying this patch:

1. Fetch envelopes from IMAP server
2. Verify CC field is populated for emails with CC recipients
3. Verify empty vector for emails without CC
4. Test with multiple CC recipients
5. Verify reply/forward functionality still works

## Future Considerations

- BCC field extraction (though typically not available in envelope responses for privacy)
- TO field conversion to Vec<Address> for consistency (currently only first recipient)
