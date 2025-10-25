use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

/// FHIR R4 ValueSet entry (from Bundle)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetEntry {
    pub url: String,
    pub version: Option<String>,
    pub name: Option<String>,
    pub title: Option<String>,
    pub status: Option<String>,
    pub description: Option<String>,
    pub publisher: Option<String>,
    pub expansion: Option<Vec<ValueSetConcept>>,
}

/// Concept from ValueSet expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetConcept {
    pub system: String,
    pub code: String,
    pub display: Option<String>,
}

pub struct ValueSetR4Parser;

impl ValueSetR4Parser {
    /// Parse FHIR R4 ValueSet Bundle from JSON file
    /// The file should be a Bundle resource containing ValueSet entries
    pub fn parse_bundle<P: AsRef<Path>, F>(path: P, mut callback: F) -> Result<usize>
    where
        F: FnMut(ValueSetEntry) -> Result<()>,
    {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read ValueSet bundle file")?;

        let bundle: Value = serde_json::from_str(&content)
            .context("Failed to parse JSON")?;

        let mut count = 0;

        // Check if this is a Bundle resource
        if let Some(resource_type) = bundle.get("resourceType").and_then(|v| v.as_str()) {
            if resource_type == "Bundle" {
                // Parse bundle entries
                if let Some(entries) = bundle.get("entry").and_then(|v| v.as_array()) {
                    for entry in entries {
                        if let Some(resource) = entry.get("resource") {
                            if let Ok(valueset) = Self::parse_valueset(resource) {
                                callback(valueset)?;
                                count += 1;
                            }
                        }
                    }
                }
            } else if resource_type == "ValueSet" {
                // Single ValueSet resource
                if let Ok(valueset) = Self::parse_valueset(&bundle) {
                    callback(valueset)?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Parse a single ValueSet resource
    fn parse_valueset(resource: &Value) -> Result<ValueSetEntry> {
        let url = resource
            .get("url")
            .and_then(|v| v.as_str())
            .context("ValueSet missing required 'url' field")?
            .to_string();

        let version = resource
            .get("version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let name = resource
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let title = resource
            .get("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let status = resource
            .get("status")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let description = resource
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let publisher = resource
            .get("publisher")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Parse expansion if present
        let expansion = if let Some(expansion_obj) = resource.get("expansion") {
            Self::parse_expansion(expansion_obj)
        } else {
            None
        };

        Ok(ValueSetEntry {
            url,
            version,
            name,
            title,
            status,
            description,
            publisher,
            expansion,
        })
    }

    /// Parse the expansion section of a ValueSet
    fn parse_expansion(expansion: &Value) -> Option<Vec<ValueSetConcept>> {
        let contains = expansion.get("contains")?.as_array()?;

        let concepts: Vec<ValueSetConcept> = contains
            .iter()
            .filter_map(|item| {
                let system = item.get("system")?.as_str()?.to_string();
                let code = item.get("code")?.as_str()?.to_string();
                let display = item
                    .get("display")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                Some(ValueSetConcept {
                    system,
                    code,
                    display,
                })
            })
            .collect();

        if concepts.is_empty() {
            None
        } else {
            Some(concepts)
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valueset_basic() {
        let json = r#"
        {
            "resourceType": "ValueSet",
            "url": "http://example.org/ValueSet/test",
            "version": "1.0.0",
            "name": "TestValueSet",
            "title": "Test Value Set",
            "status": "active",
            "expansion": {
                "contains": [
                    {
                        "system": "http://snomed.info/sct",
                        "code": "12345",
                        "display": "Test Concept"
                    }
                ]
            }
        }
        "#;

        let resource: Value = serde_json::from_str(json).unwrap();
        let valueset = ValueSetR4Parser::parse_valueset(&resource).unwrap();

        assert_eq!(valueset.url, "http://example.org/ValueSet/test");
        assert_eq!(valueset.version, Some("1.0.0".to_string()));
        assert_eq!(valueset.name, Some("TestValueSet".to_string()));
        assert!(valueset.expansion.is_some());

        let expansion = valueset.expansion.unwrap();
        assert_eq!(expansion.len(), 1);
        assert_eq!(expansion[0].code, "12345");
    }
}
