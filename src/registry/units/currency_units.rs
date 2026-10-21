use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    AbacusError,
    registry::unit_registry::UnitRegistry,
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

/// ECB reference date (`YYYY-MM-DD`) of the bundled [`FIXED_EXCHANGE_RATES`].
pub const FALLBACK_RATES_DATE: &str = "2026-10-20";

/// Endpoint used for live exchange rates.
pub const FRANKFURTER_URL: &str = "https://api.frankfurter.dev/v1/latest?base=USD";

/// A cache file is considered fresh if it was written less than this many seconds ago...
const CACHE_MAX_AGE_SECS: u64 = 86_400;
/// ...and its ECB reference date is at most this many days old (weekends and bank holidays
/// mean the newest published date can legitimately lag by a few days).
const CACHE_MAX_DATE_LAG_DAYS: i64 = 5;
/// Upper bound on a plausible `rate_per_usd` (the weakest traded currencies are ~1e5 per USD).
const MAX_PLAUSIBLE_RATE: f64 = 1e7;
/// Lower bound on a plausible `rate_per_usd`.
const MIN_PLAUSIBLE_RATE: f64 = 1e-4;

/// Fixed exchange rates against USD (base = 1.0 USD) captured from
/// <https://api.frankfurter.dev/v1/latest?base=USD> for [`FALLBACK_RATES_DATE`].
///
/// These are used until live or cached rates are loaded; see [`is_using_fallback_rates`].
/// `pound`/`pounds` always mean the avoirdupois mass unit; use `GBP`, `£`, `quid`,
/// `sterling` or `pound sterling` for the currency.
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
        rate_per_usd: 0.89166,
    },
    CurrencyDefinition {
        code: "GBP",
        display: "GBP",
        aliases: &[
            "GBP",
            "gbp",
            "£",
            "quid",
            "sterling",
            "pound sterling",
            "pounds sterling",
        ],
        rate_per_usd: 0.74925,
    },
    CurrencyDefinition {
        code: "JPY",
        display: "JPY",
        aliases: &["JPY", "jpy", "yen", "¥"],
        rate_per_usd: 158.13,
    },
    CurrencyDefinition {
        code: "CAD",
        display: "CAD",
        aliases: &["CAD", "cad"],
        rate_per_usd: 1.4051,
    },
    CurrencyDefinition {
        code: "AUD",
        display: "AUD",
        aliases: &["AUD", "aud"],
        rate_per_usd: 1.4317,
    },
    CurrencyDefinition {
        code: "CHF",
        display: "CHF",
        aliases: &["CHF", "chf"],
        rate_per_usd: 0.82363,
    },
    CurrencyDefinition {
        code: "CNY",
        display: "CNY",
        aliases: &["CNY", "cny", "yuan", "renminbi", "rmb", "RMB"],
        rate_per_usd: 6.678,
    },
    CurrencyDefinition {
        code: "INR",
        display: "INR",
        aliases: &["INR", "inr", "rupee", "rupees", "₹"],
        rate_per_usd: 94.93,
    },
    CurrencyDefinition {
        code: "MXN",
        display: "MXN",
        aliases: &["MXN", "mxn", "peso", "pesos"],
        rate_per_usd: 18.5216,
    },
    CurrencyDefinition {
        code: "BRL",
        display: "BRL",
        aliases: &["BRL", "brl", "real", "reais", "R$"],
        rate_per_usd: 5.0235,
    },
    CurrencyDefinition {
        code: "SEK",
        display: "SEK",
        aliases: &["SEK", "sek", "krona", "kronor"],
        rate_per_usd: 9.918,
    },
    CurrencyDefinition {
        code: "NOK",
        display: "NOK",
        aliases: &["NOK", "nok", "krone", "kroner"],
        rate_per_usd: 9.3843,
    },
    CurrencyDefinition {
        code: "NZD",
        display: "NZD",
        aliases: &["NZD", "nzd"],
        rate_per_usd: 1.7609,
    },
    CurrencyDefinition {
        code: "SGD",
        display: "SGD",
        aliases: &["SGD", "sgd"],
        rate_per_usd: 1.2785,
    },
    CurrencyDefinition {
        code: "HKD",
        display: "HKD",
        aliases: &["HKD", "hkd"],
        rate_per_usd: 7.8487,
    },
    CurrencyDefinition {
        code: "KRW",
        display: "KRW",
        aliases: &["KRW", "krw", "won", "₩"],
        rate_per_usd: 1_327.84,
    },
    CurrencyDefinition {
        code: "TRY",
        display: "TRY",
        aliases: &["TRY", "try", "lira", "₺"],
        rate_per_usd: 49.577,
    },
    CurrencyDefinition {
        code: "ZAR",
        display: "ZAR",
        aliases: &["ZAR", "zar", "rand"],
        rate_per_usd: 16.4752,
    },
    CurrencyDefinition {
        code: "PLN",
        display: "PLN",
        aliases: &["PLN", "pln", "zloty"],
        rate_per_usd: 3.9026,
    },
    CurrencyDefinition {
        code: "DKK",
        display: "DKK",
        aliases: &["DKK", "dkk"],
        rate_per_usd: 6.6648,
    },
    CurrencyDefinition {
        code: "CZK",
        display: "CZK",
        aliases: &["CZK", "czk"],
        rate_per_usd: 21.693,
    },
    CurrencyDefinition {
        code: "HUF",
        display: "HUF",
        aliases: &["HUF", "huf"],
        rate_per_usd: 325.17,
    },
    CurrencyDefinition {
        code: "ILS",
        display: "ILS",
        aliases: &["ILS", "ils", "shekel", "₪"],
        rate_per_usd: 3.1077,
    },
    CurrencyDefinition {
        code: "IDR",
        display: "IDR",
        aliases: &["IDR", "idr", "rupiah"],
        rate_per_usd: 17_986.0,
    },
    CurrencyDefinition {
        code: "MYR",
        display: "MYR",
        aliases: &["MYR", "myr", "ringgit"],
        rate_per_usd: 4.0827,
    },
    CurrencyDefinition {
        code: "PHP",
        display: "PHP",
        aliases: &["PHP", "php"],
        rate_per_usd: 63.133,
    },
    CurrencyDefinition {
        code: "THB",
        display: "THB",
        aliases: &["THB", "thb", "baht", "฿"],
        rate_per_usd: 33.65,
    },
    CurrencyDefinition {
        code: "RON",
        display: "RON",
        aliases: &["RON", "ron", "leu"],
        rate_per_usd: 4.7533,
    },
    CurrencyDefinition {
        code: "ISK",
        display: "ISK",
        aliases: &["ISK", "isk"],
        rate_per_usd: 121.44,
    },
];

