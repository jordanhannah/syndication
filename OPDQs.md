Enhanced Workflow with Two-Way QR Exchange

1. Doctor Selects Questionnaire: On the desktop app, the doctor chooses a questionnaire template.

2. Doctor animated QR Code Generation: The app generates a session, including a new public/private key pair. It then creates QR codes containing the questionnaire structure, the doctor's public key, and a unique session ID.

3. Patient Scans & Fills: The patient scans this QR code with their tablet, which loads the questionnaire. They then fill out their answers directly on the tablet.

4. On-Device Encryption: The patient's tablet generates a random, one-time AES key and uses it to encrypt the questionnaire responses. It then encrypts that AES key using the doctor's public key (from step 2).

5. Animated QR Code Generation: The encrypted responses and the encrypted AES key are combined and encoded into a series of animated QR codes for transmission.

6. Doctor Scans Responses: The doctor's computer scans the animated QR codes from the patient's tablet.

7. Decryption and Viewing: The doctor's app uses its corresponding private key to decrypt the AES key, and then uses that AES key to decrypt and securely view the patient's answers.

HIPAA Benefits:

- ✅ No pre-shared secrets - keys exchanged via visual QR
- ✅ End-to-end encryption - only intended doctor can decrypt
- ✅ Zero network transmission - complete air-gap
- ✅ Patient controls sharing - scan only the doctor they choose
- ✅ Fresh keys per session - can use ephemeral keys
- ✅ Simple UX - two scans, that's it

Marketing:

"Enterprise-Grade Encryption, Zero Network Risk"

- ✅ Optional No-PHI mode
- ✅ RSA-OAEP-4096 for key exchange
- ✅ AES-256-GCM for data encryption (Only the intended staff can decrypt responses)
- ✅ OS keychain integration
- ✅ SHA-256 for checksums and audit trails
- ✅ Web Crypto API usage
- ✅ QR code 100% offline patient data transmission
- ✅ HIPAA Compliant
- ✅ Azure Multi-Factor Authentication/Role-Based Access
- ✅ Custom Session expiry time
- ✅ 7-year Encrypted audit logs (optional Azure Upload)
- ✅ Single-use patient data (deleted post QR scanning)
- ✅ Staff apps use HTTPS/TLS for Azure access
- ✅ (Optional) Patient Device 100% Offline (De-identified Audit Logs encrypted/stored locally on-device)
- ✅ Real-time monitoring of security events

Database Schemas

## Doctor's Device (Clinician Side - IndexedDB with idb-keyval)

**Architecture**: Uses **IndexedDB** for questionnaire responses/keys with **idb-keyval** simple API wrapper.
**Tamper Detection**:

- Each audit entry contains `previous_log_hash` (SHA-256 of previous entry)
- Chain verification: Walk backward through IndexedDB cursor to detect modifications
- Append-only writes (entries never updated, only added)
- Integrity check on app startup (iterate all entries, verify hash chain)

**Auto-Deletion Policy**:

- Patient responses auto-delete after viewing + export OR 30 days (whichever comes first)
- Audit logs retained for 7 years (HIPAA requirement)
- Background task periodically scans `auto_delete_at` field and deletes expired responses
- Deleted responses leave audit trail entry with `action = 'deleted'`

**Note**: Encryption keys NEVER stored in IndexedDB - stored in OS keychain only (desktop) or SubtleCrypto with extractable=false (web).

**Note**: `my_responses` store cleared immediately after QR generation. No long-term storage on patient device.

Platform Architecture

## Web Version (SolidJS Standalone)

- **Questionnaire Source**: Downloads from Azure on-demand via HTTPS
- **OPDQ Storage**: IndexedDB with idb-keyval wrapper
  - Questionnaire sessions, patient responses, audit logs
  - Same TypeScript interfaces as desktop version
  - Browser-native IndexedDB (no polyfills needed)
- **Encryption**: Web Crypto API (built-in)
  - RSA-OAEP-4096 + AES-256-GCM for responses
  - CryptoKey storage with `extractable: false` (non-exportable keys)
- **Platforms**: Browser (Chrome/Safari/Firefox), Mobile devices (iOS/Android via PWA)

## Desktop Version (Tauri)

- **Questionnaire Source**: Bundled in app at compile-time
- **OPDQ Storage**: IndexedDB (via WebView) with idb-keyval wrapper
  - Patient responses, sessions, audit logs, questionnaire templates
  - Persisted to disk by WebView engine (encrypted at OS level on desktop)
  - 100% shared TypeScript code with web version
- **Terminology Storage**: redb key-value store (Rust native) + Tantivy indexes
  - SNOMED, AMT, LOINC, ValueSets
  - SQLite for initial import, then loaded into redb for fast lookups
  - Tantivy indexes built from redb for sub-millisecond search
- **Encryption**:
  - Web Crypto API in WebView for OPDQ data (same as web version)
  - OS keychain for RSA private keys (Tauri keyring API)
  - Optional: OS-level disk encryption (BitLocker, FileVault)
