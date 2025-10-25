use crate::auth::TokenManager;
use anyhow::{Context, Result};
use atom_syndication::{Entry, Feed};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// NCTS syndication endpoint
pub const SYNDICATION_FEED_URL: &str = "https://api.healthterminologies.gov.au/syndication/v1/syndication.xml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TerminologyType {
    Snomed,
    Loinc,
    ValueSets,
    Amt,
}

impl TerminologyType {
    /// Returns the category terms used in NCTS feed to identify this terminology type
    pub fn category_terms(&self) -> Vec<&str> {
        match self {
            TerminologyType::Snomed => vec!["SCT_RF2_SNAPSHOT"], // Only SNAPSHOT - server doesn't expose DELTA
            TerminologyType::Loinc => vec!["LOINC"], // Not available - proprietary binary only
            TerminologyType::ValueSets => vec!["FHIR_Bundle"], // FHIR R4 Bundles only
            TerminologyType::Amt => vec!["AMT_CSV"], // CSV format only
        }
    }

    /// Checks if an Atom entry category matches this terminology type
    pub fn matches_category(&self, category_term: &str) -> bool {
        self.category_terms().contains(&category_term)
    }

    pub fn display_name(&self) -> &str {
        match self {
            TerminologyType::Snomed => "SNOMED CT-AU",
            TerminologyType::Loinc => "LOINC",
            TerminologyType::ValueSets => "Value Sets",
            TerminologyType::Amt => "Australian Medicines Terminology",
        }
    }