fn currency_unit(def: &CurrencyDefinition, rate_per_usd: f64) -> Arc<Unit> {
    Arc::new(Unit {
        scalar: 1.0 / rate_per_usd,
        offset: 0.0,
        dimensions: Dimensions::CURRENCY,
        display: UnitExpr::single(def.display),
    })
}

/// Registers all currency units with default fixed exchange rates into the provided map.
pub fn register_currency_units<S: std::hash::BuildHasher>(map: &mut HashMap<String, Arc<Unit>, S>) {
    for def in FIXED_EXCHANGE_RATES {
        let unit = currency_unit(def, def.rate_per_usd);
        for &alias in def.aliases {
            map.insert(alias.to_string(), Arc::clone(&unit));
        }
    }
}

/// Updates currency exchange rates in the provided unit map.
/// `rates` maps currency code (e.g. "EUR") to `rate_per_usd` (e.g. 0.89166).
/// Non-finite or non-positive rates are ignored.
pub fn update_currency_rates_in_map<S: std::hash::BuildHasher, R: std::hash::BuildHasher>(
    map: &mut HashMap<String, Arc<Unit>, S>,
    rates: &HashMap<String, f64, R>,
) {
    for def in FIXED_EXCHANGE_RATES {
        if let Some(&rate_per_usd) = rates.get(def.code)
            && rate_per_usd.is_finite()
            && rate_per_usd > 0.0
        {
            let unit = currency_unit(def, rate_per_usd);
            for &alias in def.aliases {
                map.insert(alias.to_string(), Arc::clone(&unit));
            }
        }
    }
}

