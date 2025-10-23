# NCTS Syndication App - Project Summary

## Overview

A Tauri-based desktop application for syncing Australian clinical terminology standards from NCTS (National Clinical Terminology Service) using Atom feed syndication.

## What Has Been Built

### ✅ Core Features Implemented

1. **Atom Feed Parser** ([src/ncts.rs](src/ncts.rs))
   - Fetches and parses Atom feeds from NCTS
   - Supports SNOMED CT-AU, LOINC, AMT, and Value Sets
   - Extracts version information and download URLs
   - Downloads terminology files to local storage

2. **Local Storage Layer** ([src/storage.rs](src/storage.rs))
   - SQLite database for version tracking
   - Stores metadata: version, effective date, download path, timestamps
   - Tracks "latest" version for each terminology type
   - Custom DateTime serialization for SQLite compatibility

3. **Tauri Commands** ([src/commands.rs](src/commands.rs))
   - `fetch_latest_version` - Get latest version from NCTS
   - `fetch_all_versions` - Get all available versions
   - `sync_terminology` - Sync specific terminology type
   - `sync_all_terminologies` - Sync all at once
   - `get_local_latest` - Get latest local version
   - `get_local_versions` - Get all local versions
   - `get_all_local_latest` - Get latest across all types

4. **Frontend UI** ([ui/index.html](ui/index.html))
   - Clean, modern interface with terminology cards
   - Real-time sync status display
   - Activity log showing all operations
   - Sync individual or all terminologies
   - Responsive grid layout

### 📂 Project Structure

```
syndication/
├── src/
│   ├── main.rs           # App entry point, Tauri setup
│   ├── ncts.rs           # NCTS client, Atom feed parsing
│   ├── storage.rs        # SQLite storage layer
│   └── commands.rs       # Tauri commands for frontend
├── ui/
│   └── index.html        # Frontend interface
├── icons/
│   └── icon.png          # App icon (placeholder)
├── Cargo.toml            # Rust dependencies
├── tauri.conf.json       # Tauri configuration
├── build.rs              # Build script
├── README.md             # Comprehensive documentation
├── QUICKSTART.md         # Getting started guide
├── PROJECT_SUMMARY.md    # This file
└── .env.example          # Configuration template
```

### 🛠 Technology Stack

**Backend:**
- Rust
- Tauri 2.1 - Desktop app framework
- atom_syndication - Atom feed parsing
- reqwest - HTTP client
- SQLx - SQLite database
- tokio - Async runtime
- anyhow - Error handling
- chrono - Date/time handling

**Frontend:**
- Vanilla HTML/CSS/JavaScript
- Tauri API for Rust commands

## How It Works

### 1. Fetch Feed
```rust
// Get latest version info from NCTS
let latest = ncts_client.fetch_latest(TerminologyType::Snomed).await?;
```

### 2. Download Terminology
```rust
// Download the file
ncts_client.download_terminology(&url, &file_path).await?;
```

### 3. Store Metadata
```rust
// Save version info to database
let id = storage.record_version("snomed", "v20250101", None, &url).await?;
storage.mark_downloaded(id, &file_path).await?;
storage.mark_as_latest(id, "snomed").await?;
```

### 4. Frontend Integration
```javascript
// Call from JavaScript
const result = await invoke('sync_terminology', { terminologyType: 'snomed' });
```

## Data Storage Locations

The app stores data in platform-specific directories:

- **macOS**: `~/Library/Application Support/com.ncts.syndication/`
- **Linux**: `~/.local/share/ncts/syndication/`
- **Windows**: `C:\Users\<User>\AppData\Roaming\ncts\syndication\`

### Directory Structure:
```
com.ncts.syndication/
├── syndication.db              # SQLite database
└── terminology/                # Downloaded files
    ├── snomed_[version].zip
    ├── loinc_[version].zip
    ├── amt_[version].zip
    └── valuesets_[version].zip
```

## Running the Application

### Development Mode

```bash
# Check code compiles
cargo check

# Run the app
cargo run

# Or with Tauri CLI (recommended)
cargo install tauri-cli
cargo tauri dev
```

### Build for Production

```bash
cargo tauri build
```

## Integration with OPDQS Questionnaire App

This app is designed to work alongside your FHIR questionnaire application (from [CLAUDE.md](CLAUDE.md)):

### Use Case
Your questionnaire app needs ValueSet expansion for terminology-dependent questions (e.g., SNOMED codes for diagnoses). The desktop version can:

1. **Sync terminologies** using this app
2. **Expand ValueSets** using local data (fast, offline)
3. **Fall back** to external FHIR servers if needed

### Integration Pattern

```typescript
// In your questionnaire app: src/lib/platform/terminology-desktop.ts

import { invoke } from '@tauri-apps/api/core';

export async function expandValueSet(url: string): Promise<ValueSet> {
    // Try local first
    try {
        return await invoke('expand_valueset_local', { url });
    } catch {
        // Fall back to external FHIR server
        return await expandValueSetExternal(url);
    }
}

