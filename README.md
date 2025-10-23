# NCTS Syndication App

A local Tauri application for syncing and managing Australian clinical terminology standards via atom-based syndication with the National Clinical Terminology Service (NCTS).

## Features

- **Atom Feed Syndication**: Automatically fetches the latest terminology versions from NCTS using Atom feeds
- **Terminology Support**:
  - SNOMED CT-AU SNAPSHOT (Systematized Nomenclature of Medicine - Australian Edition)
  - AMT CSV (Australian Medicines Terminology)
  - FHIR R4 Bundles (NCTS Value Sets)
  - ❌ LOINC not available (proprietary binary format only)
- **Local Storage**: SQLite database for tracking versions and downloads
- **Offline Support**: Downloaded terminology files stored locally
- **Version Management**: Track multiple versions and identify the latest
- **SHA-256 Validation**: Automatic checksum verification for downloaded files

## Architecture

### Backend (Rust)

- **Tauri**: Desktop application framework
- **atom_syndication**: Parse NCTS Atom feeds
- **reqwest**: HTTP client for fetching feeds and downloading terminology files
- **SQLx**: SQLite database for version tracking
- **tokio**: Async runtime

### Frontend

- Simple HTML/CSS/JavaScript interface
- Tauri API for invoking Rust commands
- Real-time status updates and logging

### Project Structure

```
syndication/
├── src/
│   ├── main.rs              # Application entry point
│   ├── ncts.rs              # NCTS client and Atom feed parsing
│   ├── storage.rs           # SQLite storage layer
│   └── commands.rs          # Tauri commands exposed to frontend
├── ui/
│   └── index.html           # Frontend interface
├── tauri.conf.json          # Tauri configuration
├── build.rs                 # Build script
└── Cargo.toml               # Rust dependencies
```

## NCTS Integration

### Unified Syndication Feed

The application connects to the NCTS unified syndication feed:

- **Endpoint**: `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`
- **Authentication**: OAuth2 Bearer token (required)
- **Structure**: Single Atom feed containing all terminology types
- **Filtering**: Entries are filtered by category term + title:
  - **SNOMED CT-AU**: `SCT_RF2_SNAPSHOT` only (DELTA not exposed by server)
  - **AMT**: `AMT_CSV` only
  - **FHIR Bundles**: `FHIR_Bundle` + title contains "(R4)" + excludes SNOMED reference sets
  - **LOINC**: ❌ NOT available in syndication feed (proprietary binary only)

### How It Works

1. **Fetch Feed**: Request the Atom feed from NCTS for a terminology type
2. **Parse Entries**: Extract version information, download URLs, and metadata
3. **Identify Latest**: Find the most recent version based on updated date
4. **Download**: Retrieve the terminology file (typically a ZIP archive)
5. **Store Metadata**: Save version info and file path to SQLite database
6. **Mark as Latest**: Update the database to identify this as the current version

## Database Schema

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

## Available Commands

The following Tauri commands are exposed to the frontend:

### `fetch_latest_version(terminology_type: String)`
Fetch the latest version information from NCTS for a specific terminology type.

```javascript
const latest = await invoke('fetch_latest_version', {
    terminologyType: 'snomed'
});
```

### `fetch_all_versions(terminology_type: String)`
Fetch all available versions from the NCTS feed.

```javascript
const versions = await invoke('fetch_all_versions', {
    terminologyType: 'loinc'
});
```

### `sync_terminology(terminology_type: String)`
Sync the latest version of a terminology type (fetch + download + store).

```javascript
const result = await invoke('sync_terminology', {
    terminologyType: 'amt'
});
```

Returns:
```javascript
{
    terminology_type: "amt",
    success: true,
    latest_version: "v20250101",
    error: null
}
```

### `sync_all_terminologies()`
Sync all terminology types in sequence.

```javascript
const results = await invoke('sync_all_terminologies');
```

### `get_local_latest(terminology_type: String)`
Get the latest locally stored version for a terminology type.

```javascript
const version = await invoke('get_local_latest', {
    terminologyType: 'valuesets'
});
```

### `get_local_versions(terminology_type: String)`
Get all locally stored versions for a terminology type.

```javascript
const versions = await invoke('get_local_versions', {
    terminologyType: 'snomed'
});
```

