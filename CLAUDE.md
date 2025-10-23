# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Tauri desktop application for syncing Australian clinical terminology standards from the National Clinical Terminology Service (NCTS) using Atom feed syndication.

**Supported Terminologies:**
- **SNOMED CT-AU SNAPSHOT** - RF2 SNAPSHOT format only (~500MB+, quarterly updates)
- **AMT CSV** - CSV format only (~50MB, monthly updates)
- **NCTS FHIR R4 Bundles** - Value Sets in JSON format (<10MB, as-needed updates)

**Not Supported:**
- SNOMED DELTA format (not exposed by NCTS syndication feed)
- LOINC (proprietary binary format only, not available via syndication)

**Stack**: Rust + Tauri 2.1 | SQLite (SQLx) | Atom Syndication | Tokio async runtime

## Commands

```bash
# Development
cargo check                 # Check compilation
cargo run                   # Run the app in dev mode
cargo tauri dev            # Run with Tauri CLI (recommended)
cargo test                 # Run tests

# Testing NCTS connectivity
./test_ncts.sh             # Test if NCTS endpoints are accessible

# Production build
cargo tauri build          # Build production app bundle

# Debugging with logs
RUST_LOG=debug cargo run   # Enable debug logging
```

## Architecture

### Core Modules

**[src/auth.rs](src/auth.rs)** - OAuth2 Authentication

- `TokenManager` - Manages OAuth2 access tokens with automatic refresh
- Client Credentials grant flow via NCTS token endpoint
- Token caching with 60-second pre-expiry refresh
- Loads `NCTS_CLIENT_ID` and `NCTS_CLIENT_SECRET` from `.env`

**[src/ncts.rs](src/ncts.rs)** - NCTS Client & Atom Feed Parser

- `NctsClient` - HTTP client for fetching feeds and downloading files with Bearer auth
- `TerminologyType` enum - Snomed (SNAPSHOT), Amt (CSV), ValueSets (R4 Bundles), Loinc (not used)
- `FeedEntry` - Parsed Atom feed entry with version metadata
- Unified Feed URL: `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`
- **Category-based filtering** targets three specific terminology formats:
  - **SNOMED SNAPSHOT**: `SCT_RF2_SNAPSHOT` category only
  - **AMT CSV**: `AMT_CSV` category only
  - **Value Sets (R4)**: `FHIR_Bundle` category + title contains "(R4)" + excludes SNOMED reference sets

**[src/storage.rs](src/storage.rs)** - SQLite Storage Layer

- `TerminologyStorage` - Version tracking and file management
- `TerminologyVersion` - Version record with download metadata
- Custom DateTime serialization for SQLite compatibility
- Database schema with version tracking and "latest" marking

**[src/commands.rs](src/commands.rs)** - Tauri Commands (Frontend API)

- `sync_terminology(terminology_type)` - Sync specific terminology (SNOMED/AMT/ValueSets)
- `sync_all_terminologies()` - Sync all three supported types (SNOMED SNAPSHOT, AMT CSV, Value Sets)
- `fetch_latest_version(terminology_type)` - Get latest version from NCTS
- `get_local_latest(terminology_type)` - Get latest local version from database
- `AppState` - Shared state with NctsClient and TerminologyStorage

**[src/main.rs](src/main.rs)** - App Entry Point

- Loads `.env` file for credentials on startup
- Tauri builder setup
- App state initialization (TokenManager, NctsClient, TerminologyStorage)
- Data directory setup using `directories` crate
- Command registration

**[ui/index.html](ui/index.html)** - Frontend Interface

- Vanilla HTML/CSS/JavaScript
- Terminology cards with sync buttons
- Activity log for operation tracking
- Uses Tauri API to invoke Rust commands

### Data Storage

Platform-specific directories managed by `directories` crate:

- **macOS**: `~/Library/Application Support/com.ncts.syndication/`
- **Linux**: `~/.local/share/ncts/syndication/`
- **Windows**: `C:\Users\<User>\AppData\Roaming\ncts\syndication\`

Structure:

```
com.ncts.syndication/
├── syndication.db                  # SQLite database
└── terminology/                    # Downloaded terminology files
    ├── snomed_[version].zip        # SNOMED CT-AU SNAPSHOT (RF2 format)
    ├── amt_[version].csv           # AMT CSV format
    └── valuesets_[version].json    # FHIR R4 Value Set Bundles
