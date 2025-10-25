Enhanced Workflow with Two-Way QR Exchange

Phase 1: Setup (Patient → Doctor)

1. Patient's tablet scans QR code from doctor's computer
2. QR contains:

   - Questionnaire definition (questions, options, validation rules)
   - Doctor's public encryption key
   - Session identifier
   - Clinic/doctor metadata

Phase 2: Completion (Patient → Doctor)

3. Patient answers questionnaire on tablet
4. Responses encrypted using doctor's public key
5. Patient's tablet generates animated QR sequence with encrypted responses (BC-UR format)
6. Doctor's computer scans animated QR sequence and decrypts with private key

HIPAA Benefits:

- ✅ No pre-shared secrets - keys exchanged via visual QR
- ✅ End-to-end encryption - only intended doctor can decrypt
- ✅ Zero network transmission - complete air-gap
- ✅ Patient controls sharing - scan only the doctor they choose
- ✅ Fresh keys per session - can use ephemeral keys
- ✅ Simple UX - two scans, that's it

QR Code #1: Doctor → Patient (Session Initiation)

{
"type": "questionnaire_session",
"version": 1,
"session_id": "uuid-v4",
"timestamp": 1729900800,
"doctor": {
"id": "dr-uuid",
"name": "Dr. Smith", // optional, for patient confirmation
"clinic": "Melbourne Medical Centre" // optional
},
"questionnaire": {
"id": "diabetes-screening-v2",
"title": "Diabetes Screening Questionnaire",
"version": "2.0",
"questions": [
{
"id": "q1",
"text": "Do you have a family history of diabetes?",
"type": "boolean",
"required": true
},
{
"id": "q2",
"text": "What is your activity level?",
"type": "choice",
"options": ["Sedentary", "Moderate", "Active"],
"required": true
}
// ... more questions
]
},
"encryption": {
"algorithm": "RSA-OAEP-4096", // or "X25519" for smaller QR
"public_key": "base64_public_key"
},
"expires_at": 1729904400 // 1 hour validity
}

QR Code #2: Patient → Doctor (Response Submission - BC-UR Animated QR)

Note: Response is ALWAYS transmitted via BC-UR animated QR sequence (fountain-coded frames).
This eliminates format selection logic and handles all payload sizes gracefully.

BC-UR Frame Format:
ur:crypto-response/1-5/lpadascf... (frame 1 of ~5)
ur:crypto-response/2-5/lpaoascf... (frame 2 of ~5)
ur:crypto-response/3-5/lpaxascf... (frame 3 of ~5)
...

Each frame contains:
{
"session_id": "same-uuid-from-qr1",
"questionnaire_id": "diabetes-screening-v2",
"encrypted_payload": "base64_encrypted_data_chunk",
"encryption": {
"algorithm": "RSA-OAEP-4096",
"ephemeral_key": "base64_aes_key_encrypted_with_rsa" // hybrid encryption
}
}

Hybrid Encryption Strategy (used for all questionnaires):

1. Generate random AES-256-GCM key
2. Encrypt responses with AES key
3. Encrypt AES key with doctor's RSA public key
4. BC-UR encodes the encrypted payload into fountain-coded frames

Benefits:
- Small questionnaires (~1KB): 1 frame, scans in ~100ms
- Medium questionnaires (~3KB): 2 frames, scans in ~200ms
- Large questionnaires (~10KB): 6 frames, scans in ~600ms
- No size limits, no format guessing, single code path

Database Schemas

Doctor's Device (Clinician Side):

-- Store active sessions
CREATE TABLE questionnaire_sessions (
id TEXT PRIMARY KEY, -- UUID
questionnaire_id TEXT NOT NULL,
public_key TEXT NOT NULL,
private_key_ref TEXT NOT NULL, -- reference to OS keychain
created_at TEXT NOT NULL,
expires_at TEXT NOT NULL,
status TEXT NOT NULL DEFAULT 'pending' -- pending, completed, expired
);

