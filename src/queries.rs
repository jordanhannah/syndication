use crate::search::{SearchResult, TerminologySearch};
use crate::storage::TerminologyStorage;
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Code lookup result with synonyms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLookupResult {
    pub code: String,
    pub system: String,
    pub display: String,
    pub active: bool,
    pub synonyms: Vec<String>,
}

/// ValueSet expansion result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetExpansion {
    pub url: String,
    pub version: Option<String>,
    pub title: Option<String>,
    pub total: usize,
    pub concepts: Vec<ValueSetConceptResult>,
}

/// Concept in ValueSet expansion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetConceptResult {
    pub system: String,
    pub code: String,
    pub display: Option<String>,
}

/// Code validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: Option<String>,
}

/// Simplified list result for browse operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValueSetListItem {
    pub url: String,
    pub title: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
}

pub struct TerminologyQueries;

impl TerminologyQueries {
    /// Look up a SNOMED concept by code with all its synonyms
    pub fn lookup_snomed_code(
        storage: &TerminologyStorage,
        code: &str,
    ) -> Result<Option<CodeLookupResult>> {
        // Get the concept
        let concept = storage.get_snomed_concept(code)?;

        if let Some(concept) = concept {
            // Get all descriptions for synonyms
            let descriptions = storage.get_snomed_descriptions(code)?;

            // Find FSN as display (type_id = '900000000000003001')
            let fsn = descriptions
                .iter()
                .find(|d| d.type_id == "900000000000003001" && d.active)
                .map(|d| d.term.clone());

            let display = fsn.unwrap_or_else(|| "Unknown".to_string());

            // Collect all active terms as synonyms
            let synonyms: Vec<String> = descriptions
                .iter()
                .filter(|d| d.active)
                .map(|d| d.term.clone())
                .collect();

            Ok(Some(CodeLookupResult {
                code: code.to_string(),
                system: "http://snomed.info/sct".to_string(),
                display,
                active: concept.active,
                synonyms,
            }))
        } else {
            Ok(None)
        }
    }

    /// Look up an AMT code by ID
    pub fn lookup_amt_code(
        storage: &TerminologyStorage,
        code: &str,
    ) -> Result<Option<CodeLookupResult>> {
        let amt_code = storage.get_amt_code(code)?;

        if let Some(amt_code) = amt_code {
            Ok(Some(CodeLookupResult {
                code: amt_code.id.clone(),
                system: "http://hl7.org/fhir/sid/ncts-amt".to_string(),
                display: amt_code.preferred_term.clone(),
                active: true,
                synonyms: vec![amt_code.preferred_term],
            }))
        } else {
            Ok(None)
        }
    }

    /// Search SNOMED descriptions using Tantivy
    pub fn search_snomed(
        searcher: &TerminologySearch,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        searcher.search_snomed(query, limit)
    }

    /// Search AMT codes using Tantivy with optional code type filtering
    pub fn search_amt(
        searcher: &TerminologySearch,
        query: &str,
        limit: usize,
        code_types: Option<&[String]>,
    ) -> Result<Vec<SearchResult>> {
        searcher.search_amt(query, limit, code_types)
    }

    /// Search ValueSets using Tantivy
    pub fn search_valuesets(
        searcher: &TerminologySearch,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        searcher.search_valuesets(query, limit)
    }

    /// Search across all terminologies
    pub fn search_all(
        searcher: &TerminologySearch,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        searcher.search_all(query, limit)
    }

    /// Expand a ValueSet to get all its concepts
    pub fn expand_valueset(
        storage: &TerminologyStorage,
        valueset_url: &str,
    ) -> Result<Option<ValueSetExpansion>> {
        // Get ValueSet metadata
        let valueset = storage.get_valueset(valueset_url)?;

        if let Some(valueset) = valueset {
            // Get all concepts in this ValueSet
            let concepts = storage.get_valueset_concepts(valueset_url)?;

            let concept_results: Vec<ValueSetConceptResult> = concepts
                .into_iter()
                .map(|c| ValueSetConceptResult {
                    system: c.system,
                    code: c.code,
                    display: c.display,
                })
                .collect();

            Ok(Some(ValueSetExpansion {
                url: valueset.url,
                version: valueset.version,
                title: valueset.title,
                total: concept_results.len(),
                concepts: concept_results,
            }))
        } else {
            Ok(None)
        }
    }

    /// Validate that a code exists in a ValueSet
    pub fn validate_code(
        storage: &TerminologyStorage,
        code: &str,
        system: &str,
        valueset_url: &str,
    ) -> Result<ValidationResult> {
        let is_valid = storage.valueset_contains_code(valueset_url, system, code)?;

        if is_valid {
            Ok(ValidationResult {
                valid: true,
                message: Some(format!("Code {} is valid in ValueSet {}", code, valueset_url)),
            })
        } else {
            Ok(ValidationResult {
                valid: false,
                message: Some(format!("Code {} not found in ValueSet {}", code, valueset_url)),
            })
        }
    }

    /// List all available ValueSets
    pub fn list_valuesets(storage: &TerminologyStorage) -> Result<Vec<ValueSetListItem>> {
        let valuesets = storage.get_all_valuesets()?;

        let items: Vec<ValueSetListItem> = valuesets
            .into_iter()
            .map(|vs| ValueSetListItem {
                url: vs.url,
                title: vs.title,
                name: vs.name,
                description: vs.description,
            })
            .collect();

        Ok(items)
    }
}
