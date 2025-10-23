# NCTS Resources

## https://www.healthterminologies.gov.au/specs/v3/content-types/concepts/

Concepts

This section provides the background required to describe and conceptualise the NCTS content types through introduction of shared terminology, properties and characteristics.
Coding System Identifier

A Coding System Identifier is used to identify a coding system. It is typically used when using a code from a coding system as a value in a coded data element, for example in a structured message or document.
Content Type

A content type is a classification identifying units of information exchange. A terminology content type defines a specific kind of terminology content, along with its associated metadata, release and distribution formats, and versioning schemes.

This document defines a Content Type as an NCTS-specific terminology content type. Content Types are the terminology content types that have been defined for and are relevant to NCTS infrastructure services, applications and technical specifications.
Content Item

A Content Item is a concrete instance of a Content Type, and conforms to all the mandatory conformance points for that Content Type within this technical specification. All Content Items possess the following two attributes:

    Content Item Identifier
    Content Item Version

Content Item Identifier

Content Item Identifiers are technical identifiers that identify a Content Item independent of any specific version of that item. The identifier allows the Content Item to be recognised regardless of where it may be encountered. The existence of a Content Item Identifier allows assertions to be made about the item without reference to a version of the item – this can facilitate management of multiple instances of the item held in systems designed for sharing and distributing Content Items.

Content Item Identifiers should not be confused with Coding System Identifiers. While similar, each serves a different purpose. For some Content Types, the Content Item Identifier and Coding System Identifier are the same (e.g. LOINC); for others they are different (e.g. SNOMED CT). See Summary for details.
Content Item Version

A Content Item Version specifies the version of a Content Item.

In the NCTS, Content Item Versions must have a property that allows implementers to determine version order from oldest to most recent, given two or more Content Item Version values associated with the same Content Item Identifier. Versioning semantics for each Content Type will be detailed in the relevant sections of this specification.
Distribution Format

Distribution Format refers to the technical structure of terminology content provided by the content producer for the distribution of the Content Item, such as a release file, package of release files, or the payload delivered in response to a request to a server.

Distribution Formats may be associated with an Internet Assigned Numbers Authority (IANA) assigned media type (formerly known as MIME types).

## https://www.healthterminologies.gov.au/specs/v3/content-types/snomed-ct/

SNOMED CT

SNOMED CT® (Systematized Nomenclature of Medicine – Clinical Terms) terminologies are a supported NCTS Content Type. SNOMED CT terminologies are produced by SNOMED International, its member nations and affiliates. SNOMED International provides the international release of SNOMED CT, member nations provide extensions as their national release of SNOMED CT, and affiliates may produce their own extensions of either the international release or a member nation's extension.

This section describes the conformance points for using SNOMED CT terminologies as an NCTS Content Type.
Coding System Identifier

    CP 1

    The following URI SHALL be the Coding System Identifier for the SNOMED CT Content Type:

    http://snomed.info/sct

Note

This URI is provided by the IHTSDO as the system URI for all SNOMED CT concepts across all editions. Further, this URI is explicitly stated in the HL7™ FHIR® standard Specification as the system URI when referring to SNOMED CT codes, for example for use with FHIR Coding and CodeableConcept data types.
Content Item Identifier

    CP 2

    The Content Item Identifier for the SNOMED CT Content Type SHALL be the SNOMED CT edition URI.

Example

http://snomed.info/sct/32506021000036107
Note

A technical description of the SNOMED CT edition URI is described in the SNOMED CT URI Standard. It provides a unique identifier for any SNOMED CT terminology, based on the module ID.
Content Item Version

    CP 3

    The Content Item Version for the SNOMED CT Content Type SHALL be the SNOMED CT Version URI.

Example

http://snomed.info/sct/32506021000036107/version/20160430
Note

A technical description of the SNOMED CT version URI is described in the SNOMED CT URI Standard. It provides a unique version identifier for any SNOMED CT terminology, based on the module ID and a timestamp.
Distribution Formats

    CP 5

    Each Distribution Format for the SNOMED CT Content Type SHALL consist of a ZIP file containing files that conform to SNOMED CT Release Format 2 (RF2).

Note

A technical description of RF2 is available in the SNOMED CT Technical Implementation Guide. RF2 supports Full, Snapshot, and Delta releases of SNOMED CT terminologies.

    CP 6

    The following SNOMED CT Distribution Formats SHALL be defined:

        Delta: Containing only Delta RF2 release files
        Snapshot: Containing only Snapshot RF2 release files
        Full: Containing only Full RF2 release files
        All: Containing the Delta, Snapshot, and Full RF2 release files

    CP 7

    Each SNOMED CT Distribution Format SHALL be a single ZIP file with the following structure and folder naming conventions:

    <SNOMED CT _Distribution Format_ ZIP file>

        SnomedCT_Release_<Country|Namespace>_<VersionDate>
        RF2Release
            <Type>
            Refset
                Content
                Language
                Map
                Metadata
            Terminology

    Where:

        <SNOMED CT _Distribution Format_ ZIP file> is the single ZIP file distribution format.
        <Country|Namespace>_<VersionDate> SHALL be replaced with values corresponding to the SNOMED CT release as described in the SNOMED Technical Implementation Guide.
        <Type> is a folder in the ZIP file and SHALL have a value of either Delta, Snapshot, or Full, depending on the Distribution Format.
        The contents of the <Type> folder SHALL conform to "File Naming Conventions" within the SNOMED Technical Implementation Guide.
        The contents of the <Type> folder SHALL be RF2 files as described in the SNOMED CT-AU Technical Implementation Guide.

