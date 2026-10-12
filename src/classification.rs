//! Classification of xmllint results.
//!
//! Distinguishing the phases avoids blaming the XML when the problem is in the
//! XSD or in xmllint failing to run at all. All user-visible strings are resolved
//! through i18n from the keys returned by [`Phase::title_key`] and
//! [`Phase::explanation_key`].

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The XML is well formed but violates the schema.
    SchemaViolation,
    /// The XSD has its own errors and could not be compiled.
    SchemaCompileError,
    /// The XSD file could not be opened or located.
    SchemaNotFound,
    /// The document is not well-formed XML.
    MalformedXml,
    /// Environment failure: missing binary, bad arguments, timeout.
    Environment,
}

impl Phase {
    pub fn title_key(self) -> &'static str {
        match self {
            Phase::SchemaViolation => "phase.schema_violation.title",
            Phase::SchemaCompileError => "phase.schema_compile_error.title",
            Phase::SchemaNotFound => "phase.schema_not_found.title",
            Phase::MalformedXml => "phase.malformed_xml.title",
            Phase::Environment => "phase.environment.title",
        }
    }

    pub fn explanation_key(self) -> &'static str {
        match self {
            Phase::SchemaViolation => "phase.schema_violation.explanation",
            Phase::SchemaCompileError => "phase.schema_compile_error.explanation",
            Phase::SchemaNotFound => "phase.schema_not_found.explanation",
            Phase::MalformedXml => "phase.malformed_xml.explanation",
            Phase::Environment => "phase.environment.explanation",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Detail {
    Timeout { seconds: u64 },
    Spawn { error: String },
    MissingXml,
    MissingXsd,
    MissingXmllint,
    ThreadLost,
}

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub phase: Phase,
    pub success: bool,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub detail: Option<Detail>,
}

impl ValidationResult {
    pub fn error_count(&self) -> usize {
        count_errors(&self.stderr)
    }
}

pub fn count_errors(stderr: &str) -> usize {
    stderr
        .lines()
        .filter(|l| l.to_ascii_lowercase().contains("error"))
        .count()
}

/// Determines from xmllint's stderr which phase failed.
///
/// Order matters: schema and environment problems are checked before document
/// problems, because their messages share vocabulary with the rest.
pub fn classify(exit_code: Option<i32>, stderr: &str) -> Phase {
    let s = stderr.to_ascii_lowercase();

    if s.contains("failed to locate the main schema") {
        return Phase::SchemaNotFound;
    }

    if s.contains("schemas parser error")
        || s.contains("failed to parse the xml resource")
        || s.contains("xmlschemascompilationcontext")
    {
        return Phase::SchemaCompileError;
    }

    if s.contains("start tag expected")
        || s.contains("premature end of data in tag")
        || s.contains("opening and ending tag mismatch")
        || s.contains("extra content at the end of the document")
        || s.contains("couldn't find end of start tag")
        || s.contains("document is empty")
        || s.contains("not well-formed")
        || s.contains("comment may not contain")
        || s.contains("sequence ']]>' not allowed")
        || s.contains("xml declaration allowed only")
    {
        return Phase::MalformedXml;
    }

    if s.contains("schemas validity error") || s.contains("fails to validate") {
        return Phase::SchemaViolation;
    }

    if s.contains("parser error") {
        return Phase::MalformedXml;
    }

    if s.contains("no such file or directory")
        || s.contains("failed to load")
        || s.contains("failed to open")
        || s.contains("usage :")
    {
        return Phase::Environment;
    }

    match exit_code {
        Some(0) | Some(1) | Some(4) => Phase::SchemaViolation,
        Some(3) => Phase::SchemaCompileError,
        _ => Phase::Environment,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real xmllint (libxml2) output captured for each scenario.

    #[test]
    fn schema_violation() {
        let stderr = "invalido.xml:2: element precio: Schemas validity error : Element 'precio': 'abc' is not a valid value of the atomic type 'xs:decimal'.\ninvalido.xml fails to validate\n";
        assert_eq!(classify(Some(3), stderr), Phase::SchemaViolation);
        assert_eq!(count_errors(stderr), 1);
    }

    #[test]
    fn schema_compile_error() {
        // A malformed XSD produces "parser error" lines from the XSD itself, yet
        // the correct phase is still SchemaCompileError: the XML was never validated.
        let stderr = "xsd_roto.xsd:8: parser error : Opening and ending tag mismatch: complexType line 4 and element\n    </xs:element>\n                 ^\nxsd_roto.xsd:9: parser error : Opening and ending tag mismatch: element line 3 and schema\n</xs:schema>\n            ^\nxsd_roto.xsd:10: parser error : Premature end of data in tag schema line 2\n\n^\nSchemas parser error : Failed to parse the XML resource 'xsd_roto.xsd'.\nWXS schema xsd_roto.xsd failed to compile\n";
        assert_eq!(classify(Some(5), stderr), Phase::SchemaCompileError);
    }

    #[test]
    fn schema_not_found() {
        // "Schemas parser error" is present too; SchemaNotFound still wins.
        let stderr = "warning: failed to load external entity \"falta.xsd\"\nSchemas parser error : Failed to locate the main schema resource at 'falta.xsd'.\nWXS schema falta.xsd failed to compile\n";
        assert_eq!(classify(Some(5), stderr), Phase::SchemaNotFound);
    }

    #[test]
    fn malformed_xml() {
        let stderr = "roto.xml:2: parser error : Opening and ending tag mismatch: nombre line 2 and precio\n<producto><nombre>Teclado</precio>\n                                  ^\nroto.xml:3: parser error : Premature end of data in tag producto line 2\n\n^\n";
        assert_eq!(classify(Some(1), stderr), Phase::MalformedXml);
    }

    #[test]
    fn inaccessible_input_xml() {
        let stderr = "warning: failed to load external entity \"falta.xml\"\n";
        assert_eq!(classify(Some(1), stderr), Phase::Environment);
    }

    #[test]
    fn invalid_arguments() {
        let stderr = "Usage : xmllint [options] XMLfile ...\n";
        assert_eq!(classify(Some(2), stderr), Phase::Environment);
    }

    #[test]
    fn fallback_by_exit_code() {
        assert_eq!(
            classify(Some(4), "something unexpected"),
            Phase::SchemaViolation
        );
        assert_eq!(
            classify(Some(2), "something unexpected"),
            Phase::Environment
        );
    }

    #[test]
    fn error_counting() {
        let stderr = "doc.xml:3: element precio: Schemas validity error : ...\ndoc.xml:5: element total: Schemas validity error : ...\ndoc.xml fails to validate\n";
        assert_eq!(count_errors(stderr), 2);
        assert_eq!(count_errors(""), 0);
    }
}
