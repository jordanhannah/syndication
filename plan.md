apologies. instead of 'MP' it should be 'MP PT' and instead of 'TP' -> 'TPP TP PT' in fact here are the real AMT csv headers

```csv
CTPP SCTID,CTPP PT,ARTG_ID,TPP SCTID,TPP PT,TPUU SCTID,TPUU PT,TPP TP SCTID,TPP TP PT,TPUU TP SCTID,TPUU TP PT,MPP SCTID,MPP PT,MPUU SCTID,MPUU PT,MP SCTID,MP PT
```

Plan: Update AMT CSV Headers and Index Configuration

Understanding from Sample Data:

- Hierarchy flows RIGHT to LEFT: MP PT (parent) ← MPUU PT ← MPP PT ← TPUU TP PT ← TPP TP PT ← TPUU
  PT ← TPP PT ← CTPP PT (most specific)
- Patient Index: MP PT + TPP TP PT columns
- Doctor Index: MP PT + MPUU PT + TPP TP PT + TPUU PT columns

Changes Required:

1.  CLAUDE.md (Documentation)

- Line 97: Change "AMT Medications Patient Index (MP, TP only)" → "AMT Medications Patient Index
  (MP PT, TPP TP PT)"
- Line 98: Change "AMT Medications Doctor Index (MP, TP, MPUU, TPUU)" → "AMT Medications Doctor
  Index (MP PT, MPUU PT, TPP TP PT, TPUU PT)"

2.  src/parsers/amt_csv.rs (Parser + Tests)

- Line 41: Update documentation comment to list all 17 CSV columns
- Lines 72-82: Reverse the hierarchy array to reflect correct parent-child relationships (MP as
  root parent, CTPP as leaf)
- Lines 273-276: Update test CSV header to include all columns: CTPP SCTID,CTPP PT,ARTG_ID,TPP
  SCTID,TPP PT,TPUU SCTID,TPUU PT,TPP TP SCTID,TPP TP PT,TPUU TP SCTID,TPUU TP PT,MPP SCTID,MPP
  PT,MPUU SCTID,MPUU PT,MP SCTID,MP PT
- Lines 277-285: Add test rows with all product types (using your provided sample data)
- Lines 296-324: Add test assertions for TPUU, TPP TP, TPUU TP, MPUU product types

3.  src/commands.rs (Search API Comments)

- Line 409: Update comment to clarify patient search uses MP PT and TPP TP PT columns
- Line 416: Update comment to clarify doctor search includes MP PT, MPUU PT, TPP TP PT, TPUU PT

4.  src/search.rs (Index Configuration - if needed)

- Verify that search filtering correctly handles:
  - Patient index: code_type IN ("MP", "TPP TP")
  - Doctor index: code_type IN ("MP", "MPUU", "TPP TP", "TPUU")

No Changes Needed:

- src/storage.rs - AmtCode struct is already flexible
- src/import.rs - Uses parser callbacks (type-agnostic)
- src/queries.rs - Type-agnostic queries
- Database schema - Already supports all product types

Doctors index:
MP, MPUU, TPUU TP, TPUU

Patient's index:
MP, TPUU TP

\*i never want the following if the include an exact match for MP:
TPP TP, TPUU TP, TPP, TPUU