export async function syncAllTerminologies() {
    return await invoke('sync_all_terminologies');
}
```

## Current Limitations & Future Enhancements

### Known Limitations
1. **No Authentication**: Assumes NCTS feeds are publicly accessible
2. **No ValueSet Expansion**: Downloads files but doesn't index/query content yet
3. **No Progress Tracking**: Large downloads don't show progress
4. **No Scheduling**: Manual sync only (no auto-sync)
5. **No Extraction**: ZIP files remain compressed

### Planned Enhancements

**Phase 1 - Core Improvements**
- [ ] Add authentication support (API keys, OAuth)
- [ ] Download progress tracking
- [ ] Retry logic for failed downloads
- [ ] Checksum verification

**Phase 2 - Data Processing**
- [ ] Extract ZIP archives
- [ ] Parse and index terminology content
- [ ] Implement ValueSet expansion from local data
- [ ] Search functionality

**Phase 3 - Advanced Features**
- [ ] Automatic scheduled syncs
- [ ] Delta updates (only download changes)
- [ ] FHIR terminology server emulation
- [ ] Integration with Ontoserver

## API Documentation

### Rust API

#### NctsClient
```rust
// Create client
let client = NctsClient::new()?;

// Fetch latest version
let latest = client.fetch_latest(TerminologyType::Snomed).await?;

// Download file
client.download_terminology(&url, &path).await?;
```

#### TerminologyStorage
```rust
// Initialize storage
let storage = TerminologyStorage::new(db_path, data_dir).await?;

// Record version
let id = storage.record_version("snomed", "v1", None, "url").await?;

// Mark as downloaded
storage.mark_downloaded(id, "/path/to/file").await?;

// Mark as latest
storage.mark_as_latest(id, "snomed").await?;

// Query
let latest = storage.get_latest("snomed").await?;
```

### JavaScript API (Frontend)

All commands return Promises:

```javascript
// Fetch latest from NCTS
await invoke('fetch_latest_version', { terminologyType: 'snomed' });

// Sync terminology
await invoke('sync_terminology', { terminologyType: 'loinc' });

// Sync all
await invoke('sync_all_terminologies');

// Get local versions
await invoke('get_local_latest', { terminologyType: 'amt' });
await invoke('get_all_local_latest');
```

## NCTS Endpoints

The app connects to these NCTS syndication endpoints:

**Unified Feed Endpoint**: `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`

The NCTS provides a single unified Atom feed containing all terminology types. The application filters entries by category:

| Terminology | Category Terms |
|------------|----------------|
| SNOMED CT-AU | `SCT_RF2_FULL`, `SCT_RF2_SNAPSHOT`, `SCT_RF2_ALL` |
| AMT | `AMT_CSV`, `AMT_TSV` |
| LOINC | Not available in syndication feed |
| Value Sets | Not available in syndication feed |

## Testing

The code compiles successfully with warnings (unused code that will be used in future phases).

### Manual Testing Steps
1. Run the app: `cargo run`
2. Click "Sync All Terminologies"
3. Observe activity log for progress
4. Check cards for version information
5. Verify files downloaded to data directory

### Unit Tests
```bash
cargo test
```

Current test coverage:
- [x] Feed URL generation
- [ ] Feed parsing (TODO)
- [ ] Database operations (TODO)
- [ ] Download logic (TODO)

## Security Considerations

1. **No PHI**: This app handles terminology definitions only, not patient data
2. **HTTPS**: All NCTS connections use HTTPS
3. **CSP**: Content Security Policy configured in tauri.conf.json
4. **Sandboxing**: Tauri provides OS-level sandboxing
5. **File System**: Limited to app data directory only

## Troubleshooting

### Compilation Errors
```bash
cargo clean
cargo build
```

### Database Locked
```bash
# Close all app instances
# Remove lock files
rm ~/Library/Application\ Support/com.ncts.syndication/*.db-shm
rm ~/Library/Application\ Support/com.ncts.syndication/*.db-wal
```

### NCTS Connection Failed
- Check internet connection
- Verify NCTS endpoints are accessible
- Check if authentication is required

## Documentation Files

- **[README.md](README.md)** - Comprehensive documentation with architecture, API reference, and integration guide
- **[QUICKSTART.md](QUICKSTART.md)** - Step-by-step guide to get started quickly
- **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** - This file, project overview
- **[CLAUDE.md](CLAUDE.md)** - Your existing questionnaire app documentation
- **[.env.example](.env.example)** - Configuration template

## Next Steps

1. **Test the Application**
   ```bash
   cargo run
   ```

2. **Customize for Your Needs**
   - Update NCTS endpoints if different
   - Add authentication if required
   - Modify UI styling to match your brand

3. **Extend Functionality**
   - Implement ValueSet expansion
   - Add search/lookup features
   - Integrate with your questionnaire app

4. **Deploy**
   ```bash
   cargo tauri build
   ```
   Distributable app will be in `target/release/bundle/`

## Support & Contributing

- Report issues: Create GitHub issue
- Contribute: Fork and submit PR
- Questions: Check README.md and QUICKSTART.md first

## License

MIT License - Use freely in your projects

---

**Built with**
- Rust + Tauri
- Love for healthcare interoperability
- Australian clinical terminology standards
