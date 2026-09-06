use std::{collections::HashMap, sync::Arc};

use crate::{
    AbacusError,
    units::{
        dimensions::Dimensions,
        unit::{Unit, UnitExpr},
    },
};

pub struct CurrencyDefinition {
    pub code: &'static str,
    pub display: &'static str,
    pub aliases: &'static [&'static str],
    pub rate_per_usd: f64,
}

/// Fixed exchange rates against USD (base = 1.0 USD) captured from https://api.frankfurter.dev/v1/latest?base=USD on September 1, 2026.
pub const FIXED_EXCHANGE_RATES: &[CurrencyDefinition] = &[
    CurrencyDefinition {
        code: "USD",
        display: "$",
        aliases: &["USD", "usd", "dollar", "dollars", "$"],
        rate_per_usd: 1.0,
    },
    CurrencyDefinition {
        code: "EUR",
        display: "EUR",
        aliases: &["EUR", "eur", "euro", "euros", "€"],
        rate_per_usd: 0.86281,
    },
    CurrencyDefinition {
        code: "GBP",
        display: "GBP",
        aliases: &["GBP", "gbp", "pound", "pounds", "£"],
        rate_per_usd: 0.73904,
    },
    CurrencyDefinition {
        code: "JPY",
        display: "JPY",
        aliases: &["JPY", "jpy", "yen", "¥"],
        rate_per_usd: 160.16,
    },
    CurrencyDefinition {
        code: "CAD",
        display: "CAD",
        aliases: &["CAD", "cad"],
        rate_per_usd: 1.3888,
    },
    CurrencyDefinition {
        code: "AUD",
        display: "AUD",
        aliases: &["AUD", "aud"],
        rate_per_usd: 1.4003,
    },
    CurrencyDefinition {
        code: "CHF",
        display: "CHF",
        aliases: &["CHF", "chf"],
        rate_per_usd: 0.81053,
    },
    CurrencyDefinition {
        code: "CNY",
        display: "CNY",
        aliases: &["CNY", "cny", "yuan", "renminbi", "rmb", "RMB"],
        rate_per_usd: 6.7223,
    },
    CurrencyDefinition {
        code: "INR",
        display: "INR",
        aliases: &["INR", "inr", "rupee", "rupees", "₹"],
        rate_per_usd: 94.95,
    },
    CurrencyDefinition {
        code: "MXN",
        display: "MXN",
        aliases: &["MXN", "mxn", "peso", "pesos"],
        rate_per_usd: 17.0009,
    },
    CurrencyDefinition {
        code: "BRL",
        display: "BRL",
        aliases: &["BRL", "brl", "real", "reais", "R$"],
        rate_per_usd: 5.1989,
    },
    CurrencyDefinition {
        code: "SEK",
        display: "SEK",
        aliases: &["SEK", "sek", "krona", "kronor"],
        rate_per_usd: 9.5897,
    },
    CurrencyDefinition {
        code: "NOK",
        display: "NOK",
        aliases: &["NOK", "nok", "krone", "kroner"],
        rate_per_usd: 9.3343,
    },
    CurrencyDefinition {
        code: "NZD",
        display: "NZD",
        aliases: &["NZD", "nzd"],
        rate_per_usd: 1.6975,
    },
    CurrencyDefinition {
        code: "SGD",
        display: "SGD",
        aliases: &["SGD", "sgd"],
        rate_per_usd: 1.2734,
    },
    CurrencyDefinition {
        code: "HKD",
        display: "HKD",
        aliases: &["HKD", "hkd"],
        rate_per_usd: 7.841,
    },
    CurrencyDefinition {
        code: "KRW",
        display: "KRW",
        aliases: &["KRW", "krw", "won", "₩"],
        rate_per_usd: 1374.61,
    },
    CurrencyDefinition {
        code: "TRY",
        display: "TRY",
        aliases: &["TRY", "try", "lira", "₺"],
        rate_per_usd: 48.274,
    },
    CurrencyDefinition {
        code: "ZAR",
        display: "ZAR",
        aliases: &["ZAR", "zar", "rand"],
        rate_per_usd: 16.1558,
    },
    CurrencyDefinition {
        code: "PLN",
        display: "PLN",
        aliases: &["PLN", "pln", "zloty"],
        rate_per_usd: 3.7371,
    },
    CurrencyDefinition {
        code: "DKK",
        display: "DKK",
        aliases: &["DKK", "dkk"],
        rate_per_usd: 6.4494,
    },
    CurrencyDefinition {
        code: "CZK",
        display: "CZK",
        aliases: &["CZK", "czk"],
        rate_per_usd: 20.845,
    },
    CurrencyDefinition {
        code: "HUF",
        display: "HUF",
        aliases: &["HUF", "huf"],
        rate_per_usd: 316.4,
    },
    CurrencyDefinition {
        code: "ILS",
        display: "ILS",
        aliases: &["ILS", "ils", "shekel", "₪"],
        rate_per_usd: 3.0145,
    },
    CurrencyDefinition {
        code: "IDR",
        display: "IDR",
        aliases: &["IDR", "idr", "rupiah"],
        rate_per_usd: 17745.0,
    },
    CurrencyDefinition {
        code: "MYR",
        display: "MYR",
        aliases: &["MYR", "myr", "ringgit"],
        rate_per_usd: 4.0395,
    },
    CurrencyDefinition {
        code: "PHP",
        display: "PHP",
        aliases: &["PHP", "php"],
        rate_per_usd: 62.429,
    },
    CurrencyDefinition {
        code: "THB",
        display: "THB",
        aliases: &["THB", "thb", "baht", "฿"],
        rate_per_usd: 33.265,
    },
    CurrencyDefinition {
        code: "RON",
        display: "RON",
        aliases: &["RON", "ron", "leu"],
        rate_per_usd: 4.5355,
    },
    CurrencyDefinition {
        code: "ISK",
        display: "ISK",
        aliases: &["ISK", "isk"],
        rate_per_usd: 121.48,
    },
];