/// Returns `true` if every currency in `registry` still uses the bundled
/// [`FIXED_EXCHANGE_RATES`] (dated [`FALLBACK_RATES_DATE`]), i.e. no live, cached or
/// manually set rate has been applied.
#[must_use]
pub fn is_using_fallback_rates(registry: &UnitRegistry) -> bool {
    FIXED_EXCHANGE_RATES.iter().all(|def| {
        registry.get(def.code).is_none_or(|unit| {
            (unit.scalar - 1.0 / def.rate_per_usd).abs() <= f64::EPSILON * unit.scalar.abs()
        })
    })
}

// ---------------------------------------------------------------------------------------------
// Minimal, dependency-free JSON reader (enough for the Frankfurter response format).
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(fields) => fields.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

const MAX_JSON_DEPTH: usize = 16;

struct JsonReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

fn json_err(msg: &str) -> AbacusError {
    AbacusError::EvaluationError(format!("invalid exchange-rate JSON: {msg}"))
}

impl<'a> JsonReader<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while let Some(b' ' | b'\t' | b'\n' | b'\r') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), AbacusError> {
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b) {
            self.pos += 1;
            Ok(())
        } else {
            Err(json_err(&format!("expected '{}'", b as char)))
        }
    }

    fn parse_document(mut self) -> Result<Json, AbacusError> {
        let value = self.parse_value(0)?;
        self.skip_ws();
        if self.pos != self.bytes.len() {
            return Err(json_err("trailing characters"));
        }
        Ok(value)
    }

    fn parse_value(&mut self, depth: usize) -> Result<Json, AbacusError> {
        if depth > MAX_JSON_DEPTH {
            return Err(json_err("nesting too deep"));
        }
        self.skip_ws();
        match self.bytes.get(self.pos) {
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string().map(Json::Str),
            Some(b't') => self.parse_literal("true", Json::Bool(true)),
            Some(b'f') => self.parse_literal("false", Json::Bool(false)),
            Some(b'n') => self.parse_literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            Some(_) => Err(json_err("unexpected character")),
            None => Err(json_err("unexpected end of input")),
        }
    }

    fn parse_literal(&mut self, lit: &str, value: Json) -> Result<Json, AbacusError> {
        if self.bytes[self.pos..].starts_with(lit.as_bytes()) {
            self.pos += lit.len();
            Ok(value)
        } else {
            Err(json_err("invalid literal"))
        }
    }

    fn parse_number(&mut self) -> Result<Json, AbacusError> {
        let start = self.pos;
        while let Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') = self.bytes.get(self.pos) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| json_err("invalid number"))?;
        let value: f64 = text.parse().map_err(|_| json_err("invalid number"))?;
        if !value.is_finite() {
            return Err(json_err("non-finite number"));
        }
        Ok(Json::Num(value))
    }

    fn parse_string(&mut self) -> Result<String, AbacusError> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let start = self.pos;
            while let Some(&b) = self.bytes.get(self.pos) {
                if b == b'"' || b == b'\\' || b < 0x20 {
                    break;
                }
                self.pos += 1;
            }
            out.push_str(
                std::str::from_utf8(&self.bytes[start..self.pos])
                    .map_err(|_| json_err("invalid UTF-8 in string"))?,
            );
            match self.bytes.get(self.pos) {
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let esc = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| json_err("unterminated escape"))?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .and_then(|h| std::str::from_utf8(h).ok())
                                .and_then(|h| u32::from_str_radix(h, 16).ok())
                                .ok_or_else(|| json_err("invalid \\u escape"))?;
                            self.pos += 4;
                            out.push(char::from_u32(hex).unwrap_or('\u{fffd}'));
                        }
                        _ => return Err(json_err("invalid escape")),
                    }
                }
                _ => return Err(json_err("unterminated string")),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Json, AbacusError> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                _ => return Err(json_err("expected ',' or ']'")),
            }
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<Json, AbacusError> {
        self.expect(b'{')?;
        let mut fields = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(fields));
        }
        loop {
            self.skip_ws();
            let key = self.parse_string()?;
            self.expect(b':')?;
            let value = self.parse_value(depth + 1)?;
            fields.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(fields));
                }
                _ => return Err(json_err("expected ',' or '}'")),
            }
        }
    }
}

