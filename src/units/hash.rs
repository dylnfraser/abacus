use crate::units::value::Value;
use std::collections::HashMap;

/// A map of named key-value pairs representing structured calculation outputs (e.g. regression model results, inference test results).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hash {
    pub values: HashMap<String, Value>,
}

// Implementation functions for Hash
impl Hash {
    // Default constructor for the Hash
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Builds a `Hash` from an existing map (also available via `From<HashMap<String, Value>>`).
    #[must_use]
    pub fn from_map(values: HashMap<String, Value>) -> Self {
        Self { values }
    }

    pub fn insert(&mut self, key: impl Into<String>, value: Value) {
        self.values.insert(key.into(), value);
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    pub fn simplify_unit_display(
        &mut self,
        registry: &crate::registry::unit_registry::UnitRegistry,
    ) {
        for v in self.values.values_mut() {
            v.simplify_unit_display(registry);
        }
    }

    #[must_use]
    pub fn to_display(&self) -> String {
        self.to_string()
    }

    /// Returns a new `Hash` with all numeric values rounded to `sig_figs` significant figures.
    #[must_use]
    pub fn round_to_sig_figs(&self, sig_figs: usize) -> Self {
        let mut new_values = HashMap::new();
        for (k, v) in &self.values {
            new_values.insert(k.clone(), v.round_to_sig_figs(sig_figs));
        }
        Self { values: new_values }
    }

    /// Formats all entries in the hash formatted to `sig_figs` significant figures.
    #[must_use]
    pub fn to_display_with_sig_figs(&self, sig_figs: usize) -> String {
        self.render_with(|v| v.to_display_with_sig_figs(sig_figs))
    }

    /// Returns a new `Hash` with all numeric values rounded to `decimals` decimal places.
    #[must_use]
    pub fn round_to_decimals(&self, decimals: usize) -> Self {
        let mut new_values = HashMap::new();
        for (k, v) in &self.values {
            new_values.insert(k.clone(), v.round_to_decimals(decimals));
        }
        Self { values: new_values }
    }

    /// Returns a new `Hash` with display overrides applied to each contained `Value`.
    #[must_use]
    pub fn with_display_override(&self, overrides: &HashMap<String, String>) -> Self {
        let mut new_values = HashMap::with_capacity(self.values.len());
        for (k, v) in &self.values {
            new_values.insert(k.clone(), v.with_display_override(overrides));
        }
        Self { values: new_values }
    }

    /// Formats all entries in the hash formatted to `decimals` decimal places.
    #[must_use]
    pub fn to_display_with_decimals(&self, decimals: usize) -> String {
        self.render_with(|v| v.to_display_with_decimals(decimals))
    }

    /// Formats all entries in scientific notation.
    #[must_use]
    pub fn to_display_scientific(&self) -> String {
        self.render_with(Value::to_display_scientific)
    }

    /// Formats all entries in engineering notation.
    #[must_use]
    pub fn to_display_engineering(&self) -> String {
        self.render_with(Value::to_display_engineering)
    }

    fn render_with(&self, f: impl Fn(&Value) -> String) -> String {
        let mut entries: Vec<_> = self.values.iter().collect();
        entries.sort_by_key(|&(k, _)| k);
        let formatted: Vec<String> = entries
            .into_iter()
            .map(|(k, v)| format!("{k}: {}", f(v)))
            .collect();
        format!("{{ {} }}", formatted.join(", "))
    }
}

impl From<HashMap<String, Value>> for Hash {
    fn from(values: HashMap<String, Value>) -> Self {
        Self { values }
    }
}

impl Default for Hash {
    fn default() -> Self {
        Self::new()
    }
}

/// Display implementation for `Hash`
/// Formats the `Hash` as a human-readable string
impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{{ ")?;
        let mut entries: Vec<_> = self.values.iter().collect();
        entries.sort_by_key(|&(k, _)| k);
        for (i, (k, v)) in entries.into_iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{k}: {v}")?;
        }
        write!(f, " }}")
    }
}
