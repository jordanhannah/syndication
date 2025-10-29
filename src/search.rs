use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::tokenizer::NgramTokenizer;
use tantivy::{doc, Index, IndexReader, IndexWriter, ReloadPolicy, Term};

/// Search result from Tantivy indexes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub code: String,
    pub system: String,
    pub display: String,
    pub terminology_type: String,
    pub active: bool,
    pub score: f32,
    pub subtype: Option<String>,
}

/// Tantivy search engine for terminology search
pub struct TerminologySearch {
    snomed_index: Index,
    snomed_reader: IndexReader,
    snomed_writer: IndexWriter,

    amt_index: Index,
    amt_reader: IndexReader,
    amt_writer: IndexWriter,

    valueset_index: Index,
    valueset_reader: IndexReader,
    valueset_writer: IndexWriter,
}

impl TerminologySearch {
    /// Create a new search engine with indexes stored in the given directory
    pub fn new(index_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(index_dir)?;

        // Create SNOMED index
        let snomed_dir = index_dir.join("snomed");
        std::fs::create_dir_all(&snomed_dir)?;
        let snomed_index = Self::create_snomed_index(&snomed_dir)?;
        let snomed_reader = snomed_index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let snomed_writer = snomed_index.writer(50_000_000)?; // 50MB heap

        // Create AMT index
        let amt_dir = index_dir.join("amt");
        std::fs::create_dir_all(&amt_dir)?;
        let amt_index = Self::create_amt_index(&amt_dir)?;
        let amt_reader = amt_index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let amt_writer = amt_index.writer(50_000_000)?;

        // Create ValueSet index
        let valueset_dir = index_dir.join("valuesets");
        std::fs::create_dir_all(&valueset_dir)?;
        let valueset_index = Self::create_valueset_index(&valueset_dir)?;
        let valueset_reader = valueset_index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;
        let valueset_writer = valueset_index.writer(50_000_000)?;

        Ok(Self {
            snomed_index,
            snomed_reader,
            snomed_writer,
            amt_index,
            amt_reader,
            amt_writer,
            valueset_index,
            valueset_reader,
            valueset_writer,
        })
    }

    /// Create SNOMED description index schema
    fn create_snomed_index(index_dir: &Path) -> Result<Index> {
        let mut schema_builder = Schema::builder();

        schema_builder.add_text_field("concept_id", STRING | STORED);

        // Configure term field with trigram tokenizer for better fuzzy search
        let text_field_indexing = TextFieldIndexing::default()
            .set_tokenizer("trigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);

        let text_options = TextOptions::default()
            .set_indexing_options(text_field_indexing)
            .set_stored();

        schema_builder.add_text_field("term", text_options);
        schema_builder.add_text_field("type_id", STRING);
        schema_builder.add_u64_field("active", INDEXED | STORED);

        let schema = schema_builder.build();
        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(index_dir)?, schema)?;

        // Register trigram tokenizer BEFORE any indexing operations
        index.tokenizers().register(
            "trigram",
            NgramTokenizer::new(3, 3, false).unwrap(),
        );

        Ok(index)
    }

    /// Create AMT code index schema
    fn create_amt_index(index_dir: &Path) -> Result<Index> {
        let mut schema_builder = Schema::builder();

        schema_builder.add_text_field("code", STRING | STORED);

        // Configure preferred_term field with trigram tokenizer for better fuzzy search
        let text_field_indexing = TextFieldIndexing::default()
            .set_tokenizer("trigram")
            .set_index_option(IndexRecordOption::WithFreqsAndPositions);

        let text_options = TextOptions::default()
            .set_indexing_options(text_field_indexing)
            .set_stored();

        schema_builder.add_text_field("preferred_term", text_options);
        schema_builder.add_text_field("code_type", STRING | STORED | FAST);

        let schema = schema_builder.build();
        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(index_dir)?, schema)?;

        // Register trigram tokenizer BEFORE any indexing operations
        index.tokenizers().register(
            "trigram",
            NgramTokenizer::new(3, 3, false).unwrap(),
        );

