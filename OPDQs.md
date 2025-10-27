# Offline Patient-Doctor Questionnaire (OPDQ) System

## Bidirectional QR Code Exchange Protocol

**Clinician:**
**add timestamp/more encryption info below?**

1. Select questionnaire
2. Generate animated QR code (Azure session ID, clinician public key, admin telemetry public key, questionnaire definition)

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
- ✅ Sanitized user input, error messages and file paths

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