```

### Database Schema

```sql
CREATE TABLE terminology_versions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    terminology_type TEXT NOT NULL,
    version TEXT NOT NULL,
    effective_date TEXT,
    download_url TEXT NOT NULL,
    file_path TEXT,
    downloaded_at TEXT,
    is_latest BOOLEAN NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(terminology_type, version)
);
```

**Database Initialization**: Uses `std::fs::create_dir_all` for directory creation (not `tokio::fs`) to avoid async runtime issues. SQLite connection string format: `sqlite:///{absolute_path}?mode=rwc` (three slashes + create mode).

## Sync Workflow

1. **Fetch Feed**: Request unified Atom feed from NCTS endpoint (`/syndication/v1/syndication.xml`)
2. **Parse Entries**: Extract version info and download URLs using `atom_syndication` crate
3. **Filter by Category**: Filter entries by category term:
   - `SCT_RF2_SNAPSHOT` for SNOMED SNAPSHOT
   - `AMT_CSV` for AMT CSV
   - `FHIR_Bundle` for Value Sets (with additional title filtering for R4)
4. **Identify Latest**: Find most recent version by `updated` date
5. **Check Existing**: Query SQLite to see if version already downloaded
6. **Download File**: Use `reqwest` to download file with Bearer auth
7. **Update Database**: Record version metadata and mark as latest

## Key Patterns

### Error Handling

- Uses `anyhow::Result` for NCTS client operations
- Custom `StorageError` enum with `thiserror` for storage layer
- Commands return `Result<T, String>` for Tauri frontend compatibility

### Async Runtime

- Tokio runtime for all async operations
- SQLx async SQLite driver
- `Arc<Mutex<TerminologyStorage>>` for shared state across commands

### DateTime Handling

- All times stored as RFC3339 strings in SQLite
- Custom `FromRow` implementation for `TerminologyVersion`
- Chrono serde with `ts_seconds` for JSON serialization

### Terminology Type Mapping

String → Enum parsing in `commands.rs`:

```rust
"snomed" → TerminologyType::Snomed      // SNAPSHOT format
"amt" → TerminologyType::Amt            // CSV format
"valuesets" → TerminologyType::ValueSets // R4 FHIR Bundles
"loinc" → TerminologyType::Loinc        // Not used (unavailable)
```

**Note**: The `Loinc` enum variant exists but is not used, as LOINC is not available via NCTS syndication.

## NCTS Integration Notes

### Current Status

- **Endpoints**: ✅ Using correct production NCTS API v1 endpoint
- **Feed Structure**: Single unified feed at `/syndication/v1/syndication.xml` with all terminologies
- **Authentication**: ✅ OAuth2 Client Credentials grant implemented
- **Token Management**: Automatic token acquisition and refresh
- **Category Filtering**: ✅ Implemented - targets SNAPSHOT, AMT CSV, and R4 Value Sets
- **Checksums**: ✅ SHA-256 validation implemented for downloads

**Supported Formats:**
- ✅ SNOMED CT-AU SNAPSHOT (RF2 SNAPSHOT format)
- ✅ AMT CSV format
- ✅ FHIR R4 Value Set Bundles

**Not Available:**
- ❌ SNOMED DELTA format (not exposed by NCTS syndication)
- ❌ LOINC (proprietary binary only, not in syndication feed)

### Authentication

OAuth2 authentication is implemented using the Client Credentials grant flow:

1. **Setup**: Add credentials to `.env` file in project root:

   ```bash
   NCTS_CLIENT_ID=your_client_id_here
   NCTS_CLIENT_SECRET=your_client_secret_here
   ```

2. **Token Flow**:
   - `TokenManager` requests access token from `https://api.healthterminologies.gov.au/oauth2/token`
   - Token cached with expiry tracking
   - Automatically refreshes 60 seconds before expiry
   - All NCTS API requests include `Authorization: Bearer {token}` header

3. **Credentials**: Obtain from NCTS Portal (clients menu) - requires NCTS account

### NCTS Feed Architecture

