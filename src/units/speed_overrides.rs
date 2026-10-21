use std::collections::HashMap;

/// Returns standard unit display overrides for speed expressions
/// (e.g. `"mi/h"` -> `"mph"`, `"km/h"` -> `"kmph"`). Keys match the rendered unit
/// display, which never contains spaces.
#[must_use]
pub fn standard_speed_overrides() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("mi/h".to_string(), "mph".to_string());
    m.insert("km/h".to_string(), "kmph".to_string());
    m
}