### `get_all_local_latest()`
Get the latest version across all terminology types.

```javascript
const allLatest = await invoke('get_all_local_latest');
```

## Installation & Usage

### Prerequisites

- Rust (latest stable)
- Node.js (for Tauri CLI)
- Cargo

### Setup

1. **Install Tauri CLI**:
   ```bash
   cargo install tauri-cli
   ```

2. **Install Dependencies**:
   ```bash
   cargo build
   ```

3. **Run in Development**:
   ```bash
   cargo tauri dev
   ```

4. **Build for Production**:
   ```bash
   cargo tauri build
   ```

### Configuration

The app stores data in platform-specific directories:

- **macOS**: `~/Library/Application Support/com.ncts.syndication/`
- **Linux**: `~/.local/share/ncts/syndication/`
- **Windows**: `C:\Users\<Username>\AppData\Roaming\ncts\syndication\`

Directory structure:
```
com.ncts.syndication/
├── syndication.db           # SQLite database
└── terminology/             # Downloaded terminology files
    ├── snomed_v20250101.zip
    ├── loinc_v20250101.zip
    ├── amt_v20250101.zip
    └── valuesets_v20250101.zip
```

## NCTS Authentication

**Note**: The current implementation assumes public access to NCTS syndication feeds. If authentication is required, you'll need to add:

1. **API Key Support**: Store and pass API keys in requests
2. **OAuth**: Implement OAuth flow for user authentication
3. **Certificate-based Auth**: Add client certificate support

To add authentication, modify the `NctsClient` in [src/ncts.rs](src/ncts.rs):

```rust
pub fn new_with_auth(api_key: String) -> Result<Self, Box<dyn Error>> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        "Authorization",
        header::HeaderValue::from_str(&format!("Bearer {}", api_key))?
    );

    let client = Client::builder()
        .user_agent("NCTS-Syndication/0.1.0")
        .default_headers(headers)
        .build()?;

    Ok(Self { client })
}
```

## Features to Add

### Short Term
- [ ] Progress tracking for large downloads
- [ ] Retry logic for failed downloads
- [ ] Checksum verification
- [ ] Automatic scheduled syncs

### Medium Term
- [ ] Extract and index terminology content
- [ ] Search functionality across terminologies
- [ ] Export to different formats (JSON, CSV)
- [ ] Delta updates (only download changes)

### Long Term
- [ ] FHIR ValueSet expansion using local data
- [ ] Integration with terminology servers (Ontoserver)
- [ ] Concept lookup and search
- [ ] Relationship browsing

## Integration with OPDQS App

This syndication app can serve as the desktop terminology backend mentioned in your CLAUDE.md:

### Desktop Integration Pattern

```typescript
// In your SolidJS app: src/lib/platform/terminology-desktop.ts

import { invoke } from '@tauri-apps/api/core';

export async function expandValueSet(url: string): Promise<ValueSet> {
    // 1. Try local SQLite/syndicated data first
    const localExpansion = await invoke('expand_valueset_local', { url });

    if (localExpansion) {
        return localExpansion;
    }

    // 2. Fall back to external FHIR server
    return expandValueSetExternal(url);
}

export async function syncTerminologies(): Promise<void> {
    await invoke('sync_all_terminologies');
}
```

### Benefits

- **Offline-first**: Terminology expansion works without network
- **Fast**: Local SQLite queries vs. network requests
- **Current**: Regular syncs keep data up-to-date
- **Fallback**: External servers available if needed

## Troubleshooting

### "Failed to fetch feed" Error

- Check your internet connection
- Verify NCTS endpoints are accessible
- Check if authentication is required

### Database Locked Error

- Ensure only one instance of the app is running
- Check file permissions on the data directory

### Download Failures

- Large files may timeout - increase timeout in `src/ncts.rs`
- Check available disk space
- Verify download URLs from NCTS feed

## License

MIT License - feel free to use in your projects

## Contributing

Contributions welcome! Please open an issue or PR.

## Related Resources

- [NCTS Website](https://www.healthterminologies.gov.au/)
- [Tauri Documentation](https://tauri.app/)
- [Atom Syndication Format](https://datatracker.ietf.org/doc/html/rfc4287)
- [FHIR Terminology Service](https://www.hl7.org/fhir/terminology-service.html)
