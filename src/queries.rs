use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Code lookup result with synonyms
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeLookupResult {
    pub code: String,
    pub system: String,
    pub display: String,
    pub active: bool,
    pub synonyms: Vec<String>,
}

/// Search result across terminologies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub code: String,
    pub system: String,
    pub display: String,
    pub terminology_type: String,
    pub active: bool,
    pub subtype: Option<String>,
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

pub struct TerminologyQueries;

impl TerminologyQueries {
    /// Look up a SNOMED concept by code with all its synonyms
    pub async fn lookup_snomed_code(
        pool: &SqlitePool,
        code: &str,
    ) -> Result<Option<CodeLookupResult>> {
        // Get the concept
        let concept: Option<(i32,)> = sqlx::query_as(
            r#"
            SELECT active
            FROM snomed_concepts
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;

        if let Some((active,)) = concept {
            // Get the fully specified name (FSN) as display
            let fsn: Option<(String,)> = sqlx::query_as(
                r#"
                SELECT term
                FROM snomed_descriptions
                WHERE concept_id = ? AND type_id = '900000000000003001' AND active = 1
                ORDER BY effective_time DESC
                LIMIT 1
                "#,
            )
            .bind(code)
            .fetch_optional(pool)
            .await?;

            let display = fsn
                .map(|(term,)| term)
                .or_else(|| Some("Unknown".to_string()))
                .unwrap();

            // Get all synonyms (type_id = '900000000000013009' for synonyms)
            let synonyms: Vec<(String,)> = sqlx::query_as(
                r#"
                SELECT DISTINCT term
                FROM snomed_descriptions
                WHERE concept_id = ? AND active = 1
                ORDER BY term
                "#,
            )
            .bind(code)
            .fetch_all(pool)
            .await?;

            let synonym_list = synonyms.into_iter().map(|(term,)| term).collect();

            Ok(Some(CodeLookupResult {
                code: code.to_string(),
                system: "http://snomed.info/sct".to_string(),
                display,
                active: active == 1,
                synonyms: synonym_list,
            }))
        } else {
            Ok(None)
        }
    }

    /// Look up an AMT code
    pub async fn lookup_amt_code(
        pool: &SqlitePool,
        code: &str,
    ) -> Result<Option<CodeLookupResult>> {
        let result: Option<(String, String)> = sqlx::query_as(
            r#"
            SELECT id, preferred_term
            FROM amt_codes
            WHERE id = ?
            LIMIT 1
            "#,
        )
        .bind(code)
        .fetch_optional(pool)
        .await?;

        if let Some((id, term)) = result {
            Ok(Some(CodeLookupResult {
                code: id,
                system: "http://hl7.org/fhir/sid/ncts-amt".to_string(),
                display: term,
                active: true,
                synonyms: vec![],
            }))
        } else {
            Ok(None)
        }
    }

    /// Full-text search across SNOMED descriptions
    pub async fn search_snomed(
        pool: &SqlitePool,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SearchResult>> {
        let search_pattern = format!("%{}%", query);

        let results: Vec<(String, String, i32)> = sqlx::query_as(
            r#"
            SELECT DISTINCT d.concept_id, d.term, c.active
            FROM snomed_descriptions d
            JOIN snomed_concepts c ON d.concept_id = c.id
            WHERE d.term LIKE ? AND d.active = 1
            ORDER BY
                CASE WHEN d.term LIKE ? THEN 0 ELSE 1 END,
                d.term
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(format!("{}%", query)) // Prefix match gets priority
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(code, display, active)| SearchResult {
                code,
                system: "http://snomed.info/sct".to_string(),
                display,
                terminology_type: "snomed".to_string(),
                active: active == 1,
                subtype: None,
            })
            .collect())
    }

    /// Full-text search across AMT codes
    pub async fn search_amt(
        pool: &SqlitePool,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SearchResult>> {
        let search_pattern = format!("%{}%", query);

        let results: Vec<(String, String, String)> = sqlx::query_as(
            r#"
            SELECT id, preferred_term, code_type
            FROM amt_codes
            WHERE preferred_term LIKE ?
            ORDER BY
                CASE WHEN preferred_term LIKE ? THEN 0 ELSE 1 END,
                preferred_term
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(format!("{}%", query)) // Prefix match gets priority
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(code, display, code_type)| SearchResult {
                code,
                system: "http://hl7.org/fhir/sid/ncts-amt".to_string(),
                display,
                terminology_type: "amt".to_string(),
                active: true,
                subtype: Some(code_type),
            })
            .collect())
    }

    /// Full-text search across ValueSets
    pub async fn search_valuesets(
        pool: &SqlitePool,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SearchResult>> {
        let search_pattern = format!("%{}%", query);

        let results: Vec<(String, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT url, title, name
            FROM valuesets
            WHERE url LIKE ? OR title LIKE ? OR name LIKE ? OR description LIKE ?
            ORDER BY
                CASE
                    WHEN url LIKE ? THEN 0
                    WHEN title LIKE ? THEN 1
                    WHEN name LIKE ? THEN 2
                    ELSE 3
                END,
                title
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(format!("{}%", query)) // URL prefix match gets priority
        .bind(format!("{}%", query)) // Title prefix match
        .bind(format!("{}%", query)) // Name prefix match
        .bind(limit)
        .fetch_all(pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(url, title, name)| {
                let display = title
                    .or(name)
                    .unwrap_or_else(|| url.split('/').last().unwrap_or("Unknown").to_string());

                SearchResult {
                    code: url.clone(),
                    system: "http://hl7.org/fhir/ValueSet".to_string(),
                    display,
                    terminology_type: "valuesets".to_string(),
                    active: true,
                    subtype: None,
                }
            })
            .collect())
    }

    /// Search across all terminologies
    pub async fn search_all(
        pool: &SqlitePool,
        query: &str,
        limit: i32,
    ) -> Result<Vec<SearchResult>> {
        let mut results = Vec::new();

        // Search SNOMED (third of the limit)
        let snomed_results = Self::search_snomed(pool, query, limit / 3).await?;
        results.extend(snomed_results);

        // Search AMT (third of the limit)
        let amt_results = Self::search_amt(pool, query, limit / 3).await?;
        results.extend(amt_results);

        // Search ValueSets (third of the limit)
        let valueset_results = Self::search_valuesets(pool, query, limit / 3).await?;
        results.extend(valueset_results);

        Ok(results)
    }

    /// Expand a ValueSet by URL
    pub async fn expand_valueset(
        pool: &SqlitePool,
        valueset_url: &str,
    ) -> Result<Option<ValueSetExpansion>> {
        // Get ValueSet metadata
        let valueset: Option<(i64, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT id, version, title
            FROM valuesets
            WHERE url = ?
            LIMIT 1
            "#,
        )
        .bind(valueset_url)
        .fetch_optional(pool)
        .await?;

        if let Some((valueset_id, version, title)) = valueset {
            // Get expansion concepts
            let concepts: Vec<(String, String, Option<String>)> = sqlx::query_as(
                r#"
                SELECT system, code, display
                FROM valueset_concepts
                WHERE valueset_id = ?
                ORDER BY display
                "#,
            )
            .bind(valueset_id)
            .fetch_all(pool)
            .await?;

            let total = concepts.len();
            let concept_list = concepts
                .into_iter()
                .map(|(system, code, display)| ValueSetConceptResult {
                    system,
                    code,
                    display,
                })
                .collect();

            Ok(Some(ValueSetExpansion {
                url: valueset_url.to_string(),
                version,
                title,
                total,
                concepts: concept_list,
            }))
        } else {
            Ok(None)
        }
    }

    /// Validate a code against a ValueSet
    pub async fn validate_code(
        pool: &SqlitePool,
        code: &str,
        system: &str,
        valueset_url: &str,
    ) -> Result<ValidationResult> {
        // Get ValueSet ID
        let valueset: Option<(i64,)> = sqlx::query_as(
            r#"
            SELECT id
            FROM valuesets
            WHERE url = ?
            LIMIT 1
            "#,
        )
        .bind(valueset_url)
        .fetch_optional(pool)
        .await?;

        if let Some((valueset_id,)) = valueset {
            // Check if code exists in ValueSet expansion
            let exists: (i32,) = sqlx::query_as(
                r#"
                SELECT COUNT(*)
                FROM valueset_concepts
                WHERE valueset_id = ? AND system = ? AND code = ?
                "#,
            )
            .bind(valueset_id)
            .bind(system)
            .bind(code)
            .fetch_one(pool)
            .await?;

            let count = exists.0;

            Ok(ValidationResult {
                valid: count > 0,
                message: if count > 0 {
                    Some("Code is in ValueSet".to_string())
                } else {
                    Some("Code not found in ValueSet".to_string())
                },
            })
        } else {
            Ok(ValidationResult {
                valid: false,
                message: Some(format!("ValueSet '{}' not found", valueset_url)),
            })
        }
    }

    /// List all available ValueSets
    pub async fn list_valuesets(pool: &SqlitePool) -> Result<Vec<(String, Option<String>)>> {
        let results: Vec<(String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT url, title
            FROM valuesets
            ORDER BY title, url
            "#,
        )
        .fetch_all(pool)
        .await?;

        Ok(results)
    }
}
