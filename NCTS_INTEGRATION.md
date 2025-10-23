# NCTS Integration Guide

> **Update**: The NCTS uses a unified syndication feed at `https://api.healthterminologies.gov.au/syndication/v1/syndication.xml`. This single feed contains all terminology types, filtered by category terms.

## About NCTS

The National Clinical Terminology Service (NCTS) provides access to Australian clinical terminology standards including:

- **SNOMED CT-AU SNAPSHOT**: Clinical terminology for diseases, findings, procedures
- **LOINC**: ❌ NOT available via syndication (proprietary binary format only)
- **AMT CSV**: Medicines and substances
- **NCTS FHIR R4 Bundles**: Curated value sets in FHIR format

## Syndication vs Direct Download

### This App Uses: Atom Syndication

**Advantages:**
- Automatic discovery of new versions
- Structured metadata (version, date, description)
- Lightweight - just fetch the feed to check for updates
- Standard format (RFC 4287)

**How it works:**
1. App requests unified Atom feed from NCTS (`/syndication/v1/syndication.xml`) with OAuth2 Bearer token
2. Feed contains all terminology releases (~59 entries)
3. App filters entries by category + title:
   - SNOMED: `SCT_RF2_SNAPSHOT` (DELTA not exposed by server)
   - AMT: `AMT_CSV` only
   - FHIR Bundles: `FHIR_Bundle` + title contains "(R4)" + excludes SNOMED reference sets
4. App identifies latest version for requested terminology
5. App downloads that specific version using Bearer authentication
6. App validates downloaded file using SHA-256 checksum from feed

### Alternative: Direct Download

NCTS may also provide direct download links. If syndication feeds are not available, modify the app to:
1. Use known download URLs
2. Parse HTML pages to find download links
3. Use NCTS API if available

## NCTS Access Requirements

### Public Access
Some NCTS resources may be publicly accessible without authentication.

### Registered Access
If authentication is required, you'll need:

1. **NCTS Account**
   - Register at https://www.healthterminologies.gov.au
   - Agree to license terms
   - Obtain credentials

2. **API Key / Token**
   - Generate in NCTS portal
   - Store securely (never commit to git)
   - Use in HTTP headers

3. **License Agreement**
   - Understand usage restrictions
   - Comply with licensing terms
   - Some terminologies have specific usage rules

## Adding Authentication

If NCTS requires authentication, modify [src/ncts.rs](src/ncts.rs):

### Option 1: API Key in Header

```rust
pub fn new_with_api_key(api_key: String) -> Result<Self> {
    use reqwest::header;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        "X-API-Key",
        header::HeaderValue::from_str(&api_key)?
    );

    let client = Client::builder()
        .user_agent("NCTS-Syndication/0.1.0")
        .default_headers(headers)
        .build()?;

    Ok(Self { client })
}
```

### Option 2: Bearer Token

```rust
pub fn new_with_bearer_token(token: String) -> Result<Self> {
    use reqwest::header;

    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        header::HeaderValue::from_str(&format!("Bearer {}", token))?
    );

    let client = Client::builder()
        .user_agent("NCTS-Syndication/0.1.0")
        .default_headers(headers)
        .build()?;

    Ok(Self { client })
}
```

### Option 3: Basic Auth

```rust
pub fn new_with_basic_auth(username: String, password: String) -> Result<Self> {
    let client = Client::builder()
        .user_agent("NCTS-Syndication/0.1.0")
        .build()?;

    // Store credentials for use in requests
    Ok(Self {
        client,
        username: Some(username),
        password: Some(password),
    })
}

// Then in fetch_feed:
let response = if let (Some(user), Some(pass)) = (&self.username, &self.password) {
    self.client.get(&feed_url)
        .basic_auth(user, Some(pass))
        .send()
        .await?
} else {
    self.client.get(&feed_url)
        .send()
        .await?
};
```

### Secure Credential Storage

Use Tauri's keyring plugin to store credentials securely:

```bash
# Add dependency
cargo add tauri-plugin-keyring
```