- **Additional Features**:
  - NCTS terminology sync (separate SQLite → redb pipeline)
  - Tantivy search indexes for terminology lookups (<1ms response time)
  - HIPAA-compliant tamper-evident audit logging with SHA-256 chain
  - Background auto-deletion of expired responses
- **Platforms**: Windows, macOS, Linux

## Data Storage Architecture

**Three Specialized Search Indexes:**

1. **AMT Medications Index** (~10MB RAM)

   - All AMT codes (CTPP, TPP, TPUU, MPP, MPUU, MP)
   - Fields: code, preferred_term, code_type
   - Trigram tokenizer for typo tolerance ("paracetamol" → "paracetmol")

2. **SNOMED Problem/Diagnosis Refset** (~50MB RAM)

   - Filtered by Australian Problem/Diagnosis refset (32570571000036108)
   - Used for past medical history autocomplete
   - Fields: concept_id, FSN, synonyms
   - Prefix-boosted ranking (exact matches first)

3. **SNOMED All Terms + LOINC** (~250MB RAM)
   - All active SNOMED descriptions
   - LOINC codes (when available via NCTS)
   - ValueSet metadata for search
   - Fields: code, system, display, terminology_type

**Index Build Process:**

- Built in-memory at app startup from redb persistence layer
- Incremental updates after terminology sync
- Serialized to disk for fast cold-start (optional optimization)

### FHIR-Compliant Search API

While Tantivy handles the heavy lifting, search commands remain FHIR-compatible from front end point of view. e.g. SolidJS will know 'if online_terminology_server = true do this vs = false do highly optimised tantivy search'.

Frontend SolidJS can wrap results in FHIR ValueSet $expand format if needed.

**Bundle Size Analysis:**

- `idb`: ~4KB gzipped
- `idb-keyval`: <600 bytes gzipped (even simpler API)
- `qr-scanner`: ~16KB gzipped
- **Total OPDQ overhead**: ~100KB gzipped

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

- ✅ Already addressed (Web Crypto API application-level encryption + OS disk encryption on desktop)
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

### Additional Best Practices

**Secure Coding (OWASP):**

**Input Validation:**

- Sanitize all questionnaire responses before storage (prevent XSS injection in IndexedDB)
- Validate BC-UR frame format before decoding (reject malformed frames)
- Limit response field lengths (prevent DoS via memory exhaustion)
- Whitelist allowed characters in session IDs, questionnaire IDs (UUID v4 format)

**Memory Safety:**

- Zero-out sensitive data after use (encryption keys, decrypted PHI)
- Use `zeroize` crate for secure memory clearing in Rust (prevents data remanence)
- JavaScript: Clear sensitive variables after use (set to `null`)
- Avoid logging decrypted responses (even in debug mode)
- Clear ArrayBuffers after encryption/decryption operations

**Operational Security:**

**Backup & Recovery:**

- Encrypted backups of audit logs (7-year retention for HIPAA/APP compliance)
- Export audit logs to immutable storage (Azure Blob WORM, S3 Glacier)
- Test recovery procedures annually (restore audit logs, verify integrity)
- Separate backup encryption key from operational key

**Incident Response:**

- Document procedure for lost/stolen devices
- Remote wipe capability for compromised devices (Azure Intune integration)
- Revoke encryption keys on device compromise (rotate master key, re-encrypt data)
- Notify affected patients within 60 days (HIPAA Breach Notification Rule)

**Physical Security (APP Compliance):**

**Device Security:**

- Require device encryption (OS-level): BitLocker (Windows), FileVault (macOS)
- Require screen lock with timeout (≤5 minutes idle)
- Warn if device is jailbroken/rooted (Tauri device integrity check)
- Enforce device compliance via Azure Conditional Access

### Implementation Priority

**Critical (Implement Before Production):**

1. ✅ Azure AD authentication with MFA
2. ✅ Role-based access control (RBAC)
3. ✅ Breach detection alerts (tamper detection, failed auth)
4. ✅ Privacy consent screen (patient app)
5. ✅ Secure memory clearing (`zeroize` for keys/PHI)

**High Priority (Next Release):**

1. ⚠️ HSM/Secure Enclave integration (hardware key storage)
2. ⚠️ Data sovereignty enforcement (Azure Conditional Access geo-blocking)
3. ⚠️ Screenshot/clipboard protection (platform APIs)
4. ⚠️ Dependency security scanning (CI/CD integration)
5. ⚠️ QR visual verification (session ID display)

**Medium Priority (Future Enhancement):**

1. ⚠️ Continuous security monitoring (anomaly detection)
2. ⚠️ Remote wipe capability (Azure Intune)
3. ⚠️ Encrypted audit log backup (Azure Blob WORM)
4. ⚠️ Cryptographic algorithm versioning (future-proofing)
