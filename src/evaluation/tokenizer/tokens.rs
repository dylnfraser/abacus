use crate::{Date, Value};

// Represents a token parsed from the input string
#[derive(Debug, Clone, PartialEq)]
pub enum Token<'a> {
    // Operators
    BinaryOp(&'static str), // e.g., '+', '-', '*', '/'
    UnaryOp(&'static str),  // e.g., 'sqrt' for square root or '!' for factorial
    Function(&'static str), // e.g., 'sin', 'binompdf', 'mean'

    ConversionOp,
    RelTimeOp(&'static str),  // "ago", "from_now", "before", "after"
    PercentChangeFrom,        // "% change from", "percent change from"
    PercentTag(&'static str), // "tip", "tax", "vat", "gst", "fee", "discount", "off", "markup"

    // Values
    Float(f64),     // parsed number
    Unit(&'a str),  // "kg", "m", "ft" borrowed directly from input text
    Ident(&'a str), // variable or constant identifier, e.g. "x", "pi", "e"

    // Grouping & Delimiters
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
    Range,                // `..` for range expressions inside functions, e.g. mean(1..10)
    DotProperty(&'a str), // `.intercept`, `.slope`, `.r2` etc.

    Val(Value), // final value
    Date(Date), // parsed date
}

/// A byte range of the source expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// The smallest span covering both `self` and `other`.
    #[must_use]
    pub fn to(self, other: Span) -> Span {
        Span::new(self.start.min(other.start), self.end.max(other.end))
    }

    /// The source text this span covers (empty if it doesn't fit `source`).
    #[must_use]
    pub fn text(self, source: &str) -> &str {
        source.get(self.start..self.end).unwrap_or("")
    }
}

/// Renders a token roughly as it was written, for error messages.
impl std::fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BinaryOp(s) | Self::UnaryOp(s) | Self::Function(s) | Self::PercentTag(s) => {
                f.write_str(s)
            }
            Self::RelTimeOp(s) => f.write_str(&s.replace('_', " ")),
            Self::ConversionOp => f.write_str("in"),
            Self::PercentChangeFrom => f.write_str("% change from"),
            Self::Float(n) => write!(f, "{n}"),
            Self::Unit(s) | Self::Ident(s) => f.write_str(s),
            Self::OpenParen => f.write_str("("),
            Self::CloseParen => f.write_str(")"),
            Self::OpenBracket => f.write_str("["),
            Self::CloseBracket => f.write_str("]"),
            Self::Comma => f.write_str(","),
            Self::Range => f.write_str(".."),
            Self::DotProperty(s) => write!(f, ".{s}"),
            Self::Val(v) => write!(f, "{v}"),
            Self::Date(d) => write!(f, "{d}"),
        }
    }
}