    /// Additional title-based filtering for entries (used for FHIR Bundles)
    /// Returns true if the entry should be included based on its title
    pub fn matches_title(&self, title: &str) -> bool {
        match self {
            TerminologyType::ValueSets => {
                // Must be R4 and must NOT be a SNOMED reference set bundle
                title.contains("(R4)") && !title.contains("SNOMED CT-AU Reference Set")
            }
            _ => true, // No title filtering for other types
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedEntry {
    pub id: String,
    pub title: String,
    pub updated: DateTime<Utc>,
    pub published: Option<DateTime<Utc>>,
    pub summary: Option<String>,
    pub download_url: Option<String>,
    pub version: Option<String>,
    pub effective_date: Option<String>,
    // NCTS extension fields (CP 94-100)
    pub content_item_identifier: Option<String>,
    pub content_item_version: Option<String>,
    pub sha256_hash: Option<String>,
    pub sct_base_version: Option<String>,  // For SNOMED Delta releases
    pub fhir_profile: Option<Vec<String>>, // Can have multiple profiles
    pub bundle_interpretation: Option<String>, // "batch" or "collection"
}

impl FeedEntry {
    /// Parse an Atom feed entry into our FeedEntry structure
    pub fn from_atom_entry(entry: &Entry) -> Self {
        let download_url = entry
            .links()
            .iter()
            .find(|link| link.rel() == "enclosure" || link.rel() == "alternate")
            .map(|link| link.href().to_string());

        let summary = entry.summary().map(|s| s.value.to_string());

        // Parse NCTS extension elements from raw XML
        let ncts_extensions = Self::parse_ncts_extensions(entry);

        Self {
            id: entry.id().to_string(),
            title: entry.title().value.to_string(),
            updated: entry.updated().to_utc(),
            published: entry.published().map(|p| p.to_utc()),
            summary,
            download_url,
            version: ncts_extensions.content_item_version.clone(),
            effective_date: None, // Could extract from title or content if needed
            content_item_identifier: ncts_extensions.content_item_identifier,
            content_item_version: ncts_extensions.content_item_version,
            sha256_hash: ncts_extensions.sha256_hash,
            sct_base_version: ncts_extensions.sct_base_version,
            fhir_profile: ncts_extensions.fhir_profile,
            bundle_interpretation: ncts_extensions.bundle_interpretation,
        }
    }

    /// Parse NCTS extension elements from Atom entry extensions
    fn parse_ncts_extensions(entry: &Entry) -> NctsExtensions {
        let mut result = NctsExtensions::default();

        // Access the extension map - extensions() returns a HashMap-like structure
        let ext_map = entry.extensions();

        // Look for NCTS namespace extensions
        // The extension map is namespace -> (name -> Vec<Extension>)
        for (_namespace, name_map) in ext_map.iter() {
            // Check if this is the NCTS namespace or root namespace
            for (name, ext_list) in name_map.iter() {
                // Get the first extension's value if present
                if let Some(ext) = ext_list.first() {
                    let value = ext.value().map(|v| v.to_string());

                    match name.as_str() {
                        "contentItemIdentifier" => result.content_item_identifier = value,
                        "contentItemVersion" => result.content_item_version = value,
                        "sha256Hash" => result.sha256_hash = value,
                        "sctBaseVersion" => result.sct_base_version = value,
                        "bundleInterpretation" => result.bundle_interpretation = value,
                        "fhirProfile" => {
                            if let Some(val) = value {
                                result.fhir_profile.get_or_insert_with(Vec::new).push(val);
                            }
                        }
                        _ => {} // Ignore unknown extensions
                    }
                }
            }
        }

        result
    }
}

/// Helper struct for NCTS extension data
#[derive(Default)]
struct NctsExtensions {
    content_item_identifier: Option<String>,
    content_item_version: Option<String>,
    sha256_hash: Option<String>,
    sct_base_version: Option<String>,
    fhir_profile: Option<Vec<String>>,
    bundle_interpretation: Option<String>,
}

pub struct NctsClient {
    client: Client,
    token_manager: TokenManager,
}

impl NctsClient {
    pub fn new(token_manager: TokenManager) -> Result<Self> {
        let client = Client::builder()
            .user_agent("NCTS-Syndication/0.1.0")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            token_manager,
        })
    }

    /// Fetch the unified NCTS Atom feed and filter for a specific terminology type
    pub async fn fetch_feed(
        &self,
        terminology_type: TerminologyType,
    ) -> Result<Vec<FeedEntry>> {
        println!("Fetching unified feed from: {}", SYNDICATION_FEED_URL);
        println!("Filtering for: {}", terminology_type.display_name());

        // Get access token
        let token = self.token_manager.get_token().await
            .context("Failed to obtain access token")?;

        let response = self
            .client
            .get(SYNDICATION_FEED_URL)
            .bearer_auth(token)
            .header("Accept", "application/atom+xml")
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to fetch feed: HTTP {}", response.status());
        }

        let feed_text = response.text().await
            .context("Failed to read response")?;
        let feed = feed_text.parse::<Feed>()
            .context("Failed to parse Atom feed")?;

        // Filter entries by category and title
        let entries: Vec<FeedEntry> = feed
            .entries()
            .iter()
            .filter(|entry| {
                // Check if any category matches our terminology type
                let category_match = entry.categories().iter().any(|cat| {
                    terminology_type.matches_category(cat.term())
                });

                // Apply additional title-based filtering if category matches
                if category_match {
                    terminology_type.matches_title(entry.title().value.as_str())
                } else {
                    false
                }
            })
            .map(FeedEntry::from_atom_entry)
            .collect();

        println!("Found {} entries for {}", entries.len(), terminology_type.display_name());
        Ok(entries)
    }

    /// Get only the latest entry from a feed
    pub async fn fetch_latest(
        &self,
        terminology_type: TerminologyType,
    ) -> Result<Option<FeedEntry>> {
        let entries = self.fetch_feed(terminology_type).await?;

        // The latest entry is typically the first one in the feed,
        // but we can also sort by updated date to be sure
        let latest = entries
            .into_iter()
            .max_by_key(|e| e.updated);

        Ok(latest)
    }

