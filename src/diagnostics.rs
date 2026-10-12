//! Model of a single schema violation.
//!
//! The app used to show xmllint's stderr as one text blob. Here each error becomes
//! structured data, so it can be located, translated, and filtered. This module is
//! not wired into the UI yet; it backs a future structured-errors pass.

use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    #[default]
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn icon(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Severity::Error => "Error",
            Severity::Warning => "Warning",
            Severity::Info => "Information",
        }
    }
}

/// Which stage failed. Telling them apart is what keeps the app from blaming the
/// XML when the problem is in the XSD or in the XML not being XML at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    #[default]
    Validity,
    InvalidSchema,
    MalformedXml,
    Environment,
}

impl Phase {
    /// When combining diagnostics, the earliest phase wins: if the XML is not well
    /// formed, nothing the schema says is relevant yet.
    pub fn priority(self) -> u8 {
        match self {
            Phase::Validity => 0,
            Phase::InvalidSchema => 1,
            Phase::MalformedXml => 2,
            Phase::Environment => 3,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Phase::Validity => "THE XML DOES NOT COMPLY WITH THE SCHEMA",
            Phase::InvalidSchema => "THE XSD COULD NOT BE COMPILED",
            Phase::MalformedXml => "THE XML IS NOT WELL FORMED",
            Phase::Environment => "THE VALIDATION COULD NOT RUN",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            Phase::Validity => "The XML is valid XML and the schema compiled. The errors below are schema violations.",
            Phase::InvalidSchema => "The schema has its own errors, so the XML was never validated. Fix the XSD first.",
            Phase::MalformedXml => "The document is not well-formed XML, so it was never validated against the schema. This says nothing about the XSD.",
            Phase::Environment => "The problem is with the environment, not with your files.",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Diagnostic {
    pub severity: Severity,
    pub phase: Phase,
    /// Which file libxml2 points at. It may be the XSD, not always the XML.
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// Local name of the element, without the `{namespace}` libxml2 prefixes.
    pub element: Option<String>,
    pub namespace: Option<String>,
    pub attribute: Option<String>,
    /// Path reconstructed by us, e.g. `Invoice/Header/Issuer/TaxId`.
    pub field_path: Option<String>,
    /// `pattern`, `maxLength`, `enumeration`, `minInclusive`...
    pub facet: Option<String>,
    /// The expected XSD type (`xs:decimal`, `xs:date`...).
    pub value_type: Option<String>,
    /// The value that triggered the error, extracted from the message.
    pub value: Option<String>,
    /// Names libxml2 lists as admissible in that position.
    pub expected: Vec<String>,
    pub message: String,
    pub suggestion: Option<String>,
    /// libxml2's original text. Never discarded.
    pub original_message: String,
    /// The line of the file exactly as it is on disk.
    pub fragment: Option<String>,
}

impl Diagnostic {
    /// The field the error points at, in the most specific form available.
    pub fn field(&self) -> Option<String> {
        match (&self.field_path, &self.element, &self.attribute) {
            (Some(path), _, Some(attr)) => Some(format!("{path}/@{attr}")),
            (Some(path), _, None) => Some(path.clone()),
            (None, Some(el), Some(attr)) => Some(format!("{el}/@{attr}")),
            (None, Some(el), None) => Some(el.clone()),
            (None, None, Some(attr)) => Some(format!("@{attr}")),
            (None, None, None) => None,
        }
    }

    /// Title of a row in the listing: `error line 12:9 - Header/Issuer/TaxId`
    pub fn title(&self) -> String {
        let location = match (self.line, self.column) {
            (Some(l), Some(c)) => format!("line {l}:{c}"),
            (Some(l), None) => format!("line {l}"),
            _ => "no location".to_string(),
        };
        match self.field() {
            Some(field) => format!("{} {location} - {field}", self.severity.icon()),
            None => format!("{} {location}", self.severity.icon()),
        }
    }

    /// Single-paragraph text for the row's "Copy" button.
    pub fn plain_text(&self) -> String {
        let mut t = String::new();
        t.push_str(self.severity.label());
        if let Some(l) = self.line {
            t.push_str(&format!(" at line {l}"));
            if let Some(c) = self.column {
                t.push_str(&format!(", column {c}"));
            }
        }
        t.push('\n');
        if let Some(field) = self.field() {
            t.push_str(&format!("Field: {field}\n"));
        }
        t.push_str(&format!("Problem: {}\n", self.message));
        if let Some(v) = &self.value {
            t.push_str(&format!("Value found: {v}\n"));
        }
        if !self.expected.is_empty() {
            t.push_str(&format!("Expected: {}\n", self.expected.join(", ")));
        }
        if let Some(s) = &self.suggestion {
            t.push_str(&format!("Suggestion: {s}\n"));
        }
        if let Some(f) = &self.fragment {
            t.push_str(&format!("XML: {}\n", f.trim()));
        }
        t.push_str(&format!("Original message: {}\n", self.original_message));
        t
    }

    /// Errors where libxml2 abandons the review of that subtree, so there may be
    /// more problems hiding behind this one.
    pub fn is_structural(&self) -> bool {
        let m = &self.original_message;
        m.contains("This element is not expected")
            || m.contains("Missing child element")
            || m.contains("No matching global declaration")
    }
}

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub phase: Phase,
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
    /// xmllint's exit code. Stored as supporting data, not for classification.
    pub exit_code: Option<i32>,
    pub duration: Duration,
    pub xml: Option<PathBuf>,
    pub xsd: Option<PathBuf>,
}

impl ValidationReport {
    pub fn count(&self, severity: Severity) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    }

    /// If there are structural errors, fixing them may reveal others libxml2 had
    /// not reached yet.
    pub fn has_structural_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_structural())
    }

    pub fn summary(&self) -> String {
        if self.valid {
            return format!(
                "No errors. Validated in {:.2} s.",
                self.duration.as_secs_f32()
            );
        }
        let errors = self.count(Severity::Error);
        let warnings = self.count(Severity::Warning);
        let mut parts = Vec::new();
        parts.push(match errors {
            1 => "1 error".to_string(),
            n => format!("{n} errors"),
        });
        if warnings > 0 {
            parts.push(match warnings {
                1 => "1 warning".to_string(),
                n => format!("{n} warnings"),
            });
        }
        format!(
            "{} - {:.2} s",
            parts.join(", "),
            self.duration.as_secs_f32()
        )
    }
}
