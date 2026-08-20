//! Structured diagnostics, and how they render.
//!
//! Design §29 asks for a code, a span and a body. The body is the part that decides whether this is
//! useful: an agent consuming a diagnostic as a repair instruction needs the two types and the two
//! paths as fields, not as prose it has to parse back out. So [`Diagnostic`] *is* the structured
//! form and the text is a projection of it — never the reverse, which is how a message ends up
//! carrying information the machine-readable form lost.

use std::fmt;

use ess_domain::name::QualifiedName;

use crate::source::Span;

/// How bad it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Compilation cannot continue past this.
    Error,
    /// Legal, and probably not meant.
    Warning,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Error => "error",
            Self::Warning => "warning",
        })
    }
}

/// A stable diagnostic code, such as `ESS-BINDING-002`.
///
/// Stable because a harness matches on it. The number is not meaningful beyond being unique within
/// its family; the family is what a reader learns to recognise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize)]
#[serde(into = "String")]
pub struct Code {
    /// Which part of the model — `BINDING`, `TYPE`, `COMPONENT`.
    pub family: &'static str,
    /// Which rule within it.
    pub number: u16,
}

impl Code {
    /// Builds one.
    pub const fn new(family: &'static str, number: u16) -> Self {
        Self { family, number }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ESS-{}-{:03}", self.family, self.number)
    }
}

impl From<Code> for String {
    fn from(code: Code) -> Self {
        code.to_string()
    }
}

/// One fact a diagnostic reports, as a field rather than a sentence.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Detail {
    /// A declaration and the type it has or requires.
    Typed {
        /// What is being described, such as `billing.invoice.InvoiceCreated.customer_email`.
        subject: String,
        /// Its type, rendered.
        type_ref: String,
        /// Whether the subject *has* this type or *requires* it — the difference a reader needs.
        requires: bool,
    },
    /// A name that does not resolve, with what was available.
    Undeclared {
        /// The name.
        name: QualifiedName,
        /// What kind of thing was looked for.
        expected: &'static str,
        /// What is declared, for a reader who mistyped.
        available: Vec<String>,
    },
    /// Something worth saying that has no better shape yet.
    Note {
        /// The text.
        text: String,
    },
}

/// One problem, ready to be rendered or consumed.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Diagnostic {
    /// Its stable code.
    pub code: Code,
    /// How bad.
    pub severity: Severity,
    /// One line: what is wrong.
    pub message: String,
    /// The facts, in the order a reader should meet them.
    pub details: Vec<Detail>,
    /// How to fix it, when there is an obvious remedy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// Where it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// Every problem found, in a stable order.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(transparent)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    /// None yet.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.0.push(diagnostic);
    }

    /// Absorbs another set.
    pub fn extend(&mut self, other: Self) {
        self.0.extend(other.0);
    }

    /// Every diagnostic.
    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.0
    }

    /// How many.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// `true` when there are none.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// `true` when at least one prevents compilation.
    pub fn has_errors(&self) -> bool {
        self.0
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    /// `true` when any diagnostic carries this code.
    pub fn contains(&self, code: Code) -> bool {
        self.0.iter().any(|diagnostic| diagnostic.code == code)
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.0.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{diagnostic}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}[{}]: {}", self.severity, self.code, self.message)?;
        for detail in &self.details {
            match detail {
                Detail::Typed {
                    subject,
                    type_ref,
                    requires,
                } => {
                    let verb = if *requires { "requires" } else { "has type" };
                    writeln!(f, "  {subject}\n      {verb} `{type_ref}`")?;
                }
                Detail::Undeclared {
                    name,
                    expected,
                    available,
                } => {
                    writeln!(f, "  `{name}` is not a declared {expected}")?;
                    if !available.is_empty() {
                        writeln!(f, "      declared: {}", available.join(", "))?;
                    }
                }
                Detail::Note { text } => writeln!(f, "  {text}")?,
            }
        }
        if let Some(hint) = &self.hint {
            writeln!(f, "  help: {hint}")?;
        }
        if let Some(span) = &self.span {
            write!(f, "  --> {span}")?;
        }
        Ok(())
    }
}