/// A validated Frankfurter response.
struct FrankfurterRates {
    rates: HashMap<String, f64>,
    date: Option<String>,
}

fn parse_frankfurter(json: &str) -> Result<FrankfurterRates, AbacusError> {
    let doc = JsonReader::new(json).parse_document()?;
    if !matches!(doc, Json::Obj(_)) {
        return Err(json_err("top-level value must be an object"));
    }

    match doc.get("base") {
        None => {}
        Some(Json::Str(base)) if base == "USD" => {}
        Some(_) => return Err(json_err("base currency must be \"USD\"")),
    }
    match doc.get("amount") {
        None => {}
        Some(Json::Num(amount)) if (*amount - 1.0).abs() < f64::EPSILON => {}
        Some(_) => return Err(json_err("amount must be 1")),
    }
    let date = match doc.get("date") {
        None => None,
        Some(Json::Str(d)) if parse_iso_date_days(d).is_some() => Some(d.clone()),
        Some(_) => return Err(json_err("date must be a YYYY-MM-DD string")),
    };

    let Some(Json::Obj(entries)) = doc.get("rates") else {
        return Err(json_err("missing 'rates' object"));
    };
    if entries.is_empty() {
        return Err(json_err("'rates' object is empty"));
    }

    let mut rates = HashMap::with_capacity(entries.len() + 1);
    rates.insert("USD".to_string(), 1.0);
    for (code, value) in entries {
        if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(json_err(&format!("invalid currency code '{code}'")));
        }
        let Json::Num(rate) = value else {
            return Err(json_err(&format!("rate for {code} is not a number")));
        };
        if !(MIN_PLAUSIBLE_RATE..=MAX_PLAUSIBLE_RATE).contains(rate) {
            return Err(json_err(&format!("implausible rate for {code}: {rate}")));
        }
        rates.insert(code.clone(), *rate);
    }

    Ok(FrankfurterRates { rates, date })
}

/// Parses and validates a Frankfurter JSON response
/// (e.g. `{"amount":1.0,"base":"USD","date":"2026-10-20","rates":{"EUR":0.89166,...}}`).
///
/// # Errors
/// Returns an error if the document is not valid JSON, the base is not USD, or any rate
/// is missing, non-numeric, non-finite, non-positive or implausibly large/small.
pub fn parse_frankfurter_json(json: &str) -> Result<HashMap<String, f64>, AbacusError> {
    parse_frankfurter(json).map(|r| r.rates)
}

/// Days since 1970-01-01 for a `YYYY-MM-DD` string.
fn parse_iso_date_days(s: &str) -> Option<i64> {
    let mut parts = s.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let days_in_month = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return None,
    };
    if !(1..=days_in_month).contains(&d) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

fn today_utc_days() -> Option<i64> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    i64::try_from(secs / 86_400).ok()
}

// ---------------------------------------------------------------------------------------------
// Network fetch
// ---------------------------------------------------------------------------------------------

/// Runs `curl` to download the Frankfurter response body.
#[cfg(feature = "live-rates")]
fn curl_fetch_body() -> Result<String, AbacusError> {
    // Prefer the system curl over whatever happens to be first on PATH.
    let curl = ["/usr/bin/curl", "/bin/curl"]
        .into_iter()
        .find(|p| Path::new(p).is_file())
        .unwrap_or("curl");

    let output = std::process::Command::new(curl)
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--connect-timeout",
            "4",
            "--max-time",
            "8",
            "--max-filesize",
            "65536",
            FRANKFURTER_URL,
        ])
        .output()
        .map_err(|e| {
            AbacusError::EvaluationError(format!("failed to execute curl for live rates: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(AbacusError::EvaluationError(format!(
            "curl failed fetching live exchange rates ({}): {}",
            output.status,
            stderr.trim()
        )));
    }

    String::from_utf8(output.stdout).map_err(|e| {
        AbacusError::EvaluationError(format!("invalid UTF-8 response from frankfurter: {e}"))
    })
}

