# Quick Start Guide

> **Update**: The app now uses the correct NCTS API v1 endpoint: `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`

## Running the NCTS Syndication App

### 1. Install Dependencies

First, make sure you have Rust installed:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 2. Build and Run

```bash
# Build the project (this will download all dependencies)
cargo build

# Run in development mode
cargo run
```

Or use Tauri CLI for a better development experience:

```bash
# Install Tauri CLI
cargo install tauri-cli

# Run with Tauri
cargo tauri dev
```

### 3. Using the Application

Once the app starts, you'll see a window with four terminology cards:

1. **SNOMED CT-AU** - Clinical terminology for procedures, diagnoses, etc.
2. **LOINC** - Laboratory and clinical observations
3. **AMT** - Australian medicines and prescriptions
4. **Value Sets** - Pre-defined value sets for FHIR

#### Sync a Single Terminology

Click the "Sync Latest" button on any card to:
- Fetch the latest version from NCTS
- Download the terminology file
- Save it locally with metadata

#### Sync All Terminologies

Click "Sync All Terminologies" at the top to sync all four at once.

#### Check Status

The cards show:
- Current sync status (Synced/Not Synced/Error)
- Version number
- Download date
- Effective date (if available)

### 4. Where Data is Stored

The app creates a local database and downloads files to:

**macOS**: `~/Library/Application Support/com.ncts.syndication/`

```
com.ncts.syndication/
├── syndication.db           # Version tracking
└── terminology/             # Downloaded files
    ├── snomed_[version].zip
    ├── loinc_[version].zip
    ├── amt_[version].zip
    └── valuesets_[version].zip
```

## Testing the API

You can test the backend independently by creating a simple test file:

```bash
# Create a test file
cat > test_sync.sh << 'EOF'
#!/bin/bash

# This requires the app to be running
# The Tauri commands are only accessible from within the Tauri context

echo "Please run the app with 'cargo tauri dev' to test the API"
EOF

chmod +x test_sync.sh
```

## Integrating with Your FHIR App

To use this with your questionnaire app (from CLAUDE.md):

### 1. Add Tauri to Your Existing Project

```bash
# In your questionnaire app directory
npm install @tauri-apps/api @tauri-apps/cli
```

### 2. Update Your terminology-desktop.ts

```typescript
import { invoke } from '@tauri-apps/api/core';

export async function getLocalTerminology(
    terminologyType: 'snomed' | 'loinc' | 'amt' | 'valuesets'
) {
    return await invoke('get_local_latest', {
        terminologyType
    });
}

export async function syncTerminologies() {
    return await invoke('sync_all_terminologies');
}

// Use for ValueSet expansion
export async function expandValueSetLocal(valueSetUrl: string) {
    // This would require implementing a ValueSet expansion command
    // in the Rust backend that queries the downloaded terminology files
    return await invoke('expand_valueset', { url: valueSetUrl });
}
```

### 3. Desktop-Only Feature Flag

```typescript
// src/lib/platform/index.ts
export const isDesktop = () => {
    return window.__TAURI__ !== undefined;
};

// Use in your components
if (isDesktop()) {
    // Use local terminology
    const terminology = await getLocalTerminology('snomed');
} else {
    // Use external FHIR server
    const terminology = await fetchFromFhirServer();
}
```

## Building for Production

```bash
# Create optimized build
cargo tauri build

# The installer will be in:
# target/release/bundle/
```

The built app will be a native application for your platform (macOS .app, Windows .exe, Linux .AppImage).

## Troubleshooting

### Build Errors

If you get compilation errors:

```bash
# Clean and rebuild
cargo clean
cargo build
```

### NCTS Connection Issues

The current implementation assumes NCTS feeds are publicly accessible. If you get 401/403 errors, you may need to:

1. Register with NCTS for API access
2. Add authentication headers in `src/ncts.rs`
3. Store credentials securely using [tauri-plugin-store](https://github.com/tauri-apps/tauri-plugin-store)

### Database Locked

If the database is locked:

```bash
# Close all instances of the app
# Remove lock files
rm ~/Library/Application\ Support/com.ncts.syndication/syndication.db-shm
rm ~/Library/Application\ Support/com.ncts.syndication/syndication.db-wal
```

## Next Steps

1. **Add Authentication**: If NCTS requires it, implement API key storage
2. **Extract & Index**: Parse downloaded ZIP files and index concepts
3. **ValueSet Expansion**: Implement local FHIR ValueSet $expand operation
4. **Auto-sync**: Add background syncing on a schedule
5. **Progress Tracking**: Show download progress for large files

## Need Help?

- Check the [README.md](README.md) for detailed documentation
- Review the [CLAUDE.md](CLAUDE.md) for integration patterns
- Open an issue on GitHub

## Example Session

```
$ cargo tauri dev

   Compiling syndication v0.1.0
    Finished dev [unoptimized + debuginfo] target(s) in 45.23s
     Running `target/debug/syndication`

Database path: "/Users/you/Library/Application Support/com.ncts.syndication/syndication.db"
Data directory: "/Users/you/Library/Application Support/com.ncts.syndication/terminology"

[App window opens]

[Click "Sync All Terminologies"]

Fetching feed from: https://www.healthterminologies.gov.au/syndication/v3/snomed-ct-au
Downloading from: https://www.healthterminologies.gov.au/download/snomed/...
Downloaded to: "...../terminology/snomed_20250101.zip"

✓ All terminologies synced successfully!
```