Note

For the Delta, Snapshot, and Full Distribution Formats only a single <Type> folder is required. For the All Distribution Format, three <Type> folders are required, one for each of Delta, Snapshot, and Full.

    CP 8

    The IANA media type for the SNOMED CT Distribution Format SHALL be application/zip.

## https://www.healthterminologies.gov.au/specs/v3/content-types/amt-flat-file/

AMT Flat File

This section describes the AMT Flat File as an NCTS Content Type.

The AMT Flat File is a Comma Separated Values (CSV) file containing columns for SNOMED CT identifiers and the preferred terms of all 7 notable concept classes in AMT, and rows for every active product in a release of AMT.
Note

The file is being released to assist some specific development activities. It will only be provided for a limited period and any use of it is contingent on the licensee acknowledging that it may be withdrawn with only 90 days’ notice.

Any licensee needing to use this file should first contact the NCTS at help@digitalhealth.gov.au for further information.
Microsoft Excel usage warning

Important: If you have configured Microsoft Excel to automatically open CSV files, it will assume the field type to be General and the identifiers and some text may be displayed incorrectly resulting in ID digit errors and strange characters for words with accents (e.g. Ménière's Diseases). To avoid this, users should first open Excel, and import the file (not simply open) from within to have the opportunity to change the settings. The settings should specify that the file be tab delimited, with the file origin of ‘unicode (UTF-8)’ and the ID column is set to Text (not General).
Content Item Identifier

    CP 114

    The Content Item Identifier for an AMT Flat File SHALL be http://amt.info/csv.

Content Item Version

    CP 125

    The Content Item Version for an NCTS Complete Code System SHALL be http://amt.info/csv/900062011000036108/version/VERSION where VERSION is the SNOMED CT-AU release version in YYYYMMDD format. For example http://amt.info/csv/900062011000036108/version/20190930.

## https://www.healthterminologies.gov.au/specs/v3/content-types/loinc/

LOINC

LOINC™ (Logical Observation Identifiers Names and Codes) is a common language (set of identifiers, names, and codes) for clinical and laboratory observations.

LOINC is supported as an NCTS Content Type. LOINC is produced by the Regenstrief Institute and is the only NCTS Content Type that has a single source for new release instances (Content Items). This section describes the conformance points for using the LOINC terminology as an NCTS Content Type.
Coding System Identifier

    CP 9

    The following URI SHALL be the Coding System Identifier for the LOINC code system:

    http://loinc.org

Note

The URL http://loinc.org is explicitly stated in the FHIR® specification as the system URI when using LOINC codes.
Content Item Identifier

    CP 10

    The following URI SHALL be the Content Item Identifier for the LOINC Content Type:

    http://loinc.org

Content Item Version

    CP 11

    The Content Item Version for the LOINC Content Type SHALL be the "x.yy" version (e.g. 2.52) of the LOINC release.

Distribution Format

    CP 13

    The Distribution Format for LOINC is a set of comma separated values (CSV) and Microsoft Excel spreadsheet (XLSX) files, formatted as described in the LOINC User's Guide.
    CP 14

    The Distribution Format for LOINC SHALL be a ZIP file containing the structure and necessary files from the LOINC ZIP release from the Regenstrief Institute.
    CP 15

    The LOINC Distribution Format ZIP file MAY exclude any irrelevant content (e.g. documentation, platform specific databases and utility applications), where omission of this content is allowed by the LOINC licence.

Note

The Regenstrief Institute releases LOINC using a ZIP archive that contains a variety of files. Many of these files are additional to the core content of LOINC and may be safely excluded to minimise the size of the archive for easier distribution and processing.

    CP 16

    The LOINC Distribution Format SHALL be a single ZIP file with the following structure and folder naming conventions:

        <LOINC Distribution Format ZIP file>
            AccessoryFiles
                DocumentOntology
                    DocumentOntology.csv
                LOINC_<Version>_MULTI-AXIAL_HIERARCHY.CSV
                PanelsForms
                    LOINC_<Version>_PanelsAndForms.xlsx
            loinc.csv

    Where <Version> SHALL be replaced with the version of the LOINC release minus the point, for example 252 for version 2.52 of LOINC.

Note

Implementers are advised that the files and folders specified in conformance point CT 16 are a minimum set, and additional files may be added. As the structure above is a subset of the files and folders of the Regenstrief Institute release of LOINC, the ZIP file provided by the Regenstrief Institute itself is an acceptable realisation of the NCTS LOINC Distribution Format.

    CP 17

    The media type for the LOINC Distribution Format SHALL be application/zip.

## https://www.healthterminologies.gov.au/specs/v3/content-types/fhir/

FHIR® Content Types

FHIR from Health Level Seven (HL7) provides a number of resources to support use of FHIR with terminology content. This specification provides profiles of the core FHIR CodeSystem, ValueSet and ConceptMap resources, thereby making them appropriate for use in other NCTS technical specifications.

This specification defines a set of profiles (expressed as FHIR StructureDefinition resources) which further constrain the base resources defined by the core FHIR specification. The purpose of these profiles is to codify best practices for managing terminology content in FHIR, and also to allow some simplifying assumptions to be made by implementers that will serve to improve consistency and interoperability within the NCTS ecosystem.

It is recommended that FHIR terminology resources assert the appropriate NCTS profile, where possible. This will result in more consistent behaviour within conformant terminology server applications (CTSAs), and a reduction in both server and client implementation complexity.

The following conformance points apply to all three of these resources when used as NCTS Content Types.

    The phrase "NCTS FHIR Content Type" refers to an NCTS Content Type defined as a profile of a FHIR resource.
    The phrase "NCTS FHIR Content Item" is used to refer to an instance of an NCTS FHIR Content Type.

Use of FHIR

    CP 18

    The NCTS FHIR Content Types define a set of profiles of resources defined within the R4 release of the HL7 FHIR Specification. Unless otherwise stated in this specification, all implementations SHALL conform to the relevant definitions, data types, cardinalities, constraints and other conformance requirements within the FHIR Specification.

Distribution Formats

    CP 19

    The Distribution Formats for each NCTS FHIR Content Type SHALL be Extensible Markup Language (XML) and JavaScript Object Notation (JSON), as specified in the FHIR Specification.

Versioning

    CP 20

    The version element SHALL contain a version value which is formatted as either:

        a "YYYYMMDD" date-only timestamp value e.g. 20160531 (the Timestamp Version Format), or;
        an "x.y.z" version value which is based on the Semantic Versioning 2.0.0 versioning scheme (the Semantic Version Format).

    CP 21

    The version format used for the initial version of an NCTS FHIR Content Item SHALL be used for all subsequent versions.

Note

The above conformance point ensures that multiple versions of an NCTS FHIR Content Item have comparable Content Item Version values. This ensures that they can be ordered from oldest to most recent.

With respect to source terminologies, not all native versioning formats will comply with these formats exactly. However, most versioning formats should be able to map onto these formats in a reasonably intuitive way. The cost of representing these terminologies in FHIR with an altered versioning system is offset by the benefit of server implementations being able to deal with version ordering consistently.

    CP 22

    Version numbers that use the Timestamp Version Format SHALL be ordered according to the dates they are derived from, with the most recent date representing the current version.

    CP 23

    Version numbers that use the Semantic Version Format SHALL be ordered according to the precedence rules defined within the Semantic Versioning specification.

Note

Versioning using the Semantic Version Format must follow the rules defined in Semantic Versioning 2.0.0. In short this is defined as

    Given a version number MAJOR.MINOR.PATCH, increment the:

        MAJOR version when you make incompatible API changes,
        MINOR version when you add functionality in a backwards compatible manner, and
        PATCH version when you make backwards compatible bug fixes.

In practice these rules must be applied to each resource based on its context. For example one type of change in the context of one resource may be considered minor, yet in the context of another resource it may be major.

The correct action must carefully consider impact to all consumers in the scope of the resource's use. Specifically, whether the change to the resource constitutes a breaking change, backwards compatible feature addition, or backwards compatible bug fix in the context of the specific resource and its users.

Typically, a more major version increment is more disruptive and should be avoided if not warranted by the change, however subtle errors can occur when changes are under-categorised. Generally, it is preferable to err towards incrementing a version more significantly than required than fail to increment it sufficiently for the change.

Examples of potentially breaking change depending upon the context of the resource are:

    Removal of codes from a ValueSet.
    Expansion of scope of a ValueSet.
    Addition of codes to a ValueSet.
    Inclusion of codes from a CodeSystem previously not present in a ValueSet.
    Removal of a mapping for a code in a ConceptMap.
    Introduction of one to many, or many to many mappings in a one to one ConceptMap.
    Removal of codes from a CodeSystem.
    Change of meaning of a code in a CodeSystem.
    Change to caseSensitive or versionNeeded fields in a CodeSystem.
    Removal of a property or filter from a CodeSystem.
    Significant change to the purpose of a resource.
    Change in copyright for a resource.

These are examples which may or may not be relevant to a particular resource, and does not represent an exhaustive list. Careful thought must be put into the context of changes to a resource and the appropriate representation of these changes in the version.

If you are unsure about how you should apply versioning to your resources please contact help@digitalhealth.gov.au for assistance.

## https://www.healthterminologies.gov.au/specs/v3/content-types/complete-code-system/

NCTS Complete Code System

This section describes the NCTS Complete Code System, a profile of the FHIR® CodeSystem resource. This Content Type is designed to represent existing coding systems, or to define new coding systems.

Note that this profile is not designed to be used to describe "external code systems" such as SNOMED CT and LOINC. A description of the ways that SNOMED CT and LOINC CodeSystems are represented within NCTS conformant server applications is available here.

    CP 24

    The version of the NCTS Complete Code System defined in this specification SHALL be 4.0.0.

    CP 25

    Instances of the NCTS Complete Code System SHALL conform to the specification of the CodeSystem resource within the FHIR Specification.

    CP 26

    An NCTS Complete Code System SHALL conform to the StructureDefinition resource available at the following canonical URL:

    https://healthterminologies.gov.au/fhir/StructureDefinition/complete-code-system-4

Content Item Identifier

    CP 27

    The Content Item Identifier for an NCTS Complete Code System SHALL be the CodeSystem.url value.

Content Item Version

    CP 28

    The Content Item Version for an NCTS Complete Code System SHALL be the CodeSystem.version value.

General conformance points

    CP 30

    NCTS Complete Code System instances SHALL NOT be used to completely or partially represent a SNOMED CT terminology.

    CP 31

    NCTS Complete Code System instances SHALL NOT be used to completely or partially represent LOINC.

Note

The rationale for the two conformance points above is that the FHIR specification defines a way of representing implicit ConceptMap resources for both SNOMED CT and LOINC. These implicit CodeSystems are incomplete in the sense that they contain metadata about the system, in the absence of the codes themselves.

Representing SNOMED CT and LOINC using the NCTS Complete Code System profile would be problematic due to the size of these code systems, as the profile mandates completeness through the CodeSystem.content element. It would also not serve the goal of having a single canonical FHIR-based representation for each source code system.
CodeSystem Supplements

FHIR allows creation of CodeSystem resources that extend other CodeSystems with additional properties and designations. These resources identify a CodeSystem they extend with the element CodeSystem.supplements.

The NCTS Complete Code System profile is intended to create FHIR CodeSystem resources that describe a CodeSystem in its entirety, not supplements. Therefore CodeSystem.supplement has been prohibited in the NCTS Complete Code System profile.

## https://www.healthterminologies.gov.au/specs/v3/content-types/composed-value-set/

NCTS Composed Value Set

This section describes the NCTS Composed Value Set, a profile of the FHIR® ValueSet resource. The NCTS Composed Value Set is designed to support the creation of ValueSets that are defined through composition of codes from one or more coding systems. This includes SNOMED CT terminologies, the LOINC coding system, and codes defined in NCTS Complete Code Systems.

    CP 32

    The version of the NCTS Composed Value Set defined in this specification SHALL be 4.0.0.

    CP 33

    Instances of the NCTS Composed Value Set SHALL conform to the specification of the ValueSet resource within the FHIR Specification.

    CP 34

    An NCTS Composed Value Set SHALL conform to the StructureDefinition resource available at the following canonical URL:

    https://healthterminologies.gov.au/fhir/StructureDefinition/composed-value-set-4

Content Item Identifier

    CP 35

    The Content Item Identifier for an NCTS Composed Value Set SHALL be the ValueSet.url value.

Content Item Version

    CP 36

    The Content Item Version for an NCTS Composed Value Set SHALL be the ValueSet.version value.

General conformance points

    CP 40

    The ValueSet.compose.include.concept.code value SHALL NOT be a post-coordinated expression.

    CP 41

    If ValueSet.compose.include.concept.designation values are used, the ValueSet.compose.include.concept.designation.use element SHALL have a value.

Note

If a designation is provided, this conformance point ensures that there is clarity around what type of designation it is. The FHIR Specification defines a binding for this field with values drawn from SNOMED CT.
Referencing code systems

    CP 122

    The ValueSet.compose.include.system element within an NCTS Composed Value Set SHALL only be used to reference:

        A CodeSystem resource representing a SNOMED CT Content Item
        A CodeSystem resource representing a LOINC Content Item
        An NCTS Complete Code System

Note

If NCTS Composed Value Sets were allowed to include CodeSystem resources not asserting the NCTS Complete Code System profile, the benefits of orderable versioning would be lost. Enforcing orderable versioning means that NCTS Composed Value Sets may refer to NCTS Complete Code Systems without specifying a business version. This is useful in cases where the code system has concept permanence, and it is desirable to always refer to its latest version.

    CP 45

    When including SNOMED CT codes in NCTS Composed Value Set instances:

        The associated ValueSet.compose.include.system value for these codes SHALL be the Coding System Identifier for SNOMED CT, and;
        The associated ValueSet.compose.include.version value for these codes SHALL be the Content Item Version of the SNOMED CT terminology in use.

    CP 46

    When including LOINC codes in NCTS Composed Value Set instances:

        the associated ValueSet.compose.include.system value for these codes SHALL be the Coding System Identifier for LOINC, and;
        the associated ValueSet.compose.include.version value for these codes SHALL be the Content Item Version of the LOINC code system in use.

## https://www.healthterminologies.gov.au/specs/v3/content-types/general-concept-map/

NCTS General Concept Map

This section describes the NCTS General Concept Map, a profile of the FHIR® ConceptMap resource. The NCTS General Concept Map is designed to support the representation of mappings between different ValueSets.

    CP 48

    The version of the NCTS General Concept Map defined in this specification SHALL be 4.0.0.

    CP 49

    Instances of the NCTS General Concept Map SHALL conform to the specification of the ConceptMap resource within the FHIR Specification.

    CP 50

    An NCTS General Concept Map SHALL conform to the StructureDefinition resource available at the following canonical URL:

    https://healthterminologies.gov.au/fhir/StructureDefinition/general-concept-map-4

Content Item Identifier

    CP 51

    The Content Item Identifier for an NCTS General Concept Map SHALL be the ConceptMap.url value.

Content Item Version

    CP 52

    The Content Item Version for an NCTS General Concept Map SHALL be the ConceptMap.version value.

Equivalence

Note that a number of values in the bound ValueSet for ConceptMap.group.element.target.equivalence in FHIR R5 have been removed. While this set has not yet stabalised, users of this profile should consider limiting use of this element to the following values to ease transition to R5 in future

    related-to

    equivalent

    broader

    narrower

    not-related-to Refer to http://build.fhir.org/valueset-concept-map-relationship.html to see the current R5 list of codes.
    Caution on advanced map feature use

    The ConceptMap resource is a relatively low maturity resource, subject to change. The elements required for a simple mapping are relatively stable, however elements supporting more advanced use cases such as rule based maps are still under development. This raises potential compatibility issues with current tooling, and potential migration issues to future FHIR versions.

    Specifically the unmapped, dependsOn and product elements should all be used with caution as they are currently under discussion and may be subject to change in FHIR R5 or beyond. Experience with, and tooling support for, these elements and their use cases is also very limited at this stage.

    If you feel your use case may require use of these elements, please contact help@digitalhealth.gov.au to help collect end use requirements and discuss options to minimise your exposure to future change.

Map source and target versioning

When creating maps, it is possible to specify either explicitly or implicitly (via omission)

    for the map as a whole
        source ValueSet version, and
        target ValueSet version;
    for a specific map group
        source CodeSystem version, and
        target CodeSystem version.

The FHIR specification states that source and target ValueSet version SHOULD be specified for a ConceptMap. While there are some circumstances where a non-version specific ValueSet may be the source or target of a map (for example an un-versioned SNOMED CT implicit ValueSet may be appropriate in the right circumstances), specifying ValueSet version for the map’s source and target is generally required to create a safe context for the map.

The specification of a source and/or target version inside a specific mapping group may be required depending upon whether the source and/or target CodeSystem/s have concept permanence.

That is, if a CodeSystem changes the meaning of code values across versions of the CodeSystem it does not have concept permanence and version must be specified to ensure a safe mapping. As a single version of a ValueSet may contain codes from multiple versions of a CodeSystem, where a CodeSystem does not have concept permanence it is unsafe to rely on the ValueSet version alone.

If a CodeSystem does have concept permanence, such as SNOMED CT, it is unnecessary to specify the version in the mapping group and may lead to maintenance burden if version is specified in each map group, depending upon the number of map groups used.

Generally, source and target ValueSet both require a version, while sourceVersion and targetVersion within a mapping group are only required for CodeSystems without concept permanence. However, valid use cases may exist that sit outside this generalisation.

## https://www.healthterminologies.gov.au/specs/v3/content-types/fhir-bundle/

FHIR Bundle

This section describes FHIR Bundle resources as an NCTS Content Type.

FHIR Bundles can be used to create a collection of FHIR resources, grouped together for a particular purpose. FHIR Bundle type

    collection is supported as a collection of FHIR resources to be persisted as a single Bundle resource, and
    batch is supported as a collection of FHIR resources to each be loaded if as they had been individual syndication entries.

Content Item Identifier

    CP 115

    The Content Item Identifier for a FHIR Bundle SHALL be a value unique to the Bundle assigned by the Bundle publisher, and SHOULD be an HTTP URI using the domain name of the publisher of the Bundle in the domain name segment of the URI.

Content Item Version

    CP 116

    The Content Item Version for an FHIR Bundle SHALL be a value assigned by the Bundle publisher that is unique to each version of the Bundle. The Content Item Version for an FHIR Bundle SHOULD be either

        a "YYYYMMDD" date-only timestamp value e.g. 20160531 (the Timestamp Version Format), or;
        an "x.y.z" version value which is based on the Semantic Versioning 2.0.0 versioning scheme (the Semantic Version Format).

## https://www.healthterminologies.gov.au/specs/v3/conformant-server-apps/syndication-api/

Syndication API

This section describes the Syndication API, and describes a set of conformance points that are required for CSSA implementations.

    CP 86

    The Syndication API defines a profiled subset of the operations defined within the Atom Publishing Protocol.

    Unless otherwise stated in the Syndication API conformance points, all implementations SHALL conform to the definitions, constraints and conformance requirements within the Atom Publishing Protocol specification.

Supported Operations

The following table shows the operations from the Atom Publishing Protocol that form part of the Syndication API.
Table 2: Syndication API operations AtomPub operation AtomPub specification URL Use case
Listing Collection Members https://tools.ietf.org/html/rfc5023#section-5.2 List all entries within a syndication feed.
Retrieving a Resource https://tools.ietf.org/html/rfc5023#section-5.4.1 Download the _Content Item_ referenced by an entry in the syndication feed.
Conformance Points

    CP 87

    A CSSA SHALL implement all operations defined within the Syndication API (see table above).

    CP 88

    The "Listing Collection Members" operation SHALL return a NCTS Syndication Feed.

## https://www.healthterminologies.gov.au/specs/v3/conformant-server-apps/syndication-api/syndication-feed/

NCTS Syndication Feed

The NCTS Syndication Feed is a profile of the Atom Syndication Format. It is returned by CSSA implementations in the response to the Listing Collection Members operation.

    CP 126

    The NCTS Syndication Feed defines a document that complies with the Atom Syndication Format.

    Unless otherwise stated in the NCTS Syndication Feed conformance points, all implementations SHALL conform to the definitions, constraints and conformance requirements within the Atom Syndication Format specification.

    CP 89

    The version of the NCTS Syndication Feed defined in this specification SHALL be 1.0.0.

Note

The NCTS Syndication Feed is versioned according to the rules of Semantic Versioning 2.0.0.

    CP 90

    The canonical URI associated with this version of the NCTS Syndication Feed SHALL be:

    http://ns.electronichealth.net.au/ncts/syndication/asf/profile/1.0.0

    CP 91

    The NCTS Atom Feed Document's <atom:feed> element SHALL include a <atom:entry> child element describing each Content Item being syndicated by the syndication server.

Note

A CSSA may include Content Items within its syndication feed which are not instances of the NCTS Content Types, or are instances of the NCTS Content Types that have alternative Distribution Formats.

    CP 107

    For <atom:entry> elements representing NCTS Content Items, there MUST be exactly one corresponding <atom:link> element.

    CP 92

    For <atom:entry> elements representing NCTS Content Items, <atom:link> elements MUST contain a type attribute. The type attribute MUST contain the corresponding media type defined within this specification.

    CP 93

    Each <atom:entry> representing an NCTS Content Item MUST be described by an <atom:category> element that conforms to the NCTS Atom Category Scheme.

    CP 94

    For NCTS Content Items, each <atom:entry> that contains an <atom:link> element MUST also contain a <ncts:sha256Hash> element. The <ncts:sha256Hash> element SHALL contain a hexadecimal encoding of the SHA-256 hash of the Content Item data.

Note

The intent of the <ncts:sha256Hash> element is to enable clients of the Syndication API to verify the integrity of the Content Item that they have downloaded via the "Retrieving a Resource" operation.

    CP 95

    If an <atom:entry> element originated from another NCTS Syndication Feed, an <atom:source> element SHALL be included as a child of the <atom:entry>.

    CP 96

    For NCTS Content Items, the <atom:entry> element SHALL include a single <ncts:contentItemIdentifier> extension element, containing the Content Item Identifier.

    CP 97

    For NCTS Content Items, the <atom:entry> element SHALL include a single <ncts:contentItemVersion> extension element, containing the Content Item Version.

    CP 99

    For NCTS FHIR® Content Items, the <atom:entry> SHALL include at least one <ncts:fhirProfile> element containing the StructureDefinition URI corresponding to that NCTS FHIR Content Type, as defined in this specification.

    CP 100

    For NCTS SNOMED CT Content Items using the Delta Distribution Format, the <atom:entry> element SHALL include a single <ncts:sctBaseVersion> element containing the Content Item Version of the SNOMED CT release to which the Delta release is intended to be applied.

Content Item Identifier

    CP 123

    Unless a specific conformance point exists in this specification for the Content Item Identifier for a FHIR resource type (refer to Content Types) the Content Item Identifier SHALL be

        the value of the url element of the FHIR resource if present, or
        be a URI uniquely identifying the resource defined by the author adding the resource to the syndication feed if the url element is not present.

Content Item Version

    CP 124

    Unless a specific conformance point exists in this specification for the Content Item Version for a FHIR resource type (refer to Content Types) the Content Item Version SHALL be

        the value of the version element of the FHIR resource if present, or
        a value uniquely identifying the business version of the resource defined by the author adding the resource to the syndication feed if the version element is not present.

    The Content Item Version SHOULD be

        a "YYYYMMDD" date-only timestamp value e.g. 20160531 (the Timestamp Version Format), or;
        an "x.y.z" version value which is based on the Semantic Versioning 2.0.0 versioning scheme (the Semantic Version Format).

    CP 112

    For FHIR Content Items, the <atom:entry> element SHOULD include at least one <ncts:fhirVersion> element containing publication.major from the FHIR Version Management Policy identifying the FHIR version to which the resource conforms. For example
    FHIR Release 	fhirVersion
    DSTU 2.1 	1.1
    STU3 	3.0
    R4 	4.0

    A full list of FHIR releases and their corresponding publication.major.minor versions can be found at the FHIR Publication (Version)History page.

    If multiple FHIR versions apply, these SHOULD be represented in multiple <ncts:fhirVersion> elements for the <atom:entry>.

    If the <atom:entry> element does not include the <ncts:fhirVersion> element, then the resource is assumed to conform to version 3.0.1 of FHIR.

    CP 113

    For FHIR® Content Items of <atom:category> FHIR_Bundle, the <atom:entry> element MAY include a single <ncts:bundleInterpretation> element containing the value batch or collection.

    In lieu of specific client side instructions, the client SHALL treat the Bundle

        as one or more individual resources for loading or processing if the <ncts:bundleInterpretation> value is batch.
        as a single resource for loading or processing (as per a syndication entry for any other FHIR resource type) if the <ncts:bundleInterpretation> is not present or has the value collection.

NCTS Atom Extension

    CP 101

    The version of the NCTS Atom Extension described in this specification SHALL be 1.0.0.

Note

The NCTS Atom Syndication Format Extension XML Schema is versioned according to the rules of Semantic Versioning 2.0.0.

    CP 102

    The XML namespace for the NCTS Atom Syndication Format Extension XML Schema SHALL be:

    http://ns.electronichealth.net.au/ncts/syndication/asf/extensions/1.0.0

    The namespace identifier used to refer to this namespace within this specification is "ncts".

NCTS Atom Category Scheme

    CP 103

    The version of the NCTS Atom Category Scheme described in this specification SHALL be 1.0.0.

Note

The NCTS Atom Category Scheme is versioned according to the rules of Semantic Versioning 2.0.0.

    CP 104

    The URI associated with this version of the NCTS Atom Category Scheme SHALL be:

    http://ns.electronichealth.net.au/ncts/syndication/asf/scheme/1.0.0

    CP 105

    An <atom:category> element used to describe an NCTS Content Item MUST have a term attribute that contains the corresponding value defined within Table 3.

    CP 106

    An <atom:category> element used to describe an NCTS Content Item MUST have a label attribute that contains the corresponding value defined within Table 3.

Table 3: Atom Category values for NCTS Content Items NCTS Content Type Distribution Format Atom Category term value Atom Category label value
SNOMED CT Delta SCT*RF2_DELTA SNOMED CT RF2 Delta
Snapshot SCT_RF2_SNAPSHOT SNOMED CT RF2 Snapshot
Full SCT_RF2_FULL SNOMED CT RF2 Full
All SCT_RF2_ALL SNOMED CT RF2 All
LOINC NCTS LOINC LOINC LOINC
FHIR resource XML/JSON in link content-type FHIR*[ResourceType] (e.g. FHIR_CodeSystem) FHIR [ResourceType] (e.g. FHIR CodeSystem)
FHIR package Tarball (gzip compressed tar file) FHIR_Package FHIR Package
AMT Flat File Comma Separated Values file AMT_CSV Australian Medicines Terminology, Comma Separated Values

## https://www.healthterminologies.gov.au/specs/v3/national-services/nts/

National Terminology Server

The NCTS National Terminology Server (NTS) is a terminology server operated at the national level. The NTS provides access to national terminology content through the NCTS FHIR® API, which is based on the HL7 FHIR standard.
Terminology Content

The NTS contains both licensed and "open access" terminology content.

Licensed content, such as SNOMED CT-AU, can not be accessed without authentication. See API Security for the details on how to authenticate with the NTS.

Open access terminology content can be accessed by anonymous clients.

Note that requests for licensed content from anonymous users will be met with a 404 Not Found response, rather than 403 Unauthorized.
FHIR API

The NTS implements and provides a subset of version 4.0.0 of the NCTS FHIR API. This API is scoped down to read-only use with the terminology content available on the NTS.
Endpoint

The base endpoint URL for the NTS FHIR API is:

https://api.healthterminologies.gov.au/integration/R4/fhir

The version number within the API endpoint corresponds to the major version component of the NCTS FHIR API version. Moving between major versions of the API (which will be potentially incompatible with each other) will require an explicit change to the endpoint URL used by clients.
Operations

The subset of NCTS FHIR API operations that are supported by the NTS is as follows:
Table 1: FHIR API operations FHIR operation FHIR specification URL Use case
RESTful API Operations
capabilities http://hl7.org/fhir/R4/http.html#capabilities Get capabilities of a CTSA
search http://hl7.org/fhir/R4/http.html#search Search Content Items
read http://hl7.org/fhir/R4/http.html#read Get current version of a _Content Item_
batch http://hl7.org/fhir/R4/http.html#transaction Submit a set of operations within a single request
Resource Operations
validate http://hl7.org/fhir/R4/resource-operations.html#validate Validate a _Content Item_
CodeSystem Operations
lookup http://hl7.org/fhir/R4/codesystem-operations.html#lookup Get details of a concept within a CodeSystem
subsumes http://hl7.org/fhir/R4/codesystem-operations.html#subsumes Test subsumption relationship between two codes
ValueSet Operations
expand http://hl7.org/fhir/R4/valueset-operations.html#expand Expand:

    Composed Value Set

validate-code http://hl7.org/fhir/R4/valueset-operations.html#validate-code

Validate that a coded value is in the set of codes allowed by a ValueSet
Validate that a coded value is in the set of codes allowed by a ValueSet
ConceptMap Operations
translate http://hl7.org/fhir/R4/conceptmap-operations.html#translate Translate a code from one ValueSet to another
closure http://hl7.org/fhir/R4/conceptmap-operations.html#closure Maintain a client-side transitive closure
Terminology Service Operations
expand (Implicit) http://hl7.org/fhir/R4/snomedct.html#implicit
http://hl7.org/fhir/R4/loinc.html#implicit Expand:

    SNOMED CT implicit ValueSet
    LOINC implicit ValueSet
    Complete Code System implicit ValueSet

translate (Implicit) https://www.hl7.org/fhir/R4/snomedct.html#implicit-cm Translate a code from one SNOMED CT implicit ValueSet to another
"capabilities" operation

The NTS supports the use of the "capabilities" operation, providing a CapabilityStatement resource that describes the capabilities of the system as specified in the NCTS FHIR API specification.

Where there are any differences with the content of the CapabilityStatement resource, this specification serves as the source of truth for declaring officially supported functionality within the NTS.
"search" operation

The NTS supports the use of the "search" operation on CodeSystem resources, as specified in the NCTS FHIR API specification. The following search parameters are supported on this operation (supported modifiers in parentheses):

    _id (contains, exact, missing)
    description (contains, exact, missing)
    identifier
    name (contains, exact, missing)
    url (above, below, missing)
    _summary

The "search" operation may be used determine which versions of SNOMED CT‑AU are currently available in the NTS.
"read" operation

The NTS supports retrieval of CodeSystem resources, using the "read" operation as specified in the NCTS FHIR API specification.
Implicit "expand" operation

The NTS supports the use of the "expand" operation on implicit ValueSets within SNOMED CT‑AU releases, as specified in the NCTS FHIR API specification. The following request parameters are supported on this operation:

    identifier
    filter
    offset
    count

"lookup" operation

The NTS FHIR API implements the "lookup" operation for codes within SNOMED CT‑AU, as specified in the NCTS FHIR API specification. The following table describes the supported input parameters, and the information returned in the response.

## https://www.healthterminologies.gov.au/specs/v3/national-services/nss/

National Syndication Server

The NCTS National Syndication Server (NSS) is a deployment of an NCTS Conformant Syndication Server Application (CSSA) operated by the NCTS System Operator to provide access to national terminology content products for syndication.
Terminology Content

The NCTS Content Items available in the NSS are as follows:
Table 5: NSS terminology content Content Item SNOMED CT, Australian extension (SCT-AU) NCTS FHIR Bundle AMT flat file
NCTS Content Type SNOMED CT FHIR Bundle AMT flat file
Content Item Identifier http://snomed.info/sct/32506021000036107 https://healthterminologies.gov.au/fhir/Bundle/fhir-resource-bundle http://amt.info/csv
Distribution Formats

    RF2 Delta
    RF2 Snapshot
    RF2 Full
    RF2 All
    CSIRO Ontoserver binary



    FHIR XML
    FHIR JSON

    Comma Separated Values file

Content Retention 6 months of releases 6 months of releases 6 months of releases

CSIRO Ontoserver binary files are pre-indexed versions of the SNOMED CT‑AU terminology releases that can be used by other Ontoserver instances, as an alternative to the RF2 format.
Note

The AMT Flat File content is being released to assist some specific development activities. It will only be provided for a limited period and any use of it is contingent on the licensee acknowledging that it may be withdrawn with only 90 days’ notice.

Any licensee needing to use this file should first contact the NCTS at help@digitalhealth.gov.au for further information.
Syndication API

The NSS provides an implementation of version 1.0.0 of the NCTS Syndication API.
Endpoint

The Atom Collection URL for the NSS Syndication API is:

https://api.healthterminologies.gov.au/syndication/v1/syndication.xml

The version number within the API endpoint corresponds to the major version component of the NCTS Syndication API version. Moving between major versions of the API (which will be potentially incompatible with each other) will require an explicit change to the endpoint URL used by clients.
File Naming Convention

The file names of the SNOMED CT Distribution Format ZIP files available from the NSS conform to the following file naming convention:

NCTS*SCT_RF2_DISTRIBUTION*<SCT Module ID>-<timestamp>-<Type>.zip

Where:

    <SCT Module ID> refers to the SNOMED CT module ID of the SNOMED CT terminology.
    <timestamp> refers to the YYYYMMDD timestamp version of the SNOMED CT terminology.
    <Type> is one of DELTA, SNAPSHOT, FULL or ALL, depending on the Distribution Format.

## https://www.healthterminologies.gov.au/specs/v3/national-services/api-security/

API Security

Both the National Terminology Server and the National Syndication Server share the same requirements regarding encryption and authentication for incoming requests.

There is a Postman collection (and corresponding environment file) that demonstrates the relevant authentication operations on the NTS and NSS:

NCTS Postman Collection

NCTS (R4) Postman Environment
System Credentials

Connecting applications must use a System Credential generated within the "clients" menu of the NCTS Portal. Upon creation of a System Credential, the user will be provided with a Client ID and Client Secret.
Access Tokens

The NCTS exposes a token endpoint that implements the Client Credentials grant type defined within RFC 6749. The token endpoint is available at the following URL:

https://api.healthterminologies.gov.au/oauth2/token
Client Credentials Grant

POST /oauth2/token

Two methods of authenticating the client application are supported:

    HTTP Basic Authentication, and;
    Client ID and Client Secret passed within the request body.

Using Basic Authentication

Client ID and Client Secret are passed within the Authorization header, used as the username and password respectively, according to the scheme described in RFC 2617.

The request uses the content type application/x-www-form-urlencoded, and provides the following data in the request body:
Attribute Data type Description
grant_type string Must be set to client_credentials
Example

POST /oauth2/token HTTP/1.1
Host: api.healthterminologies.gov.au
Authorization: Basic eW91cmNsaWVudGlkOnlvdXJjbGllbnRzZWNyZXQ=
Content-Type: application/x-www-form-urlencoded

grant_type=client_credentials

Passing Client Credentials in the Request Body

An Authorization header is not provided when using this method.

The request uses the content type application/x-www-form-urlencoded, and provides the following data in the request body:
Attribute Data type Description
grant_type string Must be set to client_credentials
client_id string The Client ID generated by the NCTS Portal
client_secret string The Client Secret generated by the NCTS Portal
Example

POST /oauth2/token HTTP/1.1
Host: api.healthterminologies.gov.au
Content-Type: application/x-www-form-urlencoded

grant_type=client_credentials&client_id=yourclientid&client_secret=yourclientsecret

Response (JSON Payload)
Attribute Data type Description
access_token string The access token, for use in authenticating subsequent requests
token_type string Will always be equal to bearer
expires_in integer Number of seconds until expiry of this access token
Example

{
"token_type": "Bearer",
"expires_in": "3600",
"access_token": "youraccesstoken"
}

Requesting Protected Resources

All requests to protected resources must be authenticated with a valid access token, using the Bearer token scheme described in RFC 6750.

The NCTS National Services support the Authorization Request Header Field method defined within the Bearer token scheme. The Form-Encoded Body Parameter and URI Query Parameter methods are not allowed.
