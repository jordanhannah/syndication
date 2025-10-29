use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
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

/// Column pair for SCTID and PT
#[derive(Debug)]
struct ColumnPair {
    product_type: String,
    sctid_idx: usize,
    pt_idx: usize,
}

pub struct AmtCsvParser;

impl AmtCsvParser {
    /// Count total AMT codes in a CSV file without importing
    /// Used for accurate progress tracking during import
    pub fn count_codes<P: AsRef<Path>>(path: P) -> Result<usize> {
        let mut count = 0;
        Self::parse(path, |_| {
            count += 1;
            Ok(())
        })?;
        Ok(count)
    }

    /// Parse AMT codes from a CSV file
    /// CSV format is wide-format with multiple product types:
    /// CTPP SCTID, CTPP PT, ARTG_ID, TPP SCTID, TPP PT, TPUU SCTID, TPUU PT,
    /// TPP TP SCTID, TPP TP PT, TPUU TP SCTID, TPUU TP PT, MPP SCTID, MPP PT,
    /// MPUU SCTID, MPUU PT, MP SCTID, MP PT
    /// Each row is expanded into multiple AmtCode entries (one per product type)
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

        // Find ARTG_ID column
        let artg_idx = headers
            .iter()
            .position(|&h| h.eq_ignore_ascii_case("ARTG_ID"));

        // Find all SCTID/PT column pairs
        let column_pairs = Self::find_column_pairs(&headers)?;

        if column_pairs.is_empty() {
            anyhow::bail!("No SCTID/PT column pairs found in AMT CSV");
        }

        // Define product type hierarchy (child -> parent)
        // Hierarchy flows right to left in CSV: MP (root) <- MPUU <- MPP <- TPUU TP <- TPP TP <- TPUU <- TPP <- CTPP (leaf)
        // Each product type's parent is the next more generic level
        let hierarchy = vec![
            ("CTPP", "TPP"),           // Containered Trade Product Pack -> Trade Product Pack
            ("TPP", "TPUU"),           // Trade Product Pack -> Trade Product Unit of Use
            ("TPUU", "TPP TP"),        // Trade Product Unit of Use -> Trade Product (TP)
            ("TPP TP", "TPUU TP"),     // Trade Product (TP) -> Trade Product Unit of Use (TP)
            ("TPUU TP", "MPP"),        // Trade Product Unit of Use (TP) -> Medicinal Product Pack
            ("MPP", "MPUU"),           // Medicinal Product Pack -> Medicinal Product Unit of Use
            ("MPUU", "MP"),            // Medicinal Product Unit of Use -> Medicinal Product (root)
        ];

        let mut count = 0;
        for line in lines {
            let line = line.context("Failed to read line")?;
            if line.trim().is_empty() {
                continue;
            }

            // Handle CSV parsing with possible quoted fields
            let fields = Self::parse_csv_line(&line);

            // Extract ARTG_ID if present
            let artg_id = artg_idx
                .and_then(|idx| fields.get(idx))
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string());

            // Build a map of product_type -> code for parent lookups
            let mut product_codes: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            // First pass: collect all codes
            for pair in &column_pairs {
                if let Some(code) = fields.get(pair.sctid_idx) {
                    if !code.is_empty() {
                        product_codes.insert(pair.product_type.clone(), code.to_string());
                    }
                }
            }

