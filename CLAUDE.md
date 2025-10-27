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

**Sync Commands:**
- `sync_terminology(terminology_type)` - Sync specific terminology (SNOMED/AMT/ValueSets)
- `sync_all_terminologies()` - Sync all three supported types (SNOMED SNAPSHOT, AMT CSV, Value Sets)
- `fetch_latest_version(terminology_type)` - Get latest version from NCTS
- `get_local_latest(terminology_type)` - Get latest local version from database

**Import & Query Commands:**
- `import_terminology(terminology_type)` - Parse and import downloaded files into database
- `search_terminology(query, types, limit)` - Full-text search across terminologies
- `lookup_code(code, system)` - Get code details with synonyms
- `expand_valueset(valueset_url)` - Expand ValueSet to concept codes
- `validate_code(code, system, valueset_url)` - Validate code against ValueSet
- `list_valuesets()` - List all available ValueSets

**State:**
- `AppState` - Shared state with NctsClient and TerminologyStorage

**[src/parsers/](src/parsers/)** - Terminology File Parsers

- `snomed_rf2.rs` - Parse SNOMED RF2 SNAPSHOT TSV files (concepts, descriptions, relationships)
- `amt_csv.rs` - Parse AMT CSV format with quoted field support
- `valueset_r4.rs` - Parse FHIR R4 ValueSet Bundles (JSON)
- Callback-based streaming parsers for memory efficiency

**[src/import.rs](src/import.rs)** - Terminology Import Module

- `TerminologyImporter` - Orchestrates file extraction and database import
- ZIP extraction for SNOMED archives to temporary directories
- Batch inserts (1000 records/batch) for optimal performance
- Recursive file searching for RF2 files in extracted archives
- Import tracking with `imported` and `imported_at` fields

**[src/queries.rs](src/queries.rs)** - Terminology Query Operations

- `lookup_snomed_code()` / `lookup_amt_code()` - Code lookup with synonyms
- `search_snomed()` / `search_amt()` / `search_all()` - Full-text search with ranking
- `expand_valueset()` - Get all codes in a ValueSet expansion
- `validate_code()` - Check if code exists in ValueSet
- `list_valuesets()` - Browse available ValueSets

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

**Version Tracking:**
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
    content_item_identifier TEXT,
    content_item_version TEXT,
    sha256_hash TEXT,
    sct_base_version TEXT,
    imported BOOLEAN NOT NULL DEFAULT 0,  -- NEW: Import tracking
    imported_at TEXT,                     -- NEW: Import timestamp
    UNIQUE(terminology_type, version)
);
```

**SNOMED CT-AU Content Tables:**
```sql
-- Concepts (core terminology entities)
CREATE TABLE snomed_concepts (
    id TEXT PRIMARY KEY,
    effective_time TEXT NOT NULL,
    active INTEGER NOT NULL,
    module_id TEXT NOT NULL,
    definition_status_id TEXT NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
);

