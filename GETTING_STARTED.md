# Getting Started with NCTS Syndication App

> **Update**: The app now uses the correct NCTS API v1 endpoint with category-based filtering. See CLAUDE.md for architecture details.

## What You've Got

A complete Tauri desktop application that can:
- Fetch the latest versions of Australian clinical terminologies from NCTS
- Download and store terminology files locally
- Track version history in SQLite
- Provide a clean UI for managing syncs

## Quick Start (5 minutes)

### 1. Test NCTS Connectivity

```bash
cd /Users/jordanhannah/development/syndication
./test_ncts.sh
```

This will test if NCTS endpoints are accessible. If you see errors, check [NCTS_INTEGRATION.md](NCTS_INTEGRATION.md).

### 2. Build and Run

```bash
# Check everything compiles
cargo check

# Run the app
cargo run
```

The app will:
- Create a database at `~/Library/Application Support/com.ncts.syndication/syndication.db`
- Create a data directory for downloads
- Open a window with the UI

### 3. Sync Terminologies

In the app window:
1. Click "Sync All Terminologies" to sync all four terminology types
2. Or click "Sync Latest" on individual cards
3. Watch the activity log for progress

## What Happens During Sync

```
1. App requests Atom feed from NCTS
   → GET https://www.healthterminologies.gov.au/syndication/v3/snomed-ct-au

2. Parses XML to extract latest version
   → Title: "SNOMED CT-AU 20250131"
   → Download URL: https://.../snomed-au-20250131.zip

3. Downloads file to local storage
   → ~/Library/Application Support/com.ncts.syndication/terminology/

4. Saves metadata to database
   → Version, date, file path, timestamps

5. Marks as "latest" version
   → Future syncs will check against this version
```

## File Structure After First Sync

```
~/Library/Application Support/com.ncts.syndication/
├── syndication.db                    # Version tracking database
└── terminology/
    ├── snomed_[version].zip          # SNOMED CT-AU
    ├── loinc_[version].zip           # LOINC
    ├── amt_[version].zip             # AMT
    └── valuesets_[version].zip       # Value Sets
```

## Common First-Run Issues

### Issue: "Failed to fetch feed"

**Likely Cause**: NCTS endpoint URLs in the code are illustrative and may not match actual NCTS API.

**Solution**:
1. Check NCTS documentation for actual syndication URLs
2. Update URLs in [src/ncts.rs](src/ncts.rs) lines 15-30
3. Recompile: `cargo build`

### Issue: "Authentication required (HTTP 401)"

**Cause**: NCTS requires authentication.

**Solution**: See [NCTS_INTEGRATION.md](NCTS_INTEGRATION.md) section "Adding Authentication"

### Issue: App compiles but won't run

**Check**:
```bash
# macOS: Check if executable is blocked
xattr -d com.apple.quarantine target/debug/syndication

# Permissions
chmod +x target/debug/syndication
```

## Integrating with Your FHIR Questionnaire App

You mentioned wanting to use this with your OPDQS questionnaire app. Here's how:

### Step 1: Understand Current Architecture

Your questionnaire app ([CLAUDE.md](CLAUDE.md)) has:
- Web version: Uses external FHIR servers for ValueSet expansion
- Desktop version (planned): Should use local terminologies

### Step 2: This App Provides

- **Syndication layer**: Keeps local terminologies up-to-date
- **Storage layer**: SQLite database tracking versions
- **Download management**: Fetches and stores terminology files

### Step 3: Next Integration Steps

You'll need to add:

1. **ValueSet Expansion**: Parse downloaded files and implement FHIR `$expand` operation
2. **Concept Lookup**: Search for specific codes across terminologies
3. **IPC Bridge**: Connect questionnaire app to syndication app

### Example Integration Architecture