            // Second pass: create AmtCode entries with parent relationships
            for pair in &column_pairs {
                let code = fields.get(pair.sctid_idx)
                    .filter(|s| !s.is_empty());
                let term = fields.get(pair.pt_idx)
                    .filter(|s| !s.is_empty());

                if let (Some(code), Some(term)) = (code, term) {
                    // Find parent code based on hierarchy
                    let parent_code = hierarchy
                        .iter()
                        .find(|(child, _)| child == &pair.product_type.as_str())
                        .and_then(|(_, parent)| product_codes.get(*parent))
                        .cloned();

                    // Build properties JSON
                    let properties = if artg_id.is_some() {
                        Some(json!({
                            "artg_id": artg_id
                        }).to_string())
                    } else {
                        None
                    };

                    let amt_code = AmtCode {
                        id: code.to_string(),
                        preferred_term: term.to_string(),
                        code_type: pair.product_type.clone(),
                        parent_code,
                        properties,
                    };

                    callback(amt_code)?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Find all SCTID/PT column pairs in headers
    fn find_column_pairs(headers: &[&str]) -> Result<Vec<ColumnPair>> {
        let mut pairs = Vec::new();
        let mut sctid_columns: Vec<(String, usize)> = Vec::new();

        // Find all SCTID columns
        for (idx, header) in headers.iter().enumerate() {
            if header.ends_with("SCTID") {
                // Extract product type (e.g., "CTPP SCTID" -> "CTPP")
                let product_type = header
                    .strip_suffix("SCTID")
                    .unwrap_or(header)
                    .trim()
                    .to_string();
                sctid_columns.push((product_type, idx));
            }
        }

        // For each SCTID column, find corresponding PT column
        for (product_type, sctid_idx) in sctid_columns {
            let pt_header = format!("{} PT", product_type);
            if let Some(pt_idx) = headers.iter().position(|&h| h == pt_header) {
                pairs.push(ColumnPair {
                    product_type,
                    sctid_idx,
                    pt_idx,
                });
            }
        }

        Ok(pairs)
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

    #[test]
    fn test_find_column_pairs() {
        let headers = vec![
            "CTPP SCTID",
            "CTPP PT",
            "ARTG_ID",
            "TPP SCTID",
            "TPP PT",
            "MP SCTID",
            "MP PT",
        ];

        let pairs = AmtCsvParser::find_column_pairs(&headers).unwrap();

        assert_eq!(pairs.len(), 3);
        assert_eq!(pairs[0].product_type, "CTPP");
        assert_eq!(pairs[0].sctid_idx, 0);
        assert_eq!(pairs[0].pt_idx, 1);

        assert_eq!(pairs[1].product_type, "TPP");
        assert_eq!(pairs[1].sctid_idx, 3);
        assert_eq!(pairs[1].pt_idx, 4);

        assert_eq!(pairs[2].product_type, "MP");
        assert_eq!(pairs[2].sctid_idx, 5);
        assert_eq!(pairs[2].pt_idx, 6);
    }

    #[test]
    fn test_find_column_pairs_with_spaces() {
        // Test the real AMT CSV header with all 17 columns including "TPP TP" and "TPUU TP"
        let headers = vec![
            "CTPP SCTID", "CTPP PT", "ARTG_ID",
            "TPP SCTID", "TPP PT",
            "TPUU SCTID", "TPUU PT",
            "TPP TP SCTID", "TPP TP PT",
            "TPUU TP SCTID", "TPUU TP PT",
            "MPP SCTID", "MPP PT",
            "MPUU SCTID", "MPUU PT",
            "MP SCTID", "MP PT",
        ];

        let pairs = AmtCsvParser::find_column_pairs(&headers).unwrap();

        // Should find all 8 product types
        assert_eq!(pairs.len(), 8, "Should find 8 SCTID/PT pairs");

        // Verify TPP TP (with space) is detected
        let tpp_tp = pairs.iter().find(|p| p.product_type == "TPP TP");
        assert!(tpp_tp.is_some(), "TPP TP should be detected");
        assert_eq!(tpp_tp.unwrap().sctid_idx, 7);
        assert_eq!(tpp_tp.unwrap().pt_idx, 8);

        // Verify TPUU TP (with space) is detected
        let tpuu_tp = pairs.iter().find(|p| p.product_type == "TPUU TP");
        assert!(tpuu_tp.is_some(), "TPUU TP should be detected");
        assert_eq!(tpuu_tp.unwrap().sctid_idx, 9);
        assert_eq!(tpuu_tp.unwrap().pt_idx, 10);

        // Verify MP is still detected
        let mp = pairs.iter().find(|p| p.product_type == "MP");
        assert!(mp.is_some(), "MP should be detected");
        assert_eq!(mp.unwrap().sctid_idx, 15);
        assert_eq!(mp.unwrap().pt_idx, 16);
    }

    #[test]
    fn test_parse_amt_real_format() {
        // Create a temporary test file with real AMT format
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::new().unwrap();

        // Write header with all 17 columns
        writeln!(
            temp_file,
            "CTPP SCTID,CTPP PT,ARTG_ID,TPP SCTID,TPP PT,TPUU SCTID,TPUU PT,TPP TP SCTID,TPP TP PT,TPUU TP SCTID,TPUU TP PT,MPP SCTID,MPP PT,MPUU SCTID,MPUU PT,MP SCTID,MP PT"
        )
        .unwrap();

        // Write test data row with all product types (from user's sample data)
        writeln!(
            temp_file,
            r#"1664881000168108,"Olsetan 40 mg tablet, 30, blister pack",358594,1664871000168105,"Olsetan 40 mg tablet, 30",1664861000168104,"Olsetan 40 mg tablet",1664821000168109,"Olsetan",1664821000168109,"Olsetan",26567011000036100,"Olmesartan medoxomil 40 mg tablet, 30",21976011000036100,"Olmesartan medoxomil 40 mg tablet",385540001,"Olmesartan""#
        )
        .unwrap();

        temp_file.flush().unwrap();

        // Parse the file
        let mut codes = Vec::new();
        let count = AmtCsvParser::parse(temp_file.path(), |code| {
            codes.push(code);
            Ok(())
        })
        .unwrap();

        // Should have 8 codes (CTPP, TPP, TPUU, TPP TP, TPUU TP, MPP, MPUU, MP)
        assert_eq!(count, 8);
        assert_eq!(codes.len(), 8);

        // Check CTPP (Containered Trade Product Pack)
        let ctpp = codes.iter().find(|c| c.code_type == "CTPP").unwrap();
        assert_eq!(ctpp.id, "1664881000168108");
        assert_eq!(ctpp.preferred_term, "Olsetan 40 mg tablet, 30, blister pack");
        assert_eq!(ctpp.parent_code, Some("1664871000168105".to_string())); // TPP
        assert!(ctpp.properties.is_some());
        assert!(ctpp.properties.as_ref().unwrap().contains("358594"));

        // Check TPP (Trade Product Pack)
        let tpp = codes.iter().find(|c| c.code_type == "TPP").unwrap();
        assert_eq!(tpp.id, "1664871000168105");
        assert_eq!(tpp.preferred_term, "Olsetan 40 mg tablet, 30");
        assert_eq!(tpp.parent_code, Some("1664861000168104".to_string())); // TPUU

        // Check TPUU (Trade Product Unit of Use)
        let tpuu = codes.iter().find(|c| c.code_type == "TPUU").unwrap();
        assert_eq!(tpuu.id, "1664861000168104");
        assert_eq!(tpuu.preferred_term, "Olsetan 40 mg tablet");
        assert_eq!(tpuu.parent_code, Some("1664821000168109".to_string())); // TPP TP

        // Check TPP TP (Trade Product - TP)
        let tpp_tp = codes.iter().find(|c| c.code_type == "TPP TP").unwrap();
        assert_eq!(tpp_tp.id, "1664821000168109");
        assert_eq!(tpp_tp.preferred_term, "Olsetan");
        assert_eq!(tpp_tp.parent_code, Some("1664821000168109".to_string())); // TPUU TP

        // Check TPUU TP (Trade Product Unit of Use - TP)
        let tpuu_tp = codes.iter().find(|c| c.code_type == "TPUU TP").unwrap();
        assert_eq!(tpuu_tp.id, "1664821000168109");
        assert_eq!(tpuu_tp.preferred_term, "Olsetan");
        assert_eq!(tpuu_tp.parent_code, Some("26567011000036100".to_string())); // MPP

        // Check MPP (Medicinal Product Pack)
        let mpp = codes.iter().find(|c| c.code_type == "MPP").unwrap();
        assert_eq!(mpp.id, "26567011000036100");
        assert_eq!(mpp.preferred_term, "Olmesartan medoxomil 40 mg tablet, 30");
        assert_eq!(mpp.parent_code, Some("21976011000036100".to_string())); // MPUU

        // Check MPUU (Medicinal Product Unit of Use)
        let mpuu = codes.iter().find(|c| c.code_type == "MPUU").unwrap();
        assert_eq!(mpuu.id, "21976011000036100");
        assert_eq!(mpuu.preferred_term, "Olmesartan medoxomil 40 mg tablet");
        assert_eq!(mpuu.parent_code, Some("385540001".to_string())); // MP

        // Check MP (Medicinal Product - root)
        let mp = codes.iter().find(|c| c.code_type == "MP").unwrap();
        assert_eq!(mp.id, "385540001");
        assert_eq!(mp.preferred_term, "Olmesartan");
        assert_eq!(mp.parent_code, None); // MP is top-level (root parent)
    }
}
