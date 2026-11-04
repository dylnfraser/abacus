use std::collections::HashMap;

/// Display overrides for speed units (`"mi/h"` -> `"mph"`). Keys match the rendered unit,
/// which never contains spaces.
#[must_use]
pub fn standard_speed_overrides() -> HashMap<String, String> {
    let mut m = HashMap::new();
    m.insert("mi/h".to_string(), "mph".to_string());
    m.insert("km/h".to_string(), "kmph".to_string());
    m
}