```rust
// In main.rs
use tauri_plugin_keyring::Keyring;

// Store credential
let keyring = Keyring::new("ncts_syndication", "api_key");
keyring.set_password(&api_key)?;

// Retrieve credential
let api_key = keyring.get_password()?;
```

## Verifying NCTS Endpoints

The endpoints in this app are illustrative. To find actual NCTS endpoints:

### 1. Check NCTS Documentation

Visit: https://www.healthterminologies.gov.au/

Look for:
- API documentation
- Syndication feeds
- Download section
- Developer resources

### 2. Test Endpoints

```bash
# Test if endpoint is accessible (requires OAuth2 token)
curl -I https://api.healthterminologies.gov.au/syndication/v1/syndication.xml

# If successful (HTTP 200):
HTTP/2 200
content-type: application/atom+xml

# If authentication required (HTTP 401):
HTTP/2 401
www-authenticate: Bearer realm="NCTS"
```

### 3. Inspect Feed Format

```bash
# Fetch and view feed
curl https://www.healthterminologies.gov.au/syndication/v3/snomed-ct-au

# You should see XML like:
<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>SNOMED CT-AU Releases</title>
  <link rel="self" href="..."/>
  <updated>2025-01-15T00:00:00Z</updated>
  <entry>
    <title>SNOMED CT-AU 20250131</title>
    <id>snomed-au-20250131</id>
    <updated>2025-01-15T00:00:00Z</updated>
    <link rel="enclosure" href="https://.../snomed-au-20250131.zip"/>
  </entry>
</feed>
```

## Common Issues

### Issue: 401 Unauthorized

**Cause**: NCTS requires authentication

**Solution**:
1. Register for NCTS access
2. Obtain API key/token
3. Implement authentication (see above)
4. Store credentials securely

### Issue: 404 Not Found

**Cause**: Endpoint URL is incorrect

**Solution**:
1. Verify endpoints in NCTS documentation
2. Update `NCTS_BASE_URL` and feed URLs in [src/ncts.rs](src/ncts.rs)
3. Check for API versioning (v2, v3, etc.)

### Issue: 403 Forbidden

**Cause**: Valid authentication but insufficient permissions

**Solution**:
1. Check NCTS account status
2. Verify license agreements are signed
3. Ensure terminology-specific access is granted
4. Contact NCTS support

### Issue: SSL/TLS Errors

**Cause**: Certificate validation issues

**Solution**:
```rust
// Only for development/testing - NOT for production
let client = Client::builder()
    .danger_accept_invalid_certs(true)  // NOT RECOMMENDED
    .build()?;

// Better: Add CA certificate
let cert = std::fs::read("ncts_ca.pem")?;
let cert = reqwest::Certificate::from_pem(&cert)?;

let client = Client::builder()
    .add_root_certificate(cert)
    .build()?;
```

### Issue: Feed Parse Errors

**Cause**: NCTS uses non-standard Atom format

**Solution**:
Inspect the actual feed and adjust parsing in `FeedEntry::from_atom_entry`:

```rust
// Handle custom elements
pub fn from_atom_entry(entry: &Entry) -> Self {
    // Check for NCTS-specific extensions
    let version = entry
        .extensions()
        .get("version")
        .and_then(|ext| ext.value())
        .map(|s| s.to_string());

    let effective_date = entry
        .extensions()
        .get("effectiveDate")
        .and_then(|ext| ext.value())
        .map(|s| s.to_string());

    // ... rest of parsing
}
```

### Issue: Download Timeout

**Cause**: Large file, slow connection

**Solution**:
```rust
// Increase timeout
let client = Client::builder()
    .timeout(std::time::Duration::from_secs(600))  // 10 minutes
    .build()?;

// Or show progress
pub async fn download_with_progress(
    &self,
    url: &str,
    destination: &Path,
) -> Result<()> {
    let response = self.client.get(url).send().await?;
    let total_size = response.content_length().unwrap_or(0);

    let mut file = tokio::fs::File::create(destination).await?;
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();

    use futures::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;

        let progress = (downloaded as f64 / total_size as f64) * 100.0;
        println!("Downloaded: {:.1}%", progress);
    }

    Ok(())
}
```

