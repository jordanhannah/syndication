# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Tauri desktop application for syncing Australian clinical terminology standards from the National Clinical Terminology Service (NCTS) using Atom feed syndication. This Tauri Project will eventually be combined with the solidJS front end to create OPDQS: Offline Patient-Doctor Questionnaires System/Outpatient Department Questionnaires.

## Stack

SolidJS | Rust + Tauri 2.1 | Redb | Atom Syndication | Tokio async runtime | Azure MFA/RBAC

## Bidirectional QR Code Exchange Protocol

**Clinician:**

1. Select questionnaire
2. Generate animated QR code (Azure session ID, clinician public key, admin telemetry public key, timestamp, encryption metadata (version, encryption method), questionnaire definition)

**Patient:**

3. Scan QR code
4. Complete questionnaire (responses encrypted with AES-256-GCM, session key encrypted with clinician's public key)
5. Generate animated QR code (Azure session ID, encrypted session key, encrypted responses, encrypted telemetry log)

**Clinician:**

6. Scan QR code
7. Decrypt session key with private key, decrypt responses with session key
8. Generate clinical note

**Administrator (separate process):**

9. Decrypt telemetry log with admin private key (clinician cannot access telemetry)
10. Review diagnostic events (QR scan success/fail, encryption errors, validation failures)

## Security & Compliance

**"HIPAA compliant Encryption, Zero Network Exposure"**

- ✅ 100% offline patient device (zero network exposure)
- ✅ Air-gapped QR code transmission
- ✅ Optional de-identified (No-PHI) mode
- ✅ RSA-OAEP-4096 key exchange (dual keypair: clinician + admin)
- ✅ AES-256-GCM payload encryption
- ✅ OS keychain integration
- ✅ SHA-256 integrity verification
- ✅ Azure MFA/RBAC authentication
- ✅ Configurable session expiration
- ✅ Ephemeral patient data on patient device (auto-deleted post-scan)
- ✅ HTTPS/TLS for staff Azure connections
- ✅ Admin-encrypted 7-year telemetry/audit logs (optional Azure sync)
- ✅ Admin/Security Dashboard for Real-time event monitoring
- ✅ Library of Copmliant Questionnaires
- ✅ Input Validation/Sanitization

### Events Captured

- QR: `qr_scan_success/failed`, `qr_generation_complete`
- Encryption: `response_encryption_success/failed`, `session_key_wrap_failed`, `telemetry_encryption_failed`
- Validation: `questionnaire_validation_failed`, `valueset_validation_failed`, `session_expired`
- Performance: `qr_scan_duration`, `encryption_duration`, `qr_generation_duration`
- Errors: `web_crypto_api_error`, `indexeddb_error`, `bc_ur_decode_error`

## Desktop Version (Tauri)

- **Questionnaire Source**: Bundled in app at compile-time and synced with Azure blob storage
- **Sensitive Information Storage**: redb key-value store + AES-256-GCM + OS keychain + SHA-256
  - HIPAA-compliant tamper-evident audit logging
  - Background auto-deletion of expired responses
- **Terminology Storage**: redb key-value store (Rust native) + Tantivy indexes
  - SNOMED, AMT, LOINC, ValueSets
  - NCTS terminology sync
- **Platforms**: Windows, macOS, Linux, iOS, android

## Data Storage Architecture

Snomed RF2 essentials to keep

✅ KEEP (Essential)

- Concepts - Active status filtering
- Descriptions - The actual searchable terms (FSN + synonyms)

❌ REMOVE (Not needed for search)

- Relationships - The massive table you mentioned. For autocomplete/search, you don't need "is-a" hierarchies
- Text Definitions - Rarely used
- Language Refsets - If only supporting English
- Association Refsets - Historical tracking

Your New Architecture Should Be:

// Tantivy index schema (in-memory at runtime)

1. AMT Medications Patient Index (MP PT, TPP TP PT)
2. AMT Medications Doctor Index (MP PT, MPUU PT, TPP TP PT, TPUU PT)
3. SNOMED Problem/Diagnosis Refset (filtered by 32570571000036108)
4. SNOMED All Terms + LOINC

// Minimal redb persistence (just for rebuilding indexes)

- snomed_concepts (concept_id, active)
- snomed_descriptions (concept_id, term, type_id, active) ← YES, KEEP THIS
- amt_codes (code, preferred_term, code_type)
- valuesets (url, title, version, concepts)

Data Flow

SNOMED RF2 files → Parse → Store in redb: - sct2_Concept_Snapshot → snomed_concepts table - sct2_Description_Snapshot → snomed_descriptions table ← ESSENTIAL

On app startup → Load from redb → Build Tantivy indexes: - Filter active descriptions from Problem/Diagnosis refset - Index FSN + synonyms with trigram tokenizer

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

**Index Build Process:**

- Built in-memory at app startup from redb persistence layer
- Incremental updates after terminology sync
- Serialized to disk for fast cold-start (optional optimization)

### FHIR-Compliant Search API

While Tantivy handles the heavy lifting, search commands remain FHIR-compatible from front end point of view. e.g. SolidJS will know 'if online_terminology_server = true do this vs = false do highly optimised tantivy search'.

Frontend SolidJS can wrap results in FHIR ValueSet $expand format if needed.

**Bundle Size Analysis:**

- `qr-scanner`: ~16KB gzipped

## Code Sharing Encouraged between web and app version

### Role-Based Access Control (RBAC)

**Azure AD App Roles:**

Roles defined in Azure AD App Registration manifest:

**Permission Matrix:**

| Action                              | Clinician     | Administrator | Auditor  | DPO      |
| ----------------------------------- | ------------- | ------------- | -------- | -------- |
| **View assigned patient responses** | ✅            | ✅            | ❌       | ✅       |
| **Export patient responses**        | ✅ (own only) | ✅ (all)      | ❌       | ✅       |
| **Delete patient responses**        | ❌            | ❌            | ❌       | ✅       |
| **Create questionnaire sessions**   | ✅            | ✅            | ❌       | ❌       |
| **Manage questionnaire templates**  | ❌            | ✅            | ❌       | ❌       |
| **View audit logs**                 | ❌ (own only) | ✅ (all)      | ✅ (all) | ✅ (all) |
| **Export audit logs**               | ❌            | ❌            | ✅       | ✅       |
| **Manage user roles**               | ❌            | ✅            | ❌       | ❌       |
| **Configure retention policies**    | ❌            | ✅            | ❌       | ✅       |
| **View breach reports**             | ❌            | ❌            | ❌       | ✅       |
| **Initiate emergency data wipe**    | ❌            | ❌            | ❌       | ✅       |

**Azure AD Conditional Access Policies (Recommended):**

- **Require MFA** for all OPDQ app access
- **Require compliant device** (Intune-managed or domain-joined)
- **Block legacy authentication** (only modern OAuth2)
- **Restrict by IP range** (clinic network only, optional)
- **Require password change** every 90 days for Administrators/DPO

**Audit Integration:**

All Azure AD authentication events automatically logged:

- Sign-ins (successful/failed)
- MFA challenges
- Token refresh events
- Conditional Access policy evaluations
- Anomalous activity detection

**Real-time monitoring of security events:**

- Failed decryption attempts (potential brute-force attacks)
- Repeated authentication failures (credential stuffing)
- Unusual access patterns (time-of-day anomalies, geographic anomalies via Azure AD)
- Export volume spikes (potential data exfiltration)
- Audit log integrity violations (tamper detection)
- **Implementation**: Alert administrator on threshold breach (e.g., 5 failed decryptions in 10 minutes)

### Australian Privacy Principles (APP 11.3 - 2024)

[Data Breach Response Plan](https://www.oaic.gov.au/privacy/privacy-guidance-for-organisations-and-government-agencies/preventing-preparing-for-and-responding-to-data-breaches/data-breach-preparation-and-response/part-2-preparing-a-data-breach-response-plan)

**Breach Notification:**

- ✅ Must notify OAIC within 30 days of eligible data breach
- **Recommendation**: Add automated breach detection alerts:
  - Audit log tampering detected (SHA-256 chain verification fails)
  - Encryption key compromise detected (unusual key access patterns)
  - Unauthorized export attempts (export by non-authorized role)
  - Data exfiltration detected (large volume exports outside business hours)
- **Implementation**: Email/SMS alert to Data Protection Officer + automatic incident report generation

**Data Sovereignty:**

- ⚠️ Health data must remain in Australia (for Australian patients)
- **Recommendation**: Add region enforcement:
  - Check device location before storing patient responses (optional, privacy consideration)
  - Use Azure Australia regions for cloud backups (if implemented)
  - Warn administrators if device detected outside Australia (Conditional Access policy)
- **Implementation**: Azure Conditional Access → Block access from outside Australia

**Explicit Consent:**

- ✅ Must obtain explicit consent before collecting health data
- **Recommendation**: Add consent screen in patient app:
  - Clear explanation of data collection ("Your responses will be encrypted and shared only with Dr. [Name]")
  - Checkbox for explicit consent ("I consent to collection of my health information")
  - Option to withdraw consent (refuse to scan QR, responses never transmitted)
- **Implementation**: SolidJS consent component before questionnaire display

### OWASP Mobile Top 10 (2024)

**M1: Improper Credential Usage:**

- ✅ Private keys stored in OS keychain (secure)
- ⚠️ **Enhancement**: Use Hardware Security Module (HSM) when available:
  - **iOS**: Secure Enclave for cryptographic operations
  - **Android**: StrongBox Keymaster (hardware-backed)
  - **macOS**: T2/M-series Secure Enclave
  - **Windows**: TPM 2.0 for key storage
- **Benefit**: Keys never leave hardware, resistant to extraction even with root access

**M2: Inadequate Supply Chain Security:**

- **Recommendation**: Add to CI/CD pipeline:
  - `cargo-audit` - Detect vulnerable Rust dependencies
  - `cargo-deny` - Enforce license and security policies
  - `npm audit` - Check JavaScript dependencies
  - Pin exact versions (avoid `^` or `~` wildcards)
  - Verify checksum of downloaded binaries (Tantivy, redb)
- **Implementation**: GitHub Actions workflow on every commit

**M5: Insecure Communication:**

- ✅ Already addressed (complete air-gap via QR, no network transmission)
- **Enhancement**: Add QR code visual verification:
  - Show first 6 + last 6 characters of session ID on both devices
  - Patient confirms match before submitting responses ("Does your code match: ABC123...XYZ789?")
  - Prevents QR substitution attacks (attacker replaces doctor's QR with their own)

**M6: Inadequate Privacy Controls:**

- **Recommendation**: Add privacy notice in patient app:
  - **Who can access**: "Only Dr. [Name] can decrypt your responses"
  - **Retention period**: "Responses auto-deleted after 30 days or when viewed + exported"
  - **Right to deletion**: "You can refuse to scan the QR code; no data is stored until you submit"
  - **Encryption details**: "Your responses are encrypted with military-grade RSA-4096 + AES-256"
- **Implementation**: Modal dialog before questionnaire, dismissable after reading

**M8: Security Misconfiguration:**

- **Recommendation**: Harden Tauri configuration:
  - ✅ Disable developer tools in production builds (`tauri.conf.json`: `devPath` only in dev)
  - ✅ Disable navigation to external URLs (`allowlist.window.open: false`)
  - ✅ Enable Content Security Policy (CSP) strict mode (no inline scripts)
  - ✅ Disable clipboard access from WebView for PHI screens
  - ⚠️ Add: Disable screenshots on PHI screens (platform-specific APIs)

**M9: Insecure Data Storage:**

- ✅ Already addressed
- **Enhancement**: Defense-in-depth measures:
  - **Clipboard protection**: Clear clipboard after 30 seconds if PHI copied
  - **Screenshot prevention**: Use Tauri API to disable screenshots on sensitive screens (iOS/Android)
  - **Task switcher obfuscation**: Blur PHI when app goes to background
  - **Memory protection**: Use `zeroize` crate to clear decrypted PHI from memory (Rust side)

**M10: Insufficient Cryptography:**

- ✅ Already using industry-standard algorithms (RSA-OAEP-4096, AES-256-GCM, SHA-256)
- **Enhancement**: Add cryptographic agility:
  - Version all encrypted blobs (allow future algorithm migration)
  - Support algorithm upgrades without breaking changes
  - Monitor NIST/OWASP for deprecated algorithms (SHA-1, MD5, RSA-2048)
  - Alert on weak algorithm detection

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

## Documentation

- **[NCTS.md](NCTS.md)** - NCTS-specific details
- **[NCTS_INTEGRATION.md](NCTS_INTEGRATION.md)** - NCTS-specific integration details