The NCTS provides a **single unified syndication feed** containing all terminology releases:

- **Endpoint**: `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`
- **Authentication**: Required - OAuth2 Bearer token
- **Structure**: One Atom feed with ~59 entries covering all terminology types

**App Filtering Strategy** - Uses `<category term="...">` elements + title-based filtering:

1. **SNOMED CT-AU SNAPSHOT**: Category `SCT_RF2_SNAPSHOT`
   - Gets RF2 SNAPSHOT format only
   - DELTA format not exposed by NCTS syndication

2. **AMT CSV**: Category `AMT_CSV`
   - Gets CSV format only

3. **Value Sets (R4)**: Category `FHIR_Bundle` + title filtering
   - Title must contain "(R4)"
   - Excludes SNOMED reference sets
   - JSON format FHIR R4 Bundles

### Testing Connectivity

Run `./test_ncts.sh` to verify NCTS endpoint is accessible. The script:
- Automatically loads credentials from `.env`
- Obtains OAuth2 access token
- Tests the v1 syndication endpoint with authentication
- Reports HTTP status and validates Atom feed structure

## Frontend-Backend Communication

JavaScript calls Rust via Tauri's `invoke` API:

```javascript
// Sync terminology
const result = await invoke("sync_terminology", {
  terminologyType: "snomed",
});

// Get all latest versions
const versions = await invoke("get_all_local_latest");
```

## Future Enhancements

**Not Yet Implemented**:

- ZIP extraction and indexing
- FHIR ValueSet `$expand` operation using local data
- Download progress tracking
- Automatic scheduled syncs
- Retry logic for failed downloads

**Integration Opportunity**:
This app can serve as a desktop terminology backend for FHIR applications needing offline ValueSet expansion. See [README.md](README.md) section "Integration with OPDQS App" for patterns.

## Terminology-Specific Notes

### Supported Terminologies

- **SNOMED CT-AU SNAPSHOT**:
  - Format: RF2 SNAPSHOT (ZIP archive)
  - Size: Large (~500MB+)
  - Update frequency: Quarterly
  - Category: `SCT_RF2_SNAPSHOT`

- **AMT CSV**:
  - Format: CSV only
  - Size: Moderate (~50MB)
  - Update frequency: Monthly
  - Category: `AMT_CSV`

- **Value Sets (FHIR R4)**:
  - Format: FHIR R4 Bundle (JSON)
  - Size: Small (<10MB)
  - Update frequency: As-needed
  - Category: `FHIR_Bundle` (filtered by title)

### Not Supported

- **SNOMED DELTA**: Not exposed by NCTS syndication feed
- **LOINC**: Proprietary binary format only, not available via syndication

## Development Tips

- Use `cargo check` frequently for fast compilation feedback
- **Test with Value Sets first** - smallest download (~10MB) before testing larger terminologies
- **AMT CSV** is moderate size (~50MB) - good for testing mid-size downloads
- **SNOMED SNAPSHOT** is large (~500MB+) - test last
- Check actual NCTS documentation for correct syndication endpoints
- Monitor console output for feed parsing details and category filtering
- Database operations are async - always `.await` storage calls
- DateTime handling is critical - use RFC3339 format for SQLite storage

## Security & Compliance

- **No PHI**: App handles terminology definitions only, not patient data
- **HTTPS Only**: All NCTS connections use HTTPS
- **Licensing**: Terminology files subject to SNOMED, AMT, and NCTS FHIR licenses - do not redistribute
- **Supported Content**: SNOMED SNAPSHOT, AMT CSV, FHIR R4 Value Sets only
- **CSP**: Content Security Policy configured in [tauri.conf.json](tauri.conf.json)
- **Sandboxing**: Tauri provides OS-level process isolation

## Documentation

- **[README.md](README.md)** - Comprehensive technical documentation
- **[QUICKSTART.md](QUICKSTART.md)** - Quick setup guide
- **[PROJECT_SUMMARY.md](PROJECT_SUMMARY.md)** - Project status overview
- **[NCTS_INTEGRATION.md](NCTS_INTEGRATION.md)** - NCTS-specific integration details
- **[GETTING_STARTED.md](GETTING_STARTED.md)** - Detailed getting started guide
