use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// AMT (Australian Medicines Terminology) Code entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmtCode {
    pub id: String,
    pub preferred_term: String,
    pub code_type: String,
    pub parent_code: Option<String>,
    pub properties: Option<String>,
}

pub struct AmtCsvParser;

impl AmtCsvParser {
    /// Parse AMT codes from a CSV file
    /// CSV format is expected to have headers with columns like:
    /// ID, Preferred Term, Type, Parent, etc.
    pub fn parse<P: AsRef<Path>, F>(path: P, mut callback: F) -> Result<usize>
    where
        F: FnMut(AmtCode) -> Result<()>,
    {
        let file = std::fs::File::open(path.as_ref())
            .context("Failed to open AMT CSV file")?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Read and parse header line to determine column positions
        let header = lines
            .next()
            .context("No header line in AMT CSV file")?
            .context("Failed to read header line")?;

        let headers: Vec<&str> = header.split(',').map(|s| s.trim()).collect();

        // Find column indices (case-insensitive)
        let id_idx = headers
            .iter()
            .position(|&h| h.eq_ignore_ascii_case("id") || h.eq_ignore_ascii_case("code"))
            .context("Could not find ID/Code column")?;

        let term_idx = headers
            .iter()
            .position(|&h| {
                h.eq_ignore_ascii_case("preferred term")
                    || h.eq_ignore_ascii_case("term")
                    || h.eq_ignore_ascii_case("display")
            })
            .context("Could not find Preferred Term/Term/Display column")?;

        let type_idx = headers
            .iter()
            .position(|&h| h.eq_ignore_ascii_case("type") || h.eq_ignore_ascii_case("kind"));

        let parent_idx = headers
            .iter()
            .position(|&h| h.eq_ignore_ascii_case("parent"));

        let mut count = 0;
        for line in lines {
            let line = line.context("Failed to read line")?;
            if line.trim().is_empty() {
                continue;
            }

            // Handle CSV parsing with possible quoted fields
            let fields = Self::parse_csv_line(&line);

            if fields.len() <= id_idx || fields.len() <= term_idx {
                continue; // Skip malformed lines
            }

            let code_type = type_idx
                .and_then(|idx| fields.get(idx))
                .map(|s| s.to_string())
                .unwrap_or_else(|| "Unknown".to_string());

            let parent_code = parent_idx
                .and_then(|idx| fields.get(idx))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            let amt_code = AmtCode {
                id: fields[id_idx].to_string(),
                preferred_term: fields[term_idx].to_string(),
                code_type,
                parent_code,
                properties: None, // Can be extended later
            };

            callback(amt_code)?;
            count += 1;
        }

        Ok(count)
    }

    /// Simple CSV line parser that handles quoted fields
    fn parse_csv_line(line: &str) -> Vec<String> {
        let mut fields = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    in_quotes = !in_quotes;
                }
                ',' if !in_quotes => {
                    fields.push(current.trim().to_string());
                    current.clear();
                }
                _ => {
                    current.push(c);
                }
            }
        }

        // Push the last field
        fields.push(current.trim().to_string());
        fields
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_csv_line() {
        let line = r#"123,"Paracetamol 500 mg tablet",MP,"#;
        let fields = AmtCsvParser::parse_csv_line(line);
        assert_eq!(fields[0], "123");
        assert_eq!(fields[1], "Paracetamol 500 mg tablet");
        assert_eq!(fields[2], "MP");
    }

    #[test]
    fn test_parse_csv_line_with_commas() {
        let line = r#"456,"Product, complex name",TP,123"#;
        let fields = AmtCsvParser::parse_csv_line(line);
        assert_eq!(fields[0], "456");
        assert_eq!(fields[1], "Product, complex name");
        assert_eq!(fields[2], "TP");
        assert_eq!(fields[3], "123");
    }
}