-- Store received responses (encrypted at rest with SQLCipher)
CREATE TABLE patient_responses (
id INTEGER PRIMARY KEY AUTOINCREMENT,
session_id TEXT NOT NULL,
questionnaire_id TEXT NOT NULL,
received_at TEXT NOT NULL,
encrypted_responses BLOB NOT NULL, -- double-encrypted: AES + SQLCipher
encryption_nonce BLOB NOT NULL,
clinician_id TEXT NOT NULL,
viewed BOOLEAN DEFAULT 0,
exported BOOLEAN DEFAULT 0,
FOREIGN KEY (session_id) REFERENCES questionnaire_sessions(id)
);

-- Audit log
CREATE TABLE access_log (
id INTEGER PRIMARY KEY AUTOINCREMENT,
response_id INTEGER NOT NULL,
action TEXT NOT NULL, -- 'received', 'viewed', 'exported', 'deleted'
clinician_id TEXT NOT NULL,
timestamp TEXT NOT NULL,
device_id TEXT NOT NULL,
FOREIGN KEY (response_id) REFERENCES patient_responses(id)
);

-- Questionnaire templates
CREATE TABLE questionnaires (
id TEXT PRIMARY KEY,
title TEXT NOT NULL,
version TEXT NOT NULL,
definition TEXT NOT NULL, -- JSON questionnaire schema
created_at TEXT NOT NULL,
updated_at TEXT NOT NULL,
active BOOLEAN DEFAULT 1
);

Patient's Device (Patient Side):

-- Store questionnaire sessions (temporary)
CREATE TABLE active_sessions (
id TEXT PRIMARY KEY,
questionnaire_id TEXT NOT NULL,
doctor_name TEXT,
clinic_name TEXT,
doctor_public_key TEXT NOT NULL,
scanned_at TEXT NOT NULL,
expires_at TEXT NOT NULL,
questionnaire_data TEXT NOT NULL -- JSON definition
);

-- Store patient's responses before QR generation (optional, can clear after)
CREATE TABLE my_responses (
id INTEGER PRIMARY KEY AUTOINCREMENT,
session_id TEXT NOT NULL,
questionnaire_id TEXT NOT NULL,
responses TEXT NOT NULL, -- encrypted JSON
created_at TEXT NOT NULL,
qr_generated BOOLEAN DEFAULT 0,
FOREIGN KEY (session_id) REFERENCES active_sessions(id)
);

Platform Architecture

## Web Version (SolidJS Standalone)

- **Questionnaire Source**: Downloads from Azure on-demand
- **Storage**: IndexedDB for temporary sessions
- **Encryption**: Web Crypto API (built-in)
- **Platforms**: Browser (Chrome/Safari/Firefox), Mobile devices (iOS/Android)

## Desktop Version (Tauri)

- **Questionnaire Source**: Bundled in app at compile-time
- **Storage**: SQLCipher encrypted database, auto-deletes after viewing/exporting
- **Encryption**: Web Crypto API in WebView (same code as web version)
- **Additional Features**: NCTS terminology sync (separate SQLite database)
- **Platforms**: Windows, macOS, Linux

Implementation Dependencies

## JavaScript/TypeScript (Shared - Both Platforms)

```json
{
  "dependencies": {
    "solid-js": "^1.8.0",
    "qr-scanner": "^1.4.2", // QR scanning
    "qrcode": "^1.5.3", // QR generation
    "@ngraveio/bc-ur": "^1.1.12", // Animated QR (fountain codes)
    "uuid": "^10.0.0"
  }
}
```

**Note**: Web Crypto API is built-in (no crypto libraries needed)

## Rust (Desktop/Tauri Only)

```toml
[dependencies]
rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
keyring = "3.8"                        # Store DB encryption key in OS keychain
uuid = { version = "1.11", features = ["v4", "serde"] }
serde_json = "1.0"
```

**Note**: Crypto handled by Web Crypto API in frontend, not Rust

BC-UR Animated QR Performance (using qr-scanner)

Performance Characteristics

- Speed: Commercial scanners like Scanbot can scan in under 0.04s (40ms) per frame
- BC-UR Record: The fastest recorded transfer of ~13KB using fountain-coded animated QR was 501ms (half a second) at
  12 FPS with 1850 bytes per QR code