    /// Download terminology data from a URL
    pub async fn download_terminology(
        &self,
        url: &str,
        destination: &std::path::Path,
    ) -> Result<()> {
        println!("Downloading from: {}", url);

        // Get access token
        let token = self.token_manager.get_token().await
            .context("Failed to obtain access token")?;

        let response = self
            .client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .context("Failed to send download request")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download: HTTP {}", response.status());
        }

        let bytes = response.bytes().await
            .context("Failed to read download bytes")?;
        tokio::fs::write(destination, bytes).await
            .context("Failed to write file")?;

        println!("Downloaded to: {:?}", destination);
        Ok(())
    }

    /// Validate a downloaded file against its SHA-256 hash
    /// Returns Ok(()) if validation passes, Error if hash doesn't match or file can't be read
    pub async fn validate_file_hash(
        file_path: &std::path::Path,
        expected_hash: &str,
    ) -> Result<()> {
        println!("Validating SHA-256 hash for: {:?}", file_path);

        // Read the file
        let mut file = tokio::fs::File::open(file_path)
            .await
            .context("Failed to open file for hash validation")?;

        // Compute SHA-256 hash
        let mut hasher = Sha256::new();
        let mut buffer = Vec::new();

        // Read file into buffer
        use tokio::io::AsyncReadExt;
        file.read_to_end(&mut buffer)
            .await
            .context("Failed to read file for hashing")?;

        hasher.update(&buffer);
        let computed_hash = hasher.finalize();
        let computed_hash_hex = hex::encode(computed_hash);

        println!("Expected hash: {}", expected_hash);
        println!("Computed hash: {}", computed_hash_hex);

        // Compare hashes (case-insensitive)
        if computed_hash_hex.eq_ignore_ascii_case(expected_hash) {
            println!("✓ Hash validation passed");
            Ok(())
        } else {
            anyhow::bail!(
                "Hash validation failed!\nExpected: {}\nComputed: {}",
                expected_hash,
                computed_hash_hex
            )
        }
    }
}

// Note: Removed Default implementation as NctsClient now requires TokenManager

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syndication_url() {
        assert_eq!(
            SYNDICATION_FEED_URL,
            "https://api.healthterminologies.gov.au/syndication/v1/syndication.xml"
        );
    }

    #[test]
    fn test_category_matching() {
        // SNOMED - only SNAPSHOT
        assert!(TerminologyType::Snomed.matches_category("SCT_RF2_SNAPSHOT"));
        assert!(!TerminologyType::Snomed.matches_category("SCT_RF2_FULL"));
        assert!(!TerminologyType::Snomed.matches_category("SCT_RF2_DELTA"));

        // AMT - only CSV
        assert!(TerminologyType::Amt.matches_category("AMT_CSV"));
        assert!(!TerminologyType::Amt.matches_category("AMT_TSV"));

        // ValueSets - FHIR_Bundle
        assert!(TerminologyType::ValueSets.matches_category("FHIR_Bundle"));
        assert!(!TerminologyType::ValueSets.matches_category("FHIR_ValueSet"));

        // Cross-type matching
        assert!(!TerminologyType::Snomed.matches_category("AMT_CSV"));
        assert!(!TerminologyType::Amt.matches_category("SCT_RF2_SNAPSHOT"));
    }

    #[test]
    fn test_title_filtering() {
        // ValueSets requires R4 and excludes SNOMED reference sets
        assert!(TerminologyType::ValueSets.matches_title("NCTS FHIR Bundle (R4) 30 September 2025"));
        assert!(!TerminologyType::ValueSets.matches_title("NCTS FHIR Bundle (STU3) 30 September 2025"));
        assert!(!TerminologyType::ValueSets.matches_title("SNOMED CT-AU Reference Set Bundle (R4)"));

        // Other types don't filter by title
        assert!(TerminologyType::Snomed.matches_title("Any Title"));
        assert!(TerminologyType::Amt.matches_title("Any Title"));
    }
}