## NCTS-Specific Terminology Notes

### SNOMED CT-AU
- Australian extension of SNOMED CT
- Includes Australian clinical terms
- Updated quarterly
- Large file size (~500MB+)
- RF2 format (Release Format 2)

### LOINC
- Laboratory observation codes
- International standard
- Updated biannually
- Moderate file size (~100MB)
- CSV/TXT format

### AMT
- Australian-specific medicines terminology
- Includes PBS and ARTG codes
- Updated monthly
- Moderate file size (~50MB)
- CSV format

### Value Sets
- Curated code sets for specific purposes
- Used in FHIR profiles
- Updated as needed
- Small file size (<10MB)
- JSON/XML format

## Integration Testing

### Test Checklist

- [ ] Can connect to NCTS endpoints
- [ ] Can authenticate (if required)
- [ ] Can fetch Atom feed
- [ ] Can parse feed entries
- [ ] Can download small test file
- [ ] Can download full terminology
- [ ] Can verify file integrity (checksum)
- [ ] Can handle network errors gracefully
- [ ] Can resume interrupted downloads
- [ ] Can detect duplicate downloads

### Mock NCTS Server for Testing

Create a local mock server:

```rust
// In tests/
use axum::{routing::get, Router};

async fn mock_snomed_feed() -> String {
    r#"<?xml version="1.0"?>
    <feed xmlns="http://www.w3.org/2005/Atom">
        <title>SNOMED CT-AU Test Feed</title>
        <entry>
            <title>Test Version 20250101</title>
            <id>test-version-1</id>
            <updated>2025-01-01T00:00:00Z</updated>
            <link rel="enclosure" href="http://localhost:3000/download/test.zip"/>
        </entry>
    </feed>"#.to_string()
}

#[tokio::test]
async fn test_with_mock_ncts() {
    let app = Router::new()
        .route("/syndication/v3/snomed-ct-au", get(mock_snomed_feed));

    // Start server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Test your client against localhost:3000
}
```

## Compliance and Licensing

### Important Legal Considerations

1. **SNOMED CT**: Requires SNOMED CT license
   - Free for some countries (including Australia)
   - Affiliate licensing for commercial use
   - Check: https://www.snomed.org/

2. **LOINC**: Free but requires agreement
   - Must agree to LOINC license
   - No commercial redistribution
   - Check: https://loinc.org/

3. **AMT**: Australian Government resource
   - Check licensing terms for your use case
   - Commercial use may have restrictions

4. **Your Obligations**:
   - Don't redistribute terminology files
   - Don't expose public APIs with raw terminology
   - Include proper attributions
   - Follow license terms strictly

## Getting Help

### NCTS Support
- Website: https://www.healthterminologies.gov.au
- Email: Check NCTS website for support contact
- Documentation: Look for API docs and user guides

### This App
- Check [README.md](README.md) for general documentation
- Review [QUICKSTART.md](QUICKSTART.md) for setup guide
- Open GitHub issue for bugs/features

### Community Resources
- FHIR Chat: https://chat.fhir.org
- HL7 Australia: https://www.hl7.org.au
- SNOMED CT Forums: https://forums.ihtsdotools.org

## Next Steps

1. **Verify NCTS Access**
   - Can you access www.healthterminologies.gov.au?
   - Do you have (or need) an account?
   - What terminologies do you need?

2. **Test Connectivity**
   ```bash
   # Try fetching a feed
   curl https://www.healthterminologies.gov.au/syndication/v3/snomed-ct-au
   ```

3. **Update Configuration**
   - If endpoints differ, update [src/ncts.rs](src/ncts.rs)
   - If auth needed, implement authentication
   - Test with small terminology first (Value Sets)

4. **Deploy and Monitor**
   - Start with manual syncs
   - Monitor for errors
   - Check file sizes and download times
   - Plan for automatic syncing

---

**Remember**: This app is a starting point. NCTS integration details may change, and you'll need to adapt based on actual NCTS API specifications and your specific access arrangements.
