use abacus::registry::units::currency_units::FIXED_EXCHANGE_RATES;
use abacus::{Abacus, AbacusError, eval};

/// Bundled fallback `rate_per_usd` for `code`, so assertions don't hardcode a rate snapshot.
fn rate(code: &str) -> f64 {
    FIXED_EXCHANGE_RATES
        .iter()
        .find(|d| d.code == code)
        .unwrap()
        .rate_per_usd
}

/// Formats `amount` the way currency results are displayed: rounded to `decimals`
/// with trailing zeros trimmed.
fn money(amount: f64, decimals: usize) -> String {
    let s = format!("{amount:.decimals$}");
    if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    }
}

/// Unique, self-cleaning temp path so tests never touch the user's real cache.
struct TempCache(std::path::PathBuf);

impl TempCache {
    fn new(tag: &str) -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self(std::env::temp_dir().join(format!(
            "abacus_test_{tag}_{}_{nanos}.json",
            std::process::id()
        )))
    }
}

impl Drop for TempCache {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Today's UTC date as `YYYY-MM-DD` (civil-from-days), for cache freshness tests.
fn today_iso() -> String {
    let days = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            / 86_400,
    )
    .unwrap();
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[test]
fn test_basic_currencies() {
    let usd = eval("100 USD").unwrap();
    assert_eq!(usd.to_display(), "$100");
    assert_eq!(usd.into_scalar().unwrap().canonical, 100.0);

    let eur = eval("50 EUR").unwrap();
    assert_eq!(eur.to_display(), "50 EUR");
    let eur_scalar = eur.into_scalar().unwrap();
    assert!((eur_scalar.canonical - 50.0 / rate("EUR")).abs() < 1e-9);

    let gbp = eval("25 GBP").unwrap();
    assert_eq!(gbp.to_display(), "25 GBP");

    let jpy = eval("1000 JPY").unwrap();
    assert_eq!(jpy.to_display(), "1000 JPY");
}

#[test]
fn test_prefix_currency_symbols() {
    let hundred_usd = eval("$100").unwrap();
    assert_eq!(hundred_usd.to_display(), "$100");
    assert_eq!(hundred_usd.into_scalar().unwrap().canonical, 100.0);

    let spaced_usd = eval("$ 100").unwrap();
    assert_eq!(spaced_usd.to_display(), "$100");

    let fifty_eur = eval("€50").unwrap();
    assert_eq!(fifty_eur.to_display(), "50 EUR");

    let spaced_eur = eval("€ 50").unwrap();
    assert_eq!(spaced_eur.to_display(), "50 EUR");

    let twenty_gbp = eval("£25").unwrap();
    assert_eq!(twenty_gbp.to_display(), "25 GBP");

    let thousand_jpy = eval("¥1000").unwrap();
    assert_eq!(thousand_jpy.to_display(), "1000 JPY");
}

#[test]
fn test_currency_aliases() {
    let dollars = eval("100 dollars").unwrap();
    assert_eq!(dollars.to_display(), "$100");
    assert_eq!(dollars.into_scalar().unwrap().canonical, 100.0);

    let one_dollar = eval("1 dollar").unwrap();
    assert_eq!(one_dollar.to_display(), "$1");
    assert_eq!(one_dollar.into_scalar().unwrap().canonical, 1.0);

    let euros = eval("50 euros").unwrap();
    assert_eq!(euros.to_display(), "50 EUR");

    let one_euro = eval("1 euro").unwrap();
    assert_eq!(one_euro.to_display(), "1 EUR");

    // `pounds` is the mass unit; the currency is GBP / £ / quid / pound(s) sterling.
    assert_eq!(eval("20 pounds").unwrap().to_display(), "20 lb");
    assert_eq!(eval("20 quid").unwrap().to_display(), "20 GBP");
    assert_eq!(eval("20 pounds sterling").unwrap().to_display(), "20 GBP");
    assert_eq!(eval("1 pound sterling").unwrap().to_display(), "1 GBP");

    let yen = eval("5000 yen").unwrap();
    assert_eq!(yen.to_display(), "5000 JPY");
}

#[test]
fn test_cross_currency_conversions() {
    // 100 USD in EUR, rounded to 2 decimals
    let eur_conv = eval("100 USD in EUR").unwrap();
    assert_eq!(
        eur_conv.to_display(),
        format!("{} EUR", money(100.0 * rate("EUR"), 2))
    );

    // 86.28 EUR in USD (within display rounding of the exact value)
    let usd_conv = eval("86.28 EUR in USD").unwrap();
    let val = usd_conv.into_scalar().unwrap();
    assert!((val.canonical - 86.28 / rate("EUR")).abs() <= 0.005 + 1e-9);

    // One USD worth of JPY converts back to exactly $1
    let jpy_conv = eval(&format!("{} JPY in USD", rate("JPY"))).unwrap();
    let val_jpy = jpy_conv.into_scalar().unwrap();
    assert!((val_jpy.canonical - 1.0).abs() < 1e-12);

    // EUR to GBP, rounded to 2 decimals
    let eur_to_gbp = eval("100 EUR in GBP").unwrap();
    assert_eq!(
        eur_to_gbp.to_display(),
        format!("{} GBP", money(100.0 / rate("EUR") * rate("GBP"), 2))
    );
}

#[test]
fn test_currency_arithmetic_and_rounding() {
    let sum = eval("$50 + $25").unwrap();
    assert_eq!(sum.to_display(), "$75");

    let diff = eval("100 EUR - 30 EUR").unwrap();
    assert_eq!(diff.to_display(), "70 EUR");

    // Proper 2-decimal rounding on division: $100 / 3 -> $33.33
    let third = eval("$100 / 3").unwrap();
    assert_eq!(third.to_display(), "$33.33");

    // Addition with cross-currency conversion
    let mixed = eval("$50 + €20 in USD").unwrap();
    assert_eq!(
        mixed.to_display(),
        format!("${}", money(50.0 + 20.0 / rate("EUR"), 2))
    );

    let mixed_in_eur = eval("(50 USD + 50 EUR) in EUR").unwrap();
    assert_eq!(
        mixed_in_eur.to_display(),
        format!("{} EUR", money(50.0 * rate("EUR") + 50.0, 2))
    );

    // Zero-decimal currency rounding for JPY
    let jpy_rounded = eval("50 EUR in JPY").unwrap();
    assert_eq!(
        jpy_rounded.to_display(),
        format!("{} JPY", money(50.0 / rate("EUR") * rate("JPY"), 0))
    );
}

#[test]
fn test_currency_with_number_scales() {
    let three_million = eval("$3 million").unwrap();
    assert_eq!(three_million.to_display(), "$3000000");
    assert_eq!(three_million.into_scalar().unwrap().canonical, 3000000.0);

    let five_billion = eval("5 billion USD / 2 million").unwrap();
    assert_eq!(five_billion.to_display(), "$2500");

    let conv = eval("$1 million in EUR").unwrap();
    assert_eq!(
        conv.to_display(),
        format!("{} EUR", money(1e6 * rate("EUR"), 2))
    );
}

#[test]
fn test_dimensional_currency_arithmetic() {
    // Rate of pay: $100 / 2 hours -> 50 $/h
    let hourly = eval("$100 / 2 hours").unwrap();
    assert_eq!(hourly.to_display(), "50 $/h");

    // Price per volume: 10 EUR / liter * 5 liter -> 50 EUR
    let fuel = eval("(10 EUR / L) * 5 L").unwrap();
    assert_eq!(fuel.to_display(), "50 EUR");
}

#[test]
fn test_currency_configuration() {
    // 1. Disable currencies
    let disabled_calc = Abacus::standard().with_currencies(false);
    assert!(matches!(
        disabled_calc.eval("100 USD"),
        Err(AbacusError::UnknownUnit(_))
    ));
    assert!(matches!(
        disabled_calc.eval("$100"),
        Err(AbacusError::UnknownUnit(_))
    ));

    // 2. Custom rate configuration
    // Suppose 1 USD = 0.50 EUR (so 1 EUR = 2 USD)
    let custom_calc = Abacus::standard().with_currency_rate("EUR", 0.50);
    let conv = custom_calc.eval("100 USD in EUR").unwrap();
    assert_eq!(conv.to_display(), "50 EUR");

    let conv_back = custom_calc.eval("50 EUR in USD").unwrap();
    assert_eq!(conv_back.to_display(), "$100");
}

#[test]
fn test_a_million_dollars_and_articles() {
    // "a million dollars"
    let million_dollars = eval("a million dollars").unwrap();
    assert_eq!(million_dollars.to_display(), "$1000000");
    assert_eq!(
        million_dollars.into_scalar().unwrap().canonical,
        1_000_000.0
    );

    // "a billion dollars in EUR"
    let billion_eur = eval("a billion dollars in EUR").unwrap();
    assert_eq!(
        billion_eur.to_display(),
        format!("{} EUR", money(1e9 * rate("EUR"), 2))
    );

    // "a dozen"
    assert_eq!(eval("a dozen").unwrap().to_display(), "12");

    // "a hundred"
    assert_eq!(eval("a hundred").unwrap().to_display(), "100");

    // "a dollar"
    assert_eq!(eval("a dollar").unwrap().to_display(), "$1");

    // "a euro"
    assert_eq!(eval("a euro").unwrap().to_display(), "1 EUR");

    // "a meter"
    assert_eq!(eval("a meter").unwrap().to_display(), "1 m");

    // "an hour in minutes"
    assert_eq!(eval("an hour in minutes").unwrap().to_display(), "60 min");

    // "50% of a million"
    assert_eq!(eval("50% of a million").unwrap().to_display(), "500000");

    // Capitalized "A million dollars"
    assert_eq!(eval("A million dollars").unwrap().to_display(), "$1000000");
}

#[test]
fn test_update_rates_from_json_and_daily_caching() {
    let cache = TempCache::new("daily_cache");
    let cache_file = &cache.0;

    let sample_json = format!(
        r#"{{
        "amount": 1.0,
        "base": "USD",
        "date": "{}",
        "rates": {{
            "EUR": 0.50,
            "GBP": 0.25,
            "JPY": 200.0
        }}
    }}"#,
        today_iso()
    );

    // 1. A standard engine has no implicit cache path, so `update_rates_from_json` never
    //    writes to the user's real cache.
    let mut calc = Abacus::standard();
    assert!(calc.currency_cache_path.is_none());
    assert!(abacus::registry::units::currency_units::is_using_fallback_rates(&calc.units));
    assert_eq!(
        calc.eval("100 USD in EUR").unwrap().to_display(),
        format!("{} EUR", money(100.0 * rate("EUR"), 2))
    );
    calc.update_rates_from_json(&sample_json).unwrap();
    assert!(!abacus::registry::units::currency_units::is_using_fallback_rates(&calc.units));
    assert_eq!(calc.eval("100 USD in EUR").unwrap().to_display(), "50 EUR");

    // 2. Write to an explicit cache file and verify it is fresh
    let mut calc_cached = Abacus::standard().with_currency_cache(cache_file);
    calc_cached.update_rates_from_json(&sample_json).unwrap();

    assert!(cache_file.exists());
    assert!(abacus::is_currency_cache_fresh(cache_file));

    // 3. Create a new calc instance pointing to this cache and update daily rates
    let mut new_calc = Abacus::standard().with_currency_cache(cache_file);
    new_calc.update_daily_rates().unwrap();

    // Rates should now reflect the cached values: 100 USD = 50 EUR, 25 GBP, 20000 JPY
    assert_eq!(
        new_calc.eval("100 USD in EUR").unwrap().to_display(),
        "50 EUR"
    );
    assert_eq!(
        new_calc.eval("100 USD in GBP").unwrap().to_display(),
        "25 GBP"
    );
    assert_eq!(
        new_calc.eval("100 USD in JPY").unwrap().to_display(),
        "20000 JPY"
    );
}