```
┌─────────────────────────────────────┐
│   OPDQS Questionnaire App           │
│   (SolidJS + Tauri)                 │
│                                     │
│   ┌───────────────────────────┐   │
│   │ terminology-desktop.ts    │   │
│   │ - expandValueSet()        │───┼──┐
│   │ - lookupCode()            │   │  │ IPC/Tauri Commands
│   │ - searchConcepts()        │   │  │
│   └───────────────────────────┘   │  │
└─────────────────────────────────────┘  │
                                         │
                                         ▼
┌─────────────────────────────────────┐
│   NCTS Syndication App (This!)      │
│   (Rust + Tauri)                    │
│                                     │
│   ┌───────────────────────────┐   │
│   │ Terminology Commands      │   │
│   │ - sync_terminology()      │   │
│   │ - expand_valueset()       │◀──┤ To Implement
│   │ - lookup_code()           │   │
│   └───────────────────────────┘   │
│                                     │
│   ┌───────────────────────────┐   │
│   │ SQLite + File Storage     │   │
│   │ - Version tracking        │   │
│   │ - Downloaded files        │   │
│   │ - Indexed concepts        │◀──┤ To Implement
│   └───────────────────────────┘   │
└─────────────────────────────────────┘
```

## Development Workflow

### Making Changes

1. **Edit Rust code**: Modify files in `src/`
2. **Check compilation**: `cargo check`
3. **Test**: `cargo test` (add tests as you go)
4. **Run**: `cargo run`

### Debugging

Add debug logging:

```rust
// In Cargo.toml, add:
// env_logger = "0.11"

// In main.rs, before run():
env_logger::init();

// In your code:
log::info!("Fetching feed from {}", url);
log::debug!("Response: {:?}", response);
log::error!("Failed to download: {}", error);
```

Run with logging:
```bash
RUST_LOG=debug cargo run
```

### Hot Reload (Frontend Only)

Currently, changes to Rust code require recompilation. For faster frontend iteration:

1. Make UI changes in [ui/index.html](ui/index.html)
2. Refresh the app (Cmd+R on macOS)
3. No recompile needed!

## Next Development Steps

### Phase 1: Basic Functionality (You Are Here! ✓)
- [x] Set up Tauri project
- [x] Implement Atom feed parsing
- [x] Create NCTS client
- [x] Build storage layer
- [x] Create basic UI
- [x] Test compilation

### Phase 2: Robustness
- [ ] Add proper error handling
- [ ] Implement retry logic
- [ ] Add download progress tracking
- [ ] Test with actual NCTS endpoints
- [ ] Add authentication if needed
- [ ] Verify checksums/hashes

### Phase 3: Terminology Processing
- [ ] Extract ZIP files
- [ ] Parse RF2 format (SNOMED)
- [ ] Parse CSV format (LOINC, AMT)
- [ ] Build search index
- [ ] Implement concept lookup

### Phase 4: FHIR Integration
- [ ] Implement ValueSet $expand
- [ ] Implement CodeSystem $lookup
- [ ] Implement ConceptMap $translate
- [ ] Add FHIR server emulation

### Phase 5: Advanced Features
- [ ] Automatic scheduled syncs
- [ ] Delta updates
- [ ] Multi-version support
- [ ] Export to various formats

## Testing Your Changes

### Manual Testing Checklist

- [ ] App starts without errors
- [ ] UI displays correctly
- [ ] Can click "Sync All"
- [ ] Activity log shows progress
- [ ] Cards update with version info
- [ ] Database file is created
- [ ] Files are downloaded
- [ ] Second sync detects "already up to date"
- [ ] App closes cleanly

### Automated Testing

