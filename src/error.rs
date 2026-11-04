use std::fmt;

// Error enum for evaluation/tokenization errors
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AbacusError {
    UnknownUnit(String),
    /// A name followed by `(` that is neither a function nor a unit.
    UnknownFunction(String),
    /// A character that cannot start any token.
    UnexpectedCharacter(char),
    IncompatibleDimensions,
    IncompatibleFunctionArguments,
    AffineUnitOperation(&'static str),
    UnexpectedToken(String),
    UnexpectedEnd,
    UnclosedParen,
    UnclosedBracket,
    IntervalInFunction,
    InvalidDate(String),
    InvalidNumber(String),
    IncompatibleOperatorType(String),
    IncompatibleWithConversion(String),
    RecursionLimitExceeded,
    ExponentLimitExceeded,
    UndefinedVariable(String),
    EvaluationError(String),
    CurrencyRateError(String),
}

// Display implementation for `AbacusError`
impl fmt::Display for AbacusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnit(unit) => write!(f, "unknown unit: {unit}"),
            Self::UnknownFunction(name) => write!(f, "unknown function: {name}"),
            Self::UnexpectedCharacter(c) => write!(f, "unexpected character: '{c}'"),
            Self::IncompatibleDimensions => {
                write!(f, "cannot perform operation on incompatible dimensions")
            }
            Self::AffineUnitOperation(op) => write!(f, "cannot {op} affine units"),
            Self::IncompatibleFunctionArguments => {
                write!(f, "incompatible function arguments")
            }
            Self::UnexpectedToken(tok) => write!(f, "unexpected token: {tok}"),
            Self::UnexpectedEnd => write!(f, "unexpected end of expression"),
            Self::UnclosedParen => write!(f, "unclosed parenthesis"),
            Self::UnclosedBracket => write!(f, "unclosed bracket in interval"),
            Self::IntervalInFunction => {
                write!(f, "interval values cannot be passed as function arguments")
            }
            Self::InvalidDate(msg) => write!(f, "invalid date: {msg}"),
            Self::InvalidNumber(num) => write!(f, "invalid number format: {num}"),
            Self::IncompatibleOperatorType(msg) => write!(f, "{msg}"),
            Self::IncompatibleWithConversion(ty) => {
                write!(f, "values of type {ty} cannot be converted")
            }
            Self::RecursionLimitExceeded => {
                write!(f, "maximum recursion depth exceeded")
            }
            Self::ExponentLimitExceeded => {
                write!(f, "maximum exponent limit exceeded")
            }
            Self::UndefinedVariable(var) => {
                write!(f, "undefined variable: {var}")
            }
            Self::EvaluationError(msg) => {
                write!(f, "{msg}")
            }
            Self::CurrencyRateError(msg) => {
                write!(f, "currency rate error: {msg}")
            }
        }
    }
}

impl std::error::Error for AbacusError {}

/// An error together with where it occurred in the source expression.
///
/// Returned by [`crate::Abacus::eval_detailed`] and [`crate::Abacus::compile`]. Its `Display`
/// output points at the offending part of the expression:
///
/// ```text
/// cannot perform operation on incompatible dimensions
///   5 m + 3 s
///       ^
///   note: m + s
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    error: AbacusError,
    span: Option<crate::evaluation::tokenizer::tokens::Span>,
    source: String,
    note: Option<String>,
}

impl Diagnostic {
    pub(crate) fn new(
        error: AbacusError,
        span: Option<crate::evaluation::tokenizer::tokens::Span>,
        source: &str,
    ) -> Self {
        Self {
            error,
            span,
            source: source.to_string(),
            note: None,
        }
    }

    pub(crate) fn with_note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }

    /// The underlying error.
    #[must_use]
    pub fn error(&self) -> &AbacusError {
        &self.error
    }

    /// Consumes the diagnostic, returning the underlying error.
    #[must_use]
    pub fn into_error(self) -> AbacusError {
        self.error
    }

    /// Byte range of the source the error refers to, when known.
    #[must_use]
    pub fn span(&self) -> Option<crate::evaluation::tokenizer::tokens::Span> {
        self.span
    }

    /// The expression that was evaluated.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Extra context, such as the units of the two sides of a failed operation.
    #[must_use]
    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;
        // Carets only make sense for a single-line source.
        if let Some(span) = self.span
            && !self.source.contains('\n')
            && span.end <= self.source.len()
        {
            let lead = self.source[..span.start].chars().count();
            let width = self.source[span.start..span.end].chars().count().max(1);
            write!(
                f,
                "\n  {}\n  {}{}",
                self.source,
                " ".repeat(lead),
                "^".repeat(width)
            )?;
        }
        if let Some(note) = &self.note {
            write!(f, "\n  note: {note}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostic {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<Diagnostic> for AbacusError {
    fn from(d: Diagnostic) -> Self {
        d.error
    }
}