/// Fetches live exchange rates from <https://api.frankfurter.dev/v1/latest?base=USD> via `curl`.
///
/// # Errors
/// Returns an error if curl is unavailable, the request fails, or the response is invalid.
#[cfg(feature = "live-rates")]
pub fn fetch_frankfurter_rates() -> Result<HashMap<String, f64>, AbacusError> {
    parse_frankfurter_json(&curl_fetch_body()?)
}

/// # Errors
/// Always returns an error because the `live-rates` feature is disabled.
#[cfg(not(feature = "live-rates"))]
pub fn fetch_frankfurter_rates() -> Result<HashMap<String, f64>, AbacusError> {
    Err(AbacusError::EvaluationError(
        "live-rates feature is not enabled".to_string(),
    ))
}

/// Fetches live exchange rates and writes the validated response to the cache file.
///
/// A failure to write the cache does not fail the fetch (the rates are still returned).
///
/// # Errors
/// Returns an error if the fetch or validation fails.
#[cfg(feature = "live-rates")]
pub fn fetch_and_cache_rates(cache_path: &Path) -> Result<HashMap<String, f64>, AbacusError> {
    let body = curl_fetch_body()?;
    let rates = parse_frankfurter_json(&body)?;
    // Caching is best-effort: a read-only or missing cache directory must not make
    // otherwise-successful live rates unavailable.
    let _ = save_rates_to_cache(&body, cache_path);
    Ok(rates)
}

/// # Errors
/// Always returns an error because the `live-rates` feature is disabled.
#[cfg(not(feature = "live-rates"))]
pub fn fetch_and_cache_rates(_cache_path: &Path) -> Result<HashMap<String, f64>, AbacusError> {
    Err(AbacusError::EvaluationError(
        "live-rates feature is not enabled".to_string(),
    ))
}

// ---------------------------------------------------------------------------------------------
// Cache file handling
// ---------------------------------------------------------------------------------------------

/// Returns the default path to the cached exchange rates file.
///
/// Resolution order: `ABACUS_CURRENCY_CACHE` (file), `ABACUS_CACHE_DIR`, `XDG_CACHE_HOME`,
/// `$HOME/.cache` / `%LOCALAPPDATA%` / `%USERPROFILE%\.cache`, and finally a per-user,
/// owner-only (`0700`) directory under the system temp dir. Files in that last location are
/// only read or written if the directory is private and not a symlink. Writes are always
/// atomic (temp file + rename), so a pre-planted symlink at the final path is replaced rather
/// than followed.
#[must_use]
pub fn default_currency_cache_path() -> PathBuf {
    let non_empty = |key: &str| std::env::var_os(key).filter(|v| !v.is_empty());

    if let Some(custom) = non_empty("ABACUS_CURRENCY_CACHE") {
        return PathBuf::from(custom);
    }
    if let Some(dir) = non_empty("ABACUS_CACHE_DIR") {
        return PathBuf::from(dir).join("currency_rates.json");
    }
    if let Some(xdg) = non_empty("XDG_CACHE_HOME") {
        return PathBuf::from(xdg)
            .join("abacus")
            .join("currency_rates.json");
    }
    if let Some(home) = non_empty("HOME") {
        return PathBuf::from(home)
            .join(".cache")
            .join("abacus")
            .join("currency_rates.json");
    }
    if let Some(local) = non_empty("LOCALAPPDATA") {
        return PathBuf::from(local)
            .join("abacus")
            .join("currency_rates.json");
    }
    if let Some(profile) = non_empty("USERPROFILE") {
        return PathBuf::from(profile)
            .join(".cache")
            .join("abacus")
            .join("currency_rates.json");
    }

    temp_fallback_cache_path()
}

/// Last-resort cache location when no home/cache directory is known: a per-user
/// directory inside the (possibly shared) system temp dir.
fn temp_fallback_cache_path() -> PathBuf {
    let user = ["USER", "USERNAME", "LOGNAME"]
        .iter()
        .find_map(|key| std::env::var_os(key).filter(|v| !v.is_empty()))
        .map(|u| {
            u.to_string_lossy()
                .chars()
                .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
                .collect::<String>()
        })
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| "default".to_string());
    std::env::temp_dir()
        .join(format!("abacus-{user}"))
        .join("currency_rates.json")
}