        Ok(index)
    }

    /// Create ValueSet index schema
    fn create_valueset_index(index_dir: &Path) -> Result<Index> {
        let mut schema_builder = Schema::builder();

        schema_builder.add_text_field("url", STRING | STORED);
        schema_builder.add_text_field("title", TEXT | STORED);
        schema_builder.add_text_field("name", TEXT);
        schema_builder.add_text_field("description", TEXT);

        let schema = schema_builder.build();
        let index = Index::open_or_create(tantivy::directory::MmapDirectory::open(index_dir)?, schema)?;

        Ok(index)
    }

    /// Index a SNOMED description
    pub fn index_snomed_description(
        &mut self,
        concept_id: &str,
        term: &str,
        type_id: &str,
        active: bool,
    ) -> Result<()> {
        let schema = self.snomed_index.schema();
        let concept_field = schema.get_field("concept_id")?;
        let term_field = schema.get_field("term")?;
        let type_field = schema.get_field("type_id")?;
        let active_field = schema.get_field("active")?;

        self.snomed_writer.add_document(doc!(
            concept_field => concept_id,
            term_field => term,
            type_field => type_id,
            active_field => if active { 1u64 } else { 0u64 },
        ))?;

        Ok(())
    }

    /// Index an AMT code
    pub fn index_amt_code(
        &mut self,
        code: &str,
        preferred_term: &str,
        code_type: &str,
    ) -> Result<()> {
        let schema = self.amt_index.schema();
        let code_field = schema.get_field("code")?;
        let term_field = schema.get_field("preferred_term")?;
        let type_field = schema.get_field("code_type")?;

        self.amt_writer.add_document(doc!(
            code_field => code,
            term_field => preferred_term,
            type_field => code_type,
        ))?;

        Ok(())
    }

    /// Index a ValueSet
    pub fn index_valueset(
        &mut self,
        url: &str,
        title: Option<&str>,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        let schema = self.valueset_index.schema();
        let url_field = schema.get_field("url")?;
        let title_field = schema.get_field("title")?;
        let name_field = schema.get_field("name")?;
        let desc_field = schema.get_field("description")?;

        let mut doc = doc!(url_field => url);

        if let Some(t) = title {
            doc.add_text(title_field, t);
        }
        if let Some(n) = name {
            doc.add_text(name_field, n);
        }
        if let Some(d) = description {
            doc.add_text(desc_field, d);
        }

        self.valueset_writer.add_document(doc)?;

        Ok(())
    }

    /// Commit all pending changes
    pub fn commit(&mut self) -> Result<()> {
        self.snomed_writer.commit()?;
        self.amt_writer.commit()?;
        self.valueset_writer.commit()?;

        // Reload readers
        self.snomed_reader.reload()?;
        self.amt_reader.reload()?;
        self.valueset_reader.reload()?;

        Ok(())
    }

    /// Clear all indexed data (for re-import)
    pub fn clear_all(&mut self) -> Result<()> {
        self.snomed_writer.delete_all_documents()?;
        self.amt_writer.delete_all_documents()?;
        self.valueset_writer.delete_all_documents()?;
        self.commit()?;
        Ok(())
    }

    /// Search SNOMED descriptions
    pub fn search_snomed(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let schema = self.snomed_index.schema();
        let term_field = schema.get_field("term")?;
        let concept_field = schema.get_field("concept_id")?;
        let active_field = schema.get_field("active")?;

        let searcher = self.snomed_reader.searcher();
        let query_parser = QueryParser::for_index(&self.snomed_index, vec![term_field]);

        // Parse query (trigram tokenizer handles fuzzy matching naturally)
        let query = query_parser.parse_query(query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let concept_id = retrieved_doc
                .get_first(concept_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let term = retrieved_doc
                .get_first(term_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let active = retrieved_doc
                .get_first(active_field)
                .and_then(|v| v.as_u64())
                .unwrap_or(0) == 1;

            results.push(SearchResult {
                code: concept_id,
                system: "http://snomed.info/sct".to_string(),
                display: term,
                terminology_type: "snomed".to_string(),
                active,
                score,
                subtype: None,
            });
        }

        Ok(results)
    }

    /// Search AMT codes with optional code type filtering
    pub fn search_amt(&self, query: &str, limit: usize, code_types: Option<&[String]>) -> Result<Vec<SearchResult>> {
        let schema = self.amt_index.schema();
        let term_field = schema.get_field("preferred_term")?;
        let code_field = schema.get_field("code")?;
        let type_field = schema.get_field("code_type")?;

        let searcher = self.amt_reader.searcher();

        // Build term query for preferred_term field
        let query_parser = QueryParser::for_index(&self.amt_index, vec![term_field]);
        let term_query = query_parser.parse_query(query)?;

        // Build final query with optional code type filter
        let final_query: Box<dyn tantivy::query::Query> = if let Some(types) = code_types {
            if types.is_empty() {
                Box::new(term_query)
            } else {
                // Build nested BooleanQuery structure:
                // Top level: (term MUST match) AND (code_type filter MUST match)
                // Nested level: (type1 SHOULD match) OR (type2 SHOULD match) OR ...

                // Build the nested code_type filter (at least one type must match)
                let type_queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = types.iter()
                    .map(|t| {
                        let term = Term::from_field_text(type_field, t);
                        (Occur::Should, Box::new(TermQuery::new(term, tantivy::schema::IndexRecordOption::Basic)) as Box<dyn tantivy::query::Query>)
                    })
                    .collect();

                let type_filter = BooleanQuery::new(type_queries);

                // Build top-level query: both clauses are MUST
                let final_clauses = vec![
                    (Occur::Must, Box::new(term_query) as Box<dyn tantivy::query::Query>),
                    (Occur::Must, Box::new(type_filter) as Box<dyn tantivy::query::Query>),
                ];

                Box::new(BooleanQuery::new(final_clauses))
            }
        } else {
            Box::new(term_query)
        };

        let top_docs = searcher.search(&*final_query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let code = retrieved_doc
                .get_first(code_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let term = retrieved_doc
                .get_first(term_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let code_type = retrieved_doc
                .get_first(type_field)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            results.push(SearchResult {
                code,
                system: "http://hl7.org/fhir/sid/ncts-amt".to_string(),
                display: term,
                terminology_type: "amt".to_string(),
                active: true,
                score,
                subtype: code_type,
            });
        }

        Ok(results)
    }

    /// Search ValueSets
    pub fn search_valuesets(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let schema = self.valueset_index.schema();
        let url_field = schema.get_field("url")?;
        let title_field = schema.get_field("title")?;
        let name_field = schema.get_field("name")?;
        let desc_field = schema.get_field("description")?;

        let searcher = self.valueset_reader.searcher();
        let query_parser = QueryParser::for_index(
            &self.valueset_index,
            vec![title_field, name_field, desc_field, url_field],
        );

        let query = query_parser.parse_query(query)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved_doc: tantivy::TantivyDocument = searcher.doc(doc_address)?;

            let url = retrieved_doc
                .get_first(url_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let title = retrieved_doc
                .get_first(title_field)
                .and_then(|v| v.as_str())
                .unwrap_or(&url)
                .to_string();

            results.push(SearchResult {
                code: url.clone(),
                system: "http://hl7.org/fhir/ValueSet".to_string(),
                display: title,
                terminology_type: "valuesets".to_string(),
                active: true,
                score,
                subtype: None,
            });
        }

        Ok(results)
    }

    /// Search across all terminologies
    pub fn search_all(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let per_terminology = limit / 3;

        let mut results = Vec::new();
        results.extend(self.search_snomed(query, per_terminology)?);
        results.extend(self.search_amt(query, per_terminology, None)?);
        results.extend(self.search_valuesets(query, per_terminology)?);

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
        results.truncate(limit);

        Ok(results)
    }

    /// Check if SNOMED index is empty
    pub fn is_snomed_empty(&self) -> Result<bool> {
        let searcher = self.snomed_reader.searcher();
        Ok(searcher.num_docs() == 0)
    }

    /// Check if AMT index is empty
    pub fn is_amt_empty(&self) -> Result<bool> {
        let searcher = self.amt_reader.searcher();
        Ok(searcher.num_docs() == 0)
    }

    /// Check if ValueSet index is empty
    pub fn is_valueset_empty(&self) -> Result<bool> {
        let searcher = self.valueset_reader.searcher();
        Ok(searcher.num_docs() == 0)
    }

    /// Clear SNOMED index only
    pub fn clear_snomed(&mut self) -> Result<()> {
        self.snomed_writer.delete_all_documents()?;
        self.snomed_writer.commit()?;
        self.snomed_reader.reload()?;
        Ok(())
    }

    /// Clear AMT index only
    pub fn clear_amt(&mut self) -> Result<()> {
        self.amt_writer.delete_all_documents()?;
        self.amt_writer.commit()?;
        self.amt_reader.reload()?;
        Ok(())
    }

    /// Clear ValueSet index only
    pub fn clear_valuesets(&mut self) -> Result<()> {
        self.valueset_writer.delete_all_documents()?;
        self.valueset_writer.commit()?;
        self.valueset_reader.reload()?;
        Ok(())
    }
}