- Throughput: This translates to approximately 25 kbps
- Frame Rate: Typical implementations run at 10-15 FPS for reliable scanning

Why qr-scanner Works Well

- WebWorker-based: Runs in a background thread, keeping UI responsive
- High detection rate: 2-3x (up to 8x) better than older libraries like jsqrcode
- Low overhead: 16 KB gzipped, minimal performance impact

Key Advantage of BC-UR Fountain Codes

- Stateless scanning: Can start scanning at any frame (don't need frame 1)
- Missing frame tolerance: Each frame contains redundant data, so you don't need every single frame
- Probabilistic completion: You need slightly more than K frames to decode K source blocks

Architecture Decision: BC-UR Only (No Single QR Mode)

This app uses BC-UR animated QR for ALL response submissions (no single static QR mode).

Benefits:
- Eliminates format selection logic (no size estimation needed)
- Single code path for both patient and doctor apps
- Handles all payload sizes gracefully (1 KB to 100 KB+)
- Minimal overhead for small payloads (1 frame = ~100ms scan, barely slower than static QR)
- Future-proof for complex questionnaires

Trade-offs:
- Slightly slower for tiny questionnaires (~50ms overhead for BC-UR decode)
- Requires 96 KB bundle (qr-scanner + qrcode + bc-ur) vs 66 KB for single QR only
- Camera must stay steady for 0.5-2 seconds (vs quick snapshot for static QR)

Conclusion: The architectural simplicity and elimination of edge cases outweigh the minor performance cost.

SolidJS Implementation Patterns

## Key Patterns

1. **Camera Lifecycle**: Use `createEffect()` to reactively control camera state
2. **BC-UR Decoder**: Always use BC-UR decoder for all responses (no single QR mode)
3. **Progressive Init**: Show loading state while libraries initialize
4. **Progress Feedback**: Show frame count and percentage for multi-frame responses

## Component Structure

```
src/
├── components/
│   ├── BCURScanner.tsx          // BC-UR frame scanner (always animated)
│   ├── BCURGenerator.tsx        // BC-UR frame display (rotating QR)
│   ├── QuestionnaireForm.tsx    // Patient questionnaire UI
│   └── ResponseViewer.tsx       // Clinician response viewer
├── utils/
│   ├── crypto.ts                // Web Crypto API (RSA/AES hybrid encryption)
│   ├── bcur.ts                  // BC-UR encoding/decoding wrapper
│   ├── storage.ts               // IndexedDB (web) / Tauri commands (desktop)
│   └── qr-protocol.ts           // QR payload schemas
└── App.tsx
```

## Code Sharing Strategy

- **100% shared**: SolidJS components, Web Crypto API, BC-UR handling
- **Platform-specific**: Storage persistence only (IndexedDB vs Tauri SQL commands)
- **Desktop-only**: NCTS terminology sync (separate Rust module)

## Simplified Flow (BC-UR Only)

**Patient App:**
```typescript
// 1. Encrypt responses with hybrid encryption
const aesKey = crypto.getRandomValues(new Uint8Array(32));
const encryptedData = await aesEncrypt(responses, aesKey);
const encryptedKey = await rsaEncrypt(aesKey, doctorPublicKey);

// 2. Always encode with BC-UR (no format decision)
const encoder = new UREncoder(encryptedPayload, 1850);
const totalFrames = encoder.fragmentsLength;

// 3. Display rotating frames
setInterval(() => {
  const frame = encoder.nextPart();
  displayQR(frame);
}, 100); // 10 FPS
```

**Doctor App:**
```typescript
// Always use BC-UR decoder (no format detection)
const decoder = new URDecoder();

scanner.start(frame => {
  decoder.receivePart(frame);

  // Show progress
  const progress = decoder.estimatedPercentComplete();
  updateUI(`Scanning: ${progress}% - Frame ${decoder.receivedPartCount()}`);

  if (decoder.isComplete()) {
    scanner.stop();
    const encryptedPayload = decoder.resultUR();
    const responses = await decryptWithPrivateKey(encryptedPayload);
    displayResponses(responses);
  }
});
```