/// Registers all currency units with default fixed exchange rates into the provided map.
pub fn register_currency_units(map: &mut HashMap<String, Arc<Unit>>) {
    for def in FIXED_EXCHANGE_RATES {
        let scalar = 1.0 / def.rate_per_usd;
        let unit = Arc::new(Unit {
            scalar,
            offset: 0.0,
            dimensions: Dimensions::CURRENCY,
            display: UnitExpr::single(def.display),
        });
        for &alias in def.aliases {
            map.insert(alias.to_string(), Arc::clone(&unit));
        }
    }
}

/// Updates currency exchange rates in the provided unit map.
/// `rates` maps currency code (e.g. "EUR") to `rate_per_usd` (e.g. 0.86281).
pub fn update_currency_rates_in_map(
    map: &mut HashMap<String, Arc<Unit>>,
    rates: &HashMap<String, f64>,
) {
    for def in FIXED_EXCHANGE_RATES {
        if let Some(&rate_per_usd) = rates.get(def.code)
            && rate_per_usd > 0.0
        {
            let scalar = 1.0 / rate_per_usd;
            let unit = Arc::new(Unit {
                scalar,
                offset: 0.0,
                dimensions: Dimensions::CURRENCY,
                display: UnitExpr::single(def.display),
            });
            for &alias in def.aliases {
                map.insert(alias.to_string(), Arc::clone(&unit));
            }
        }
    }
}

/// Zero-dependency parser for Frankfurter JSON response (e.g. `{"base":"USD","rates":{"EUR":0.86281,...}}`).
pub fn parse_frankfurter_json(json: &str) -> Result<HashMap<String, f64>, AbacusError> {
    let mut rates = HashMap::new();
    rates.insert("USD".to_string(), 1.0);

    // Locate "rates" block
    let rates_idx = json.find("\"rates\"").ok_or_else(|| {
        AbacusError::EvaluationError("missing 'rates' field in frankfurter response".to_string())
    })?;
    let rest = &json[rates_idx..];
    let open_brace = rest.find('{').ok_or_else(|| {
        AbacusError::EvaluationError("malformed rates object in frankfurter response".to_string())
    })?;
    let close_brace = rest.find('}').ok_or_else(|| {
        AbacusError::EvaluationError("unclosed rates object in frankfurter response".to_string())
    })?;

    let inner = &rest[open_brace + 1..close_brace];
    for part in inner.split(',') {
        if let Some((k, v)) = part.split_once(':') {
            let code = k.trim().trim_matches('"');
            let val_str = v.trim().trim_matches('"');
            if let Ok(rate) = val_str.parse::<f64>() {
                rates.insert(code.to_string(), rate);
            }
        }
    }

    Ok(rates)
}

/// Fetches live exchange rates from https://api.frankfurter.dev/v1/latest?base=USD via `curl`.
/// Returns an error if offline, unreachable, or parsing fails.
#[cfg(feature = "live-rates")]
pub fn fetch_frankfurter_rates() -> Result<HashMap<String, f64>, AbacusError> {
    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg("--connect-timeout")
        .arg("4")
        .arg("--max-time")
        .arg("8")
        .arg("https://api.frankfurter.dev/v1/latest?base=USD")
        .output()
        .map_err(|e| {
            AbacusError::EvaluationError(format!("failed to execute curl for live rates: {e}"))
        })?;

    if !output.status.success() {
        return Err(AbacusError::EvaluationError(
            "curl exited with non-zero status when fetching live exchange rates".to_string(),
        ));
    }

    let body = String::from_utf8(output.stdout).map_err(|e| {
        AbacusError::EvaluationError(format!("invalid UTF-8 response from frankfurter: {e}"))
    })?;

    parse_frankfurter_json(&body)
}