/// Whether `cache_path` is the shared-temp fallback location, which must only be used if its
/// directory is private (explicitly configured paths are trusted as given).
fn needs_private_dir(cache_path: &Path) -> bool {
    cache_path == temp_fallback_cache_path()
}

/// For directories in the shared temp dir, require an existing, non-symlinked directory
/// that is not accessible to group/other (or create one with mode `0700`).
#[cfg(unix)]
fn check_private_dir(dir: &Path, create: bool) -> Result<(), AbacusError> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};

    if create && std::fs::symlink_metadata(dir).is_err() {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .map_err(|e| {
                AbacusError::EvaluationError(format!("failed to create currency cache dir: {e}"))
            })?;
    }
    let meta = std::fs::symlink_metadata(dir).map_err(|e| {
        AbacusError::EvaluationError(format!("currency cache dir unavailable: {e}"))
    })?;
    if !meta.is_dir() || meta.permissions().mode() & 0o077 != 0 {
        return Err(AbacusError::EvaluationError(format!(
            "refusing to use shared currency cache dir {} (must be a private 0700 directory)",
            dir.display()
        )));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private_dir(dir: &Path, create: bool) -> Result<(), AbacusError> {
    if create {
        std::fs::create_dir_all(dir).map_err(|e| {
            AbacusError::EvaluationError(format!("failed to create currency cache dir: {e}"))
        })?;
    }
    Ok(())
}

/// Returns true if the cache file was written within the last 24 hours **and** contains a
/// valid response whose ECB reference `date` is recent (at most a few days old, to allow
/// for weekends and bank holidays).
#[must_use]
pub fn is_cache_fresh(cache_path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(cache_path) else {
        return false;
    };
    let mtime_fresh = metadata
        .modified()
        .ok()
        .and_then(|m| m.elapsed().ok())
        .is_some_and(|age| age.as_secs() < CACHE_MAX_AGE_SECS);
    if !mtime_fresh {
        return false;
    }

    let Ok(content) = read_cache_file(cache_path) else {
        return false;
    };
    let Ok(parsed) = parse_frankfurter(&content) else {
        return false;
    };
    match (
        parsed.date.as_deref().and_then(parse_iso_date_days),
        today_utc_days(),
    ) {
        (Some(date), Some(today)) => (-1..=CACHE_MAX_DATE_LAG_DAYS).contains(&(today - date)),
        _ => false,
    }
}

fn read_cache_file(cache_path: &Path) -> Result<String, AbacusError> {
    if let Some(parent) = cache_path.parent()
        && needs_private_dir(cache_path)
    {
        check_private_dir(parent, false)?;
    }
    std::fs::read_to_string(cache_path)
        .map_err(|e| AbacusError::EvaluationError(format!("failed to read currency cache: {e}")))
}

/// Validates `json_str` and saves it to `cache_path` atomically (write to a temporary file
/// in the same directory, then rename), creating parent directories if necessary.
///
/// # Errors
/// Returns an error if the JSON is invalid or the file cannot be written.
pub fn save_rates_to_cache(json_str: &str, cache_path: &Path) -> Result<(), AbacusError> {
    use std::io::Write;

    parse_frankfurter(json_str)?;

    let parent = cache_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if needs_private_dir(cache_path) {
        check_private_dir(parent, true)?;
    } else {
        std::fs::create_dir_all(parent).map_err(|e| {
            AbacusError::EvaluationError(format!("failed to create currency cache dir: {e}"))
        })?;
    }

    let file_name = cache_path
        .file_name()
        .map_or_else(|| "currency_rates.json".into(), |n| n.to_string_lossy());
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp_path = parent.join(format!(".{file_name}.{}.{nanos}.tmp", std::process::id()));

    let write_err = |e: std::io::Error| {
        AbacusError::EvaluationError(format!("failed to write currency cache: {e}"))
    };

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = options
        .open(&tmp_path)
        .and_then(|mut file| {
            file.write_all(json_str.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| std::fs::rename(&tmp_path, cache_path));
    if let Err(e) = result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(write_err(e));
    }
    Ok(())
}

/// Loads exchange rates from a cached JSON file.
///
/// # Errors
/// Returns an error if the file cannot be read or does not contain a valid response.
pub fn load_rates_from_cache(cache_path: &Path) -> Result<HashMap<String, f64>, AbacusError> {
    parse_frankfurter_json(&read_cache_file(cache_path)?)
}

/// Retrieves rates using daily cache logic:
/// - If the cache is fresh (see [`is_cache_fresh`]), loads it without network access.
/// - Otherwise attempts a live fetch via curl (if `live-rates` is enabled) and saves it.
/// - If the network fetch fails, falls back to existing (stale) cached rates if available.
///
/// # Errors
/// Returns an error if no fresh cache exists and the live fetch (and stale-cache fallback) fail.
pub fn get_or_update_daily_rates(cache_path: &Path) -> Result<HashMap<String, f64>, AbacusError> {
    if is_cache_fresh(cache_path)
        && let Ok(rates) = load_rates_from_cache(cache_path)
    {
        return Ok(rates);
    }

    #[cfg(feature = "live-rates")]
    {
        fetch_and_cache_rates(cache_path)
            .or_else(|err| load_rates_from_cache(cache_path).map_err(|_| err))
    }
    #[cfg(not(feature = "live-rates"))]
    {
        load_rates_from_cache(cache_path).map_err(|_| {
            AbacusError::EvaluationError(
                "live-rates feature is not enabled and no cache exists".to_string(),
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str =
        r#"{"amount":1.0,"base":"USD","date":"2026-10-20","rates":{"EUR":0.89166,"JPY":158.13}}"#;

    #[test]
    fn parses_valid_response() {
        let rates = parse_frankfurter_json(SAMPLE).unwrap();
        assert_eq!(rates["EUR"], 0.89166);
        assert_eq!(rates["JPY"], 158.13);
        assert_eq!(rates["USD"], 1.0);
    }

    #[test]
    fn parses_escapes_and_whitespace() {
        let json = "{ \"base\" : \"US\\u0044\", \"rates\" : { \"EUR\" : 9e-1 } , \"x\": [1, {\"y\": null}, true] }";
        assert_eq!(parse_frankfurter_json(json).unwrap()["EUR"], 0.9);
    }

    #[test]
    fn rejects_malformed_and_hostile_input() {
        for bad in [
            "",
            "[]",
            "{",
            r#"{"rates":{}}"#,
            r#"{"rates":{"EUR":inf}}"#,
            r#"{"rates":{"EUR":1e999}}"#,
            r#"{"rates":{"EUR":-1}}"#,
            r#"{"rates":{"EUR":0}}"#,
            r#"{"rates":{"EUR":1e300}}"#,
            r#"{"rates":{"EUR":"0.9"}}"#,
            r#"{"rates":{"eur":0.9}}"#,
            r#"{"base":"EUR","rates":{"USD":1.1}}"#,
            r#"{"amount":100,"rates":{"EUR":89}}"#,
            r#"{"date":"2026-02-30","rates":{"EUR":0.9}}"#,
            r#"{"rates":{"EUR":0.9}} trailing"#,
            r#"{"rates":{"EUR":0.9},"rates2":{"a":}}"#,
        ] {
            assert!(parse_frankfurter_json(bad).is_err(), "accepted: {bad}");
        }
        let deep = format!("{}{}", "[".repeat(1000), "]".repeat(1000));
        assert!(parse_frankfurter_json(&deep).is_err());
    }

    #[test]
    fn iso_dates() {
        assert_eq!(parse_iso_date_days("1970-01-01"), Some(0));
        assert_eq!(parse_iso_date_days("2000-03-01"), Some(11_017));
        assert_eq!(parse_iso_date_days("2024-02-29"), Some(19_782));
        assert_eq!(parse_iso_date_days("2023-02-29"), None);
        assert_eq!(parse_iso_date_days("2026-13-01"), None);
        assert_eq!(parse_iso_date_days("26-10-20"), None);
    }
}
