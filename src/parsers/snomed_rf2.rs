use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::Path;

/// SNOMED CT Concept (from Concept_Snapshot file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnomedConcept {
    pub id: String,
    pub effective_time: String,
    pub active: bool,
    pub module_id: String,
    pub definition_status_id: String,
}

/// SNOMED CT Description (from Description_Snapshot file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnomedDescription {
    pub id: String,
    pub effective_time: String,
    pub active: bool,
    pub module_id: String,
    pub concept_id: String,
    pub language_code: String,
    pub type_id: String,
    pub term: String,
    pub case_significance_id: String,
}

/// SNOMED CT Relationship (from Relationship_Snapshot file)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnomedRelationship {
    pub id: String,
    pub effective_time: String,
    pub active: bool,
    pub module_id: String,
    pub source_id: String,
    pub destination_id: String,
    pub relationship_group: i32,
    pub type_id: String,
    pub characteristic_type_id: String,
    pub modifier_id: String,
}

pub struct SnomedRf2Parser;

impl SnomedRf2Parser {
    /// Parse SNOMED CT Concepts from a TSV file
    pub fn parse_concepts<P: AsRef<Path>, F>(path: P, mut callback: F) -> Result<usize>
    where
        F: FnMut(SnomedConcept) -> Result<()>,
    {
        let file = std::fs::File::open(path.as_ref())
            .context("Failed to open concepts file")?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Skip header line
        lines.next().context("No header line in concepts file")?
            .context("Failed to read header line")?;

        let mut count = 0;
        for line in lines {
            let line = line.context("Failed to read line")?;
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 5 {
                continue; // Skip malformed lines
            }

            let concept = SnomedConcept {
                id: fields[0].to_string(),
                effective_time: fields[1].to_string(),
                active: fields[2] == "1",
                module_id: fields[3].to_string(),
                definition_status_id: fields[4].to_string(),
            };

            callback(concept)?;
            count += 1;
        }

        Ok(count)
    }

    /// Parse SNOMED CT Descriptions from a TSV file
    pub fn parse_descriptions<P: AsRef<Path>, F>(path: P, mut callback: F) -> Result<usize>
    where
        F: FnMut(SnomedDescription) -> Result<()>,
    {
        let file = std::fs::File::open(path.as_ref())
            .context("Failed to open descriptions file")?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Skip header line
        lines.next().context("No header line in descriptions file")?
            .context("Failed to read header line")?;

        let mut count = 0;
        for line in lines {
            let line = line.context("Failed to read line")?;
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 9 {
                continue; // Skip malformed lines
            }

            let description = SnomedDescription {
                id: fields[0].to_string(),
                effective_time: fields[1].to_string(),
                active: fields[2] == "1",
                module_id: fields[3].to_string(),
                concept_id: fields[4].to_string(),
                language_code: fields[5].to_string(),
                type_id: fields[6].to_string(),
                term: fields[7].to_string(),
                case_significance_id: fields[8].to_string(),
            };

            callback(description)?;
            count += 1;
        }

        Ok(count)
    }

    /// Parse SNOMED CT Relationships from a TSV file
    pub fn parse_relationships<P: AsRef<Path>, F>(path: P, mut callback: F) -> Result<usize>
    where
        F: FnMut(SnomedRelationship) -> Result<()>,
    {
        let file = std::fs::File::open(path.as_ref())
            .context("Failed to open relationships file")?;
        let reader = BufReader::new(file);
        let mut lines = reader.lines();

        // Skip header line
        lines.next().context("No header line in relationships file")?
            .context("Failed to read header line")?;

        let mut count = 0;
        for line in lines {
            let line = line.context("Failed to read line")?;
            if line.trim().is_empty() {
                continue;
            }

            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 10 {
                continue; // Skip malformed lines
            }

            let relationship = SnomedRelationship {
                id: fields[0].to_string(),
                effective_time: fields[1].to_string(),
                active: fields[2] == "1",
                module_id: fields[3].to_string(),
                source_id: fields[4].to_string(),
                destination_id: fields[5].to_string(),
                relationship_group: fields[6].parse().unwrap_or(0),
                type_id: fields[7].to_string(),
                characteristic_type_id: fields[8].to_string(),
                modifier_id: fields[9].to_string(),
            };

            callback(relationship)?;
            count += 1;
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_concept_line() {
        // Test parsing a single concept
        let test_data = "id\teffectiveTime\tactive\tmoduleId\tdefinitionStatusId\n\
                         12345\t20230101\t1\t67890\t900000000000074008";

        let mut count = 0;
        let result = SnomedRf2Parser::parse_concepts(
            test_data.as_bytes(),
            |concept| {
                assert_eq!(concept.id, "12345");
                assert_eq!(concept.effective_time, "20230101");
                assert!(concept.active);
                count += 1;
                Ok(())
            }
        );

        assert!(result.is_ok());
    }
}
