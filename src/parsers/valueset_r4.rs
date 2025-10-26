use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
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
                // First pass: Build CodeSystem lookup for display names
                let codesystem_lookup = Self::build_codesystem_lookup(&bundle);

                // Second pass: Parse ValueSets with CodeSystem lookup
                if let Some(entries) = bundle.get("entry").and_then(|v| v.as_array()) {
                    for entry in entries {
                        if let Some(resource) = entry.get("resource") {
                            if let Ok(valueset) = Self::parse_valueset(resource, &codesystem_lookup) {
                                callback(valueset)?;
                                count += 1;
                            }
                        }
                    }
                }
            } else if resource_type == "ValueSet" {
                // Single ValueSet resource (no CodeSystems available)
                let empty_lookup = HashMap::new();
                if let Ok(valueset) = Self::parse_valueset(&bundle, &empty_lookup) {
                    callback(valueset)?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Build a lookup table: (CodeSystem URL, code) -> display
    fn build_codesystem_lookup(bundle: &Value) -> HashMap<(String, String), String> {
        let mut lookup = HashMap::new();

        if let Some(entries) = bundle.get("entry").and_then(|v| v.as_array()) {
            for entry in entries {
                if let Some(resource) = entry.get("resource") {
                    if let Some("CodeSystem") = resource.get("resourceType").and_then(|v| v.as_str()) {
                        if let Some(url) = resource.get("url").and_then(|v| v.as_str()) {
                            if let Some(concepts) = resource.get("concept").and_then(|v| v.as_array()) {
                                for concept in concepts {
                                    if let (Some(code), Some(display)) = (
                                        concept.get("code").and_then(|v| v.as_str()),
                                        concept.get("display").and_then(|v| v.as_str()),
                                    ) {
                                        lookup.insert(
                                            (url.to_string(), code.to_string()),
                                            display.to_string(),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        lookup
    }

    /// Parse a single ValueSet resource
    fn parse_valueset(
        resource: &Value,
        codesystem_lookup: &HashMap<(String, String), String>,
    ) -> Result<ValueSetEntry> {
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

        // Parse expansion if present (pre-expanded ValueSets)
        let expansion = if let Some(expansion_obj) = resource.get("expansion") {
            Self::parse_expansion(expansion_obj)
        } else if let Some(compose_obj) = resource.get("compose") {
            // Generate expansion from compose rules
            Self::parse_compose(compose_obj, codesystem_lookup)
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

    /// Parse the compose section to generate expansion
    /// This extracts explicitly listed concepts from compose.include[].concept[]
    /// and resolves display names from the CodeSystem lookup
    fn parse_compose(
        compose: &Value,
        codesystem_lookup: &HashMap<(String, String), String>,
    ) -> Option<Vec<ValueSetConcept>> {
        let includes = compose.get("include")?.as_array()?;

        let mut concepts = Vec::new();

        for include in includes {
            if let Some(system) = include.get("system").and_then(|v| v.as_str()) {
                // Check if there's an explicit concept list
                if let Some(concept_array) = include.get("concept").and_then(|v| v.as_array()) {
                    for concept_obj in concept_array {
                        if let Some(code) = concept_obj.get("code").and_then(|v| v.as_str()) {
                            // Try to get display from concept object first
                            let mut display = concept_obj
                                .get("display")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());

                            // If not present, look up from CodeSystem
                            if display.is_none() {
                                display = codesystem_lookup
                                    .get(&(system.to_string(), code.to_string()))
                                    .cloned();
                            }

                            concepts.push(ValueSetConcept {
                                system: system.to_string(),
                                code: code.to_string(),
                                display,
                            });
                        }
                    }
                }
                // Note: We don't handle filter-based includes here (e.g., "all codes from system X")
                // Those would require resolving against the CodeSystem, which we'll handle later if needed
            }
        }

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
        let empty_lookup = HashMap::new();
        let valueset = ValueSetR4Parser::parse_valueset(&resource, &empty_lookup).unwrap();

        assert_eq!(valueset.url, "http://example.org/ValueSet/test");
        assert_eq!(valueset.version, Some("1.0.0".to_string()));
        assert_eq!(valueset.name, Some("TestValueSet".to_string()));
        assert!(valueset.expansion.is_some());

        let expansion = valueset.expansion.unwrap();
        assert_eq!(expansion.len(), 1);
        assert_eq!(expansion[0].code, "12345");
    }
}