#[cfg(not(feature = "live-rates"))]
pub fn fetch_frankfurter_rates() -> Result<HashMap<String, f64>, AbacusError> {
    Err(AbacusError::EvaluationError(
        "live-rates feature is not enabled".to_string(),
    ))
}

/// Returns the default path to the cached exchange rates file.
#[must_use]
pub fn default_currency_cache_path() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("ABACUS_CURRENCY_CACHE") {
        return std::path::PathBuf::from(custom);
    }
    if let Ok(cache_dir) = std::env::var("ABACUS_CACHE_DIR") {
        return std::path::PathBuf::from(cache_dir).join("currency_rates.json");
    }
    if let Ok(home) = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")) {
        return std::path::PathBuf::from(home)
            .join(".cache")
            .join("abacus")
            .join("currency_rates.json");
    }
    std::env::temp_dir().join("abacus_currency_rates.json")
}

/// Returns true if the cache file exists and was modified within the last 24 hours (86,400 seconds).
#[must_use]
pub fn is_cache_fresh(cache_path: &std::path::Path) -> bool {
    if let Ok(metadata) = std::fs::metadata(cache_path)
        && let Ok(modified) = metadata.modified()
        && let Ok(elapsed) = modified.elapsed()
    {
        return elapsed.as_secs() < 86_400;
    }
    false
}

/// Saves raw JSON to the specified cache path, creating parent directories if necessary.
pub fn save_rates_to_cache(
    json_str: &str,
    cache_path: &std::path::Path,
) -> Result<(), AbacusError> {
    if let Some(parent) = cache_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(cache_path, json_str)
        .map_err(|e| AbacusError::EvaluationError(format!("failed to write currency cache: {e}")))
}

/// Loads exchange rates from a cached JSON file.
pub fn load_rates_from_cache(
    cache_path: &std::path::Path,
) -> Result<HashMap<String, f64>, AbacusError> {
    let content = std::fs::read_to_string(cache_path)
        .map_err(|e| AbacusError::EvaluationError(format!("failed to read currency cache: {e}")))?;
    parse_frankfurter_json(&content)
}

/// Fetches live exchange rates and writes the raw response to the cache file.
#[cfg(feature = "live-rates")]
pub fn fetch_and_cache_rates(
    cache_path: &std::path::Path,
) -> Result<HashMap<String, f64>, AbacusError> {
    let output = std::process::Command::new("curl")
        .arg("-s")
        .arg("--connect-timeout")
        .arg("4")
        .arg("--max-time")
        .arg("8")
        .arg("https://api.frankfurter.dev/v1/latest?base=USD")
        .output()
        .map_err(|e| {
            AbacusError::EvaluationError(format!("failed to execute curl for live rates: {e}"))
        })?;

    if !output.status.success() {
        return Err(AbacusError::EvaluationError(
            "curl exited with non-zero status when fetching live exchange rates".to_string(),
        ));
    }

    let body = String::from_utf8(output.stdout).map_err(|e| {
        AbacusError::EvaluationError(format!("invalid UTF-8 response from frankfurter: {e}"))
    })?;

    let rates = parse_frankfurter_json(&body)?;
    let _ = save_rates_to_cache(&body, cache_path);
    Ok(rates)
}

#[cfg(not(feature = "live-rates"))]
pub fn fetch_and_cache_rates(
    _cache_path: &std::path::Path,
) -> Result<HashMap<String, f64>, AbacusError> {
    Err(AbacusError::EvaluationError(
        "live-rates feature is not enabled".to_string(),
    ))
}

/// Retrieves rates using daily cache logic:
/// - If cache file exists and is fresh (< 24 hours), loads from disk without network access.
/// - If not fresh or absent, attempts live fetch via curl (if live-rates enabled) and saves to disk.
/// - If network fetch fails, falls back to existing cached rates if available.
pub fn get_or_update_daily_rates(
    cache_path: &std::path::Path,
) -> Result<HashMap<String, f64>, AbacusError> {
    if is_cache_fresh(cache_path)
        && let Ok(rates) = load_rates_from_cache(cache_path)
    {
        return Ok(rates);
    }

    #[cfg(feature = "live-rates")]
    {
        match fetch_and_cache_rates(cache_path) {
            Ok(rates) => Ok(rates),
            Err(err) => {
                // Fall back to stale cache if present
                if let Ok(rates) = load_rates_from_cache(cache_path) {
                    Ok(rates)
                } else {
                    Err(err)
                }
            }
        }
    }
    #[cfg(not(feature = "live-rates"))]
    {
        // If cache exists on disk (even if older than 24h), use it
        if let Ok(rates) = load_rates_from_cache(cache_path) {
            Ok(rates)
        } else {
            Err(AbacusError::EvaluationError(
                "live-rates feature is not enabled and no cache exists".to_string(),
            ))
        }
    }
}