#[test]
fn test_per_division_operator() {
    // "a thousand dollars per day"
    let daily_rate = eval("a thousand dollars per day").unwrap();
    assert_eq!(daily_rate.to_display(), "1000 $/d");

    // "a thousand dollars per day * 3 days"
    let total = eval("(a thousand dollars per day) * 3 days").unwrap();
    assert_eq!(total.to_display(), "$3000");

    // "100 meters per second"
    let speed = eval("100 meters per second").unwrap();
    assert_eq!(speed.to_display(), "100 m/s");

    // "60 miles per hour in km/h"
    let speed_km = Abacus::standard()
        .with_decimal_places(2)
        .eval("60 miles per hour in km/h")
        .unwrap();
    assert_eq!(speed_km.to_display(), "96.56 km/h");

    // "$50 per hour * 8 hours"
    let wages = eval("$50 per hour * 8 hours").unwrap();
    assert_eq!(wages.to_display(), "$400");

    // Dimensionless: "100 per 4"
    let ratio = eval("100 per 4").unwrap();
    assert_eq!(ratio.to_display(), "25");

    // Case-insensitivity: "10 meters PER second"
    let cap_per = eval("10 meters PER second").unwrap();
    assert_eq!(cap_per.to_display(), "10 m/s");
}

#[test]
fn test_rate_accumulation_in_duration() {
    // "5 usd per second in 20 days" -> $8,640,000
    let total = eval("5 usd per second in 20 days").unwrap();
    assert_eq!(total.to_display(), "$8640000");

    // "5 usd a second in 20 days" -> $8,640,000
    let total_a = eval("5 usd a second in 20 days").unwrap();
    assert_eq!(total_a.to_display(), "$8640000");

    // "$50 per hour in 40 hours" -> $2000
    let wages = eval("$50 per hour in 40 hours").unwrap();
    assert_eq!(wages.to_display(), "$2000");

    // "60 miles per hour in 2 hours" -> 120 mi
    let dist = eval("60 miles per hour in 2 hours").unwrap();
    assert_eq!(dist.to_display(), "120 mi");

    // "10 meters a second in 5 seconds" -> 50 m
    let dist_m = eval("10 meters a second in 5 seconds").unwrap();
    assert_eq!(dist_m.to_display(), "50 m");

    // Interval rate accumulation: "[4 usd, 6 usd] per second in 10 seconds" -> [$40, $60]
    let inv = eval("[4 usd, 6 usd] per second in 10 seconds").unwrap();
    assert_eq!(inv.to_display(), "[$40, $60]");
}

#[test]
fn test_a_and_an_as_rate_division() {
    // "5 usd a second" represents 5 usd/s (not 5 usd*s)
    let rate1 = eval("5 usd a second").unwrap();
    assert_eq!(rate1.to_display(), "5 $/s");

    // "$5 a second"
    let rate2 = eval("$5 a second").unwrap();
    assert_eq!(rate2.to_display(), "5 $/s");

    // "60 miles an hour"
    let speed = eval("60 miles an hour").unwrap();
    assert_eq!(speed.to_display(), "60 mi/h");

    // "$50 an hour"
    let wage = eval("$50 an hour").unwrap();
    assert_eq!(wage.to_display(), "50 $/h");

    // "a thousand dollars a day"
    let daily = eval("a thousand dollars a day").unwrap();
    assert_eq!(daily.to_display(), "1000 $/d");
}