-- Descriptions (terms and synonyms)
CREATE TABLE snomed_descriptions (
    id TEXT PRIMARY KEY,
    effective_time TEXT NOT NULL,
    active INTEGER NOT NULL,
    module_id TEXT NOT NULL,
    concept_id TEXT NOT NULL,
    language_code TEXT NOT NULL,
    type_id TEXT NOT NULL,
    term TEXT NOT NULL,              -- Full-text searchable
    case_significance_id TEXT NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (concept_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
);

-- Relationships (concept associations)
CREATE TABLE snomed_relationships (
    id TEXT PRIMARY KEY,
    effective_time TEXT NOT NULL,
    active INTEGER NOT NULL,
    module_id TEXT NOT NULL,
    source_id TEXT NOT NULL,
    destination_id TEXT NOT NULL,
    relationship_group INTEGER NOT NULL,
    type_id TEXT NOT NULL,
    characteristic_type_id TEXT NOT NULL,
    modifier_id TEXT NOT NULL,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (source_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
    FOREIGN KEY (destination_id) REFERENCES snomed_concepts(id) ON DELETE CASCADE,
    FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
);
```

**AMT Content Tables:**
```sql
CREATE TABLE amt_codes (
    id TEXT PRIMARY KEY,
    preferred_term TEXT NOT NULL,     -- Full-text searchable
    code_type TEXT NOT NULL,
    parent_code TEXT,
    properties TEXT,                   -- JSON storage for additional properties
    version_id INTEGER NOT NULL,
    FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
);
```

**FHIR ValueSet Tables:**
```sql
-- ValueSet metadata
CREATE TABLE valuesets (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    url TEXT NOT NULL UNIQUE,
    version TEXT,
    name TEXT,
    title TEXT,
    status TEXT,
    description TEXT,
    publisher TEXT,
    version_id INTEGER NOT NULL,
    FOREIGN KEY (version_id) REFERENCES terminology_versions(id) ON DELETE CASCADE
);

-- ValueSet expansion (concept codes)
CREATE TABLE valueset_concepts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    valueset_id INTEGER NOT NULL,
    system TEXT NOT NULL,
    code TEXT NOT NULL,
    display TEXT,
    FOREIGN KEY (valueset_id) REFERENCES valuesets(id) ON DELETE CASCADE,
    UNIQUE(valueset_id, system, code)
);
```

**Indexes for Performance:**
- Concepts: `active`, `version_id`
- Descriptions: `concept_id`, `active`, `type_id`, `term` (full-text search)
- Relationships: `source_id`, `destination_id`, `type_id`, `active`
- AMT: `version_id`, `code_type`, `preferred_term`, `parent_code`
- ValueSets: `url`, `version_id`
- ValueSet Concepts: `valueset_id`, `code`, `system`, composite `(valueset_id, system, code)`

**Database Initialization**: Uses `std::fs::create_dir_all` for directory creation (not `tokio::fs`) to avoid async runtime issues. SQLite connection string format: `sqlite:///{absolute_path}?mode=rwc` (three slashes + create mode).

## Sync & Import Workflow

### Sync Phase (Download Files)

1. **Fetch Feed**: Request unified Atom feed from NCTS endpoint (`/syndication/v1/syndication.xml`)
2. **Parse Entries**: Extract version info and download URLs using `atom_syndication` crate
3. **Filter by Category**: Filter entries by category term:
   - `SCT_RF2_SNAPSHOT` for SNOMED SNAPSHOT
   - `AMT_CSV` for AMT CSV
   - `FHIR_Bundle` for Value Sets (with additional title filtering for R4)
4. **Identify Latest**: Find most recent version by `updated` date
5. **Check Existing**: Query SQLite to see if version already downloaded
6. **Download File**: Use `reqwest` to download file with Bearer auth
7. **Validate Checksum**: Verify SHA-256 hash if provided in feed
8. **Update Database**: Record version metadata and mark as latest

### Import Phase (Parse & Index Content)

1. **Check Import Status**: Verify if terminology already imported (via `imported` field)
2. **Extract Files** (SNOMED only): Unzip RF2 SNAPSHOT archive to temp directory
3. **Locate Files**: Find terminology files by pattern:
   - SNOMED: `sct2_Concept_Snapshot`, `sct2_Description_Snapshot-en`, `sct2_Relationship_Snapshot`
   - AMT: Direct CSV file access
   - ValueSets: Direct JSON Bundle access
4. **Parse Content**: Stream-parse files using callback-based parsers
5. **Batch Import**: Insert records in 1000-record batches for optimal performance
   - SNOMED: Concepts → Descriptions → Relationships (sequential for FK integrity)
   - AMT: Codes
   - ValueSets: ValueSet metadata → Expansion concepts
6. **Mark Imported**: Update `imported` and `imported_at` fields
7. **Cleanup**: Remove temporary extracted files (SNOMED)

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

JavaScript/TypeScript calls Rust via Tauri's `invoke` API:

```javascript
// Sync Commands - Download terminology files
const syncResult = await invoke("sync_terminology", {
  terminologyType: "snomed",
});

const allVersions = await invoke("get_all_local_latest");

// Import Commands - Parse and index content into database
const importResult = await invoke("import_terminology", {
  terminologyType: "valuesets",
});

// Search Commands - Full-text search across terminologies
const searchResults = await invoke("search_terminology", {
  query: "diabetes",
  terminologyTypes: ["snomed", "amt"],
  limit: 20,
});

// Lookup Commands - Get code details with synonyms
const codeDetails = await invoke("lookup_code", {
  code: "73211009",
  system: "http://snomed.info/sct",
});

// ValueSet Commands - Expand and validate
const expansion = await invoke("expand_valueset", {
  valuesetUrl: "http://healthterminologies.gov.au/fhir/ValueSet/example",
});

const validation = await invoke("validate_code", {
  code: "73211009",
  system: "http://snomed.info/sct",
  valuesetUrl: "http://healthterminologies.gov.au/fhir/ValueSet/example",
});

const allValueSets = await invoke("list_valuesets");
```

### Response Types

```typescript
// Search result
interface SearchResult {
  code: string;
  system: string;
  display: string;
  terminology_type: string;
  active: boolean;
}

// Code lookup result
interface CodeLookupResult {
  code: string;
  system: string;
  display: string;
  active: boolean;
  synonyms: string[];
}

// ValueSet expansion
interface ValueSetExpansion {
  url: string;
  version?: string;
  title?: string;
  total: number;
  concepts: ValueSetConceptResult[];
}

// Validation result
interface ValidationResult {
  valid: boolean;
  message?: string;
}
```

## Implementation Status

**✅ Implemented**:

- ✅ ZIP extraction and indexing (SNOMED RF2 archives)
- ✅ FHIR ValueSet `$expand` operation using local data
- ✅ Full-text search across SNOMED and AMT
- ✅ Code lookup with synonyms (SNOMED FSN + all descriptions)
- ✅ ValueSet expansion from database
- ✅ Code validation against ValueSets
- ✅ Batch import with performance optimization (1000 records/batch)
- ✅ SHA-256 checksum validation for downloads
- ✅ Import tracking (imported/imported_at fields)

**📋 Future Enhancements**:

- Download progress tracking (streaming progress updates)
- Automatic scheduled syncs (background daemon mode)
- Retry logic for failed downloads with exponential backoff
- SNOMED hierarchy navigation (IS-A relationships)
- Relationship traversal queries (find all descendants/ancestors)
- Full-text search ranking improvements (TF-IDF or FTS5)
- Incremental updates support (if NCTS exposes DELTA format)
- Export functionality (export terminology subsets)
- API server mode (HTTP REST API in addition to Tauri IPC)

**Integration Use Cases**:

This app now serves as a **complete desktop terminology backend** for FHIR applications needing:
- Offline ValueSet expansion for questionnaires
- Code validation for form answers
- Full-text terminology search for code selection
- Synonym lookup for display enhancement
- Local terminology browsing without API calls

## Future Project Goals: Offline Patient-Doctor Questionnaires (OPDQ)

**Vision**: Extend this app to support privacy-preserving, air-gapped clinical questionnaires using QR code exchange.

### OPDQ System Overview

The planned OPDQ feature will enable secure questionnaire workflows between doctors and patients without network transmission:

**Two-Way QR Exchange:**
1. **Doctor → Patient**: QR code contains questionnaire definition + doctor's public encryption key
2. **Patient → Doctor**: Animated BC-UR QR sequence with encrypted responses

**Key Design Principles:**
- **Zero Network Transfer**: Complete air-gap architecture using visual QR codes only
- **End-to-End Encryption**: RSA/AES hybrid encryption (responses encrypted with doctor's public key)
- **BC-UR Animated QR**: All responses use fountain-coded multi-frame QR (handles 1KB-100KB+ payloads)
- **HIPAA Compliance**: No pre-shared secrets, patient controls sharing, ephemeral session keys
- **Platform Support**: Desktop-only (Tauri + SolidJS + SQLCipher) - no web version for security

### Architecture Plans

**Frontend (SolidJS in Tauri WebView):**
- `BCURScanner.tsx` - Multi-frame QR scanner using qr-scanner library
- `BCURGenerator.tsx` - Rotating animated QR display (10-15 FPS)
- `QuestionnaireForm.tsx` - Patient questionnaire UI with validation
- `ResponseViewer.tsx` - Clinician response viewer
- Web Crypto API for all cryptographic operations (RSA-OAEP-4096, AES-256-GCM)

**Backend (Rust/Tauri):**
- SQLCipher for encrypted storage of patient responses
- Questionnaire templates bundled at compile-time
- Integration with existing NCTS terminology for ValueSet-driven questions
- Automatic data deletion after viewing/exporting responses
- OS keychain integration for private key storage

### Planned Database Extensions

**Doctor's Device (Desktop):**
```sql
CREATE TABLE questionnaire_sessions (
    id TEXT PRIMARY KEY,
    questionnaire_id TEXT NOT NULL,
    public_key TEXT NOT NULL,
    private_key_ref TEXT NOT NULL,  -- OS keychain reference
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    status TEXT DEFAULT 'pending'   -- pending, completed, expired
);

CREATE TABLE patient_responses (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    questionnaire_id TEXT NOT NULL,
    received_at TEXT NOT NULL,
    encrypted_responses BLOB NOT NULL,  -- Double-encrypted
    encryption_nonce BLOB NOT NULL,
    clinician_id TEXT NOT NULL,
    viewed BOOLEAN DEFAULT 0,
    exported BOOLEAN DEFAULT 0,
    FOREIGN KEY (session_id) REFERENCES questionnaire_sessions(id)
);

CREATE TABLE questionnaires (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    version TEXT NOT NULL,
    definition TEXT NOT NULL,  -- JSON questionnaire schema
    created_at TEXT NOT NULL,
    active BOOLEAN DEFAULT 1
);
```

**Patient's Device (Desktop):**
- Same Tauri app as doctor's device
- Uses IndexedDB for temporary session storage (cleared after QR generation)
- No persistent patient data storage

### Performance Characteristics

**BC-UR Animated QR (qr-scanner library):**
- Small questionnaires (~1KB): 1 frame, ~100ms scan time
- Medium questionnaires (~3KB): 2 frames, ~200ms scan time
- Large questionnaires (~10KB): 6 frames, ~600ms scan time
- Frame rate: 10-15 FPS for reliable scanning
- Throughput: ~25 kbps
- Fountain coding: Stateless, missing-frame tolerant

**Why BC-UR Only (No Single QR Mode):**
- Eliminates format selection logic
- Single code path for all payload sizes
- Minimal overhead for small payloads (~50ms extra vs static QR)
- Handles complex questionnaires without modification

### Technology Stack (Additional)

**JavaScript/TypeScript Dependencies:**
```json
{
  "dependencies": {
    "solid-js": "^1.8.0",
    "qr-scanner": "^1.4.2",
    "qrcode": "^1.5.3",
    "@ngraveio/bc-ur": "^1.1.12",
    "uuid": "^10.0.0"
  }
}
```

**Rust Dependencies (Desktop Only):**
```toml
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
keyring = "3.8"  # OS keychain integration
```

### Integration with Current System

The OPDQ feature will leverage existing terminology infrastructure:

1. **ValueSet-Driven Questions**: Use `expand_valueset()` and `validate_code()` commands for questionnaire answer options
2. **Code Search**: `search_terminology()` for autocomplete in questionnaire builders
3. **Offline Validation**: All terminology lookups happen locally using imported SNOMED/AMT data
4. **Shared Storage**: Separate SQLite database for OPDQ data, same directory structure

**Example Integration:**
```javascript
// Questionnaire uses ValueSet for answer options
const diabetesScreening = {
  questions: [{
    id: "diagnosis",
    text: "Primary diagnosis?",
    type: "valueset",
    valuesetUrl: "http://healthterminologies.gov.au/fhir/ValueSet/diabetes-conditions",
  }]
};

// Expand ValueSet to get answer options (offline)
const options = await invoke("expand_valueset", {
  valuesetUrl: diabetesScreening.questions[0].valuesetUrl
});

// Generate QR with questionnaire + options
const qrPayload = {
  questionnaire: diabetesScreening,
  answerOptions: options.concepts,
  doctorPublicKey: sessionKey
};
```

### Security & Privacy (OPDQ-Specific)

- **Encryption**: RSA-OAEP-4096 for key exchange, AES-256-GCM for payload
- **Key Storage**: OS keychain for private keys (macOS Keychain, Windows Credential Manager, Linux Secret Service)
- **Data at Rest**: SQLCipher encryption for all patient responses on disk
- **Ephemeral Sessions**: 1-hour session expiry, automatic cleanup
- **Audit Trail**: All access logged (received, viewed, exported, deleted)
- **No PHI Transmission**: Zero network activity during questionnaire exchange
- **Patient Consent**: Visual confirmation before scanning doctor's QR

### Implementation Roadmap

**Phase 1**: Core OPDQ Infrastructure (Desktop)
- [ ] BC-UR QR scanner/generator components (SolidJS)
- [ ] Web Crypto API wrapper for hybrid encryption
- [ ] SQLCipher integration for desktop app
- [ ] Questionnaire session management
- [ ] Basic questionnaire templates

**Phase 2**: Integration with Terminology
- [ ] ValueSet-driven question types
- [ ] Code validation in questionnaire responses
- [ ] Autocomplete using local terminology search
- [ ] Questionnaire builder UI

**Phase 3**: Advanced Features
- [ ] Complex question logic (conditional display)
- [ ] Multi-page questionnaires
- [ ] Response export formats (FHIR Questionnaire Response)
- [ ] Questionnaire version management

### Development Considerations

**Code Architecture:**
- SolidJS frontend in Tauri WebView
- Web Crypto API for all encryption
- Rust backend for SQLCipher database operations
- NCTS terminology sync remains separate module

**Testing Strategy:**
- Test questionnaire flow with smallest payload first (~1KB, 1 frame)
- Progress to medium (~3KB, 2 frames) and large (~10KB, 6 frames)
- Verify encryption/decryption round-trip
- Test session expiry and cleanup
- Validate BC-UR frame reconstruction with missing frames

**Dependencies on Current System:**
- Existing NCTS terminology data provides foundation for questionnaires
- ValueSet expansion enables standardized answer options
- Code validation ensures questionnaire responses use valid codes
- Storage patterns (SQLite, directory structure) remain consistent

See **[OPDQs.md](OPDQs.md)** for complete OPDQ technical specification.

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

### Build & Testing

- Use `cargo check` frequently for fast compilation feedback
- Run `cargo test` to execute parser unit tests
- Use `RUST_LOG=debug cargo run` for detailed logging during import
- Database operations are async - always `.await` storage calls
- DateTime handling is critical - use RFC3339 format for SQLite storage

### Import Testing Strategy

**Recommended order** (smallest to largest):
1. **Test with Value Sets first** - smallest (~10MB, few hundred ValueSets)
   - Import time: <1 minute
   - Good for verifying basic import flow
2. **AMT CSV** - moderate size (~50MB, thousands of codes)
   - Import time: 1-2 minutes
   - Tests CSV parsing and single-table import
3. **SNOMED SNAPSHOT** - large (~500MB+, 400k+ concepts)
   - Import time: 5-10 minutes
   - Tests ZIP extraction, multi-file parsing, batch inserts
   - Monitor console for progress (Importing concepts/descriptions/relationships...)

### Performance Characteristics

**Import Performance:**
- Batch size: 1000 records per insert
- SNOMED import: ~400k concepts + ~1M descriptions + ~1M relationships
- Expected database size after full import: ~500MB+
- Memory usage: Moderate (streaming parsers, batch processing)

**Query Performance:**
- Search queries: Milliseconds (with proper indexes)
- Code lookup: <10ms for single concept
- ValueSet expansion: <100ms for typical ValueSet (~100 codes)
- Full-text search: <500ms for common terms

**Optimization Tips:**
- Ensure `imported` flag is checked before re-importing
- Use `limit` parameter in search queries
- Database indexes are created automatically during migration
- Consider SQLite WAL mode for concurrent read performance (future enhancement)

### Debugging Import Issues

```rust
// Enable debug logging
RUST_LOG=debug cargo run

// Common issues:
// 1. "File not found" - Check ZIP extraction succeeded
// 2. "FK constraint failed" - Ensure concepts imported before descriptions
// 3. "Import already in progress" - Check imported flag in database
// 4. Memory issues - Reduce batch size in import.rs (currently 1000)
```

### NCTS-Specific Notes

- Check actual NCTS documentation for correct syndication endpoints
- Monitor console output for feed parsing details and category filtering
- SHA-256 validation automatically checks file integrity
- OAuth token automatically refreshed 60 seconds before expiry

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