Add tests in `src/` files:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminology_type_urls() {
        assert_eq!(
            TerminologyType::Snomed.feed_url(),
            "https://www.healthterminologies.gov.au/syndication/v3/snomed-ct-au"
        );
    }

    #[tokio::test]
    async fn test_fetch_feed() {
        // Use mock server or skip if no network
        // See NCTS_INTEGRATION.md for mock server example
    }
}
```

Run tests:
```bash
cargo test
```

## Getting Help

### Documentation in This Repo
- **[README.md](README.md)** - Complete technical documentation
- **[QUICKSTART.md](QUICKSTART.md)** - Step-by-step setup guide
- **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** - Project overview and status
- **[NCTS_INTEGRATION.md](NCTS_INTEGRATION.md)** - NCTS-specific integration details
- **[GETTING_STARTED.md](GETTING_STARTED.md)** - This file!

### External Resources
- **Tauri Docs**: https://tauri.app/
- **NCTS Website**: https://www.healthterminologies.gov.au/
- **FHIR Terminology**: https://www.hl7.org/fhir/terminology-module.html
- **Rust Book**: https://doc.rust-lang.org/book/

### Community
- **FHIR Chat**: https://chat.fhir.org (for FHIR questions)
- **Rust Users Forum**: https://users.rust-lang.org (for Rust questions)
- **Tauri Discord**: https://discord.com/invite/tauri (for Tauri questions)

## Production Deployment

When you're ready to deploy:

### 1. Build Release Version

```bash
cargo tauri build
```

Output will be in:
- **macOS**: `target/release/bundle/macos/NCTS Syndication.app`
- **Linux**: `target/release/bundle/appimage/ncts-syndication.AppImage`
- **Windows**: `target/release/bundle/msi/NCTS Syndication.msi`

### 2. Code Signing (macOS)

```bash
# Sign the app
codesign --deep --force --verify --verbose --sign "Developer ID Application: Your Name" "NCTS Syndication.app"

# Create DMG for distribution
create-dmg "NCTS Syndication.app"
```

### 3. Distribution

Options:
1. Direct download from your website
2. Mac App Store (requires Apple Developer account)
3. Windows Store (requires Microsoft account)
4. Internal distribution (for enterprise)

## Security Considerations

### Before Deployment
- [ ] Remove debug logging
- [ ] Secure credential storage
- [ ] Validate all inputs
- [ ] Test with restricted permissions
- [ ] Review CSP settings
- [ ] Enable code signing
- [ ] Test auto-updates (if implemented)

### Privacy
This app:
- ✓ Does NOT collect telemetry
- ✓ Does NOT transmit PHI
- ✓ Stores data locally only
- ✓ Uses HTTPS for NCTS connections

## Maintenance

### Regular Tasks
1. **Update dependencies**: `cargo update`
2. **Check for Tauri updates**: Monitor https://github.com/tauri-apps/tauri/releases
3. **Test with latest NCTS**: Verify endpoints still work
4. **Review logs**: Check for errors in production

### Monitoring
Consider adding:
- Error reporting (e.g., Sentry)
- Usage analytics (privacy-respecting)
- Update notifications
- Health checks

## Success Metrics

You'll know the app is working when:

1. ✓ Compiles without errors
2. ✓ Opens and displays UI
3. ✓ Successfully fetches at least one feed from NCTS
4. ✓ Downloads at least one terminology file
5. ✓ Stores version information in database
6. ✓ Shows correct version info in UI cards
7. ✓ Handles "already up to date" case
8. ✓ Gracefully handles network errors

## What's Next?

Pick your path:

### Path A: Test with Real NCTS
1. Run `./test_ncts.sh` to check connectivity
2. Update endpoints if needed
3. Add authentication if required
4. Test sync with actual NCTS

### Path B: Integrate with Questionnaire App
1. Export terminology lookup commands
2. Implement ValueSet expansion
3. Connect from your SolidJS app
4. Test end-to-end workflow

### Path C: Enhance Syndication App
1. Add download progress
2. Implement extraction
3. Build search index
4. Add scheduling

## Final Checklist

Before moving forward:

- [ ] Code compiles: `cargo check` ✓
- [ ] Understand project structure
- [ ] Know where data is stored
- [ ] Can run the app: `cargo run`
- [ ] Understand sync workflow
- [ ] Know how to test NCTS connectivity
- [ ] Familiar with documentation files
- [ ] Have next steps planned

## You're Ready!

The foundation is built. The app compiles and is ready for testing and enhancement. Choose your next step based on your priorities:

- **Need it working now?** → Test with NCTS, add auth if needed
- **Want full integration?** → Start on ValueSet expansion
- **Polish first?** → Add progress tracking, better error handling

Good luck! 🚀
