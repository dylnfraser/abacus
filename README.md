# abacus

Abacus is a fast, unit-aware mathematical evaluation engine written in Rust. It provides a Pratt parser supporting physical dimensions, implicit multiplication, unit conversions, intervals, date & relative time calculations, business day calendars, statistics, probability distributions, hypothesis testing, confidence intervals, financial mathematics, and dimension-aware linear regression.

---

## Features

- **Pratt Parser** - Operator precedence, grouping, unary/binary/postfix operators, implicit multiplication (`5(2 + 3)`, `2 sqrt(9 m^2)`), dot property access, and `as` / `in` / `to` conversions.
- **Units & Dimensions** - SI base & derived, imperial, US customary, astronomical, nautical, CGS physics, storage, typography, and historical units. Dimensions use a compact 16-byte fixed-point representation (`[i16; 8]`) fitting in a single 128-bit SIMD register.
- **Interval Arithmetic** - Both bracket (`[1, 10]`) and range (`1..10`) interval syntax with physical units, monotonic fast-paths for addition/subtraction, and singularity handling for division crossing zero.
- **Dates & Natural Relative Time** - Parse and calculate with dates (`07-08-2026`, `2026-08-07`), times (`15:30:00`, `3:30 PM`), timezones (`UTC`, `EST`, `PST`, `+02:00`), and natural language expressions (`"last thursday at 3pm + 2 weeks"`, `"today at 12 to 1"`).
- **Business Day Calendars** - $O(1)$ closed-form business day arithmetic (`07-08-2026 + 5 business days`, `10-08-2026 - 1 business day`), interval counting (`workdays(start, end)`), and workday/weekend predicates.
- **Derived SI Reduction** - Automatic reduction to standard derived units (`10 N * 5 m -> 50 J`, `100 W * 5 s -> 500 J`, `12 V * 2 A -> 24 W`).
- **Statistics & Distributions** - Descriptive statistics, dispersion, covariance, correlation, skewness, kurtosis, z-scores, quantiles, and probability distributions (Normal, Student's t, Binomial, Poisson with log-space overflow protection, Chi-Squared, F).
- **Hypothesis Testing & Confidence Intervals** — TI-84-style tests (`ZTest`, `TTest`, `2-SampZTest`, `2-SampTTest`, `1-PropZTest`, `2-PropZTest`, `Chi2Test`) and intervals (`ZInterval`, `TInterval`, `1-PropZInt`, `2-SampTInt`, `2-SampZInt`, `2-PropZInt`).
- **Linear Regression** — Dimension-aware `linreg` returning a structured hash containing `slope`, `intercept`, `r`, `r2`, `se`, means, and prediction calculations.
- **Financial Functions** — `pmt`, `fv`, `pv`, `npv`, `irr`, and `compound`.
- **Zero-Allocation Architecture** — Global lazy registry caching (`abacus::eval`), shared static dimensionless unit pool, zero-allocation lexing, and direct `Display` formatter streaming.
- **Modular Cargo Features & Serde** — Fine-grained feature flags (`units`, `stats`, `distributions`, `date`, `financial`, `serde`, `repl`).

---

## Quickstart

Add Abacus to your `Cargo.toml`:

```toml
[dependencies]
abacus = { git = "https://github.com/dylnfraser/abacus", branch = "main" }
```

### Basic Usage

```rust
use abacus::eval;

fn main() {
    // One-off zero-allocation evaluation via global cached engine:
    let res = eval("10 N * 5 m").unwrap();
    println!("{res}"); // 50 J

    let res = eval("100 km/h in m/s").unwrap();
    println!("{res}"); // 27.77777777777778 m/s

    // Interval arithmetic
    let res = eval("1..10 + 5").unwrap();
    println!("{res}"); // 6..15

    let res = eval("[9.8 m, 10.2 m] / [1.9 s, 2.1 s]").unwrap();
    println!("{res}"); // [4.666666666666667 m/s, 5.368421052631579 m/s]

    // Dates and natural relative time
    let res = eval("07-08-2026 + 5 business days").unwrap();
    println!("{res}"); // 14-08-2026

    // Currencies, natural numbers & division with 'per'
    let res = eval("$50 + €20 in USD").unwrap();
    println!("{res}"); // $73.18

    let res = eval("a thousand dollars per day * 3 days").unwrap();
    println!("{res}"); // $3000

    let res = eval("$3 million in EUR").unwrap();
    println!("{res}"); // 2588430 EUR

    // Dimension-aware linear regression
    let res = eval("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s").unwrap();
    println!("{res}"); // 50 m
}
```

### Engine Configuration

Configure engine behavior, trigonometry modes, dimensional safety, calendar/timezones, recursion limits, and output formatting directly on the `Abacus` instance:

```rust
use abacus::{Abacus, AngleMode, IntervalStyle, Notation, TimeZone, WeekendDays};

let mut calc = Abacus::standard();

// 1. Angle Mode (Degrees, Radians, Gradians)
let calc_deg = Abacus::standard().with_angle_mode(AngleMode::Degrees);
println!("{}", calc_deg.eval("sin(90)").unwrap());  // 1
println!("{}", calc_deg.eval("asin(1)").unwrap()); // 90 deg

// 2. Strict Dimensional Safety (disallow unitless promotion)
let calc_strict = Abacus::standard().with_strict_dimensions(true);
assert!(calc_strict.eval("5 m + 5").is_err()); // IncompatibleDimensions

// 3. Fixed Decimal Places (currency, engineering tables)
let calc_dec = Abacus::standard().with_decimal_places(2);
println!("{}", calc_dec.eval("10 / 3").unwrap()); // 3.33
println!("{}", calc_dec.format_result(&calc_dec.eval("5").unwrap())); // 5.00

// 4. Interval Style Preference (Bracket [a, b] vs Range a..b)
let calc_range = Abacus::standard().with_interval_style(IntervalStyle::Range);
println!("{}", calc_range.eval("[1 m, 5 m] + 2 m").unwrap()); // 3 m..7 m

// 5. Scientific & Engineering Notation (exponents multiple of 3 aligned with SI)
let calc_eng = Abacus::standard().with_notation(Notation::Engineering);
println!("{}", calc_eng.format_result(&calc_eng.eval("45000").unwrap())); // 45e3
println!("{}", calc_eng.format_result(&calc_eng.eval("0.045").unwrap())); // 45e-3

// 6. Default Timezone Anchor (anchors bare dates/times to EST, PST, etc.)
let calc_tz = Abacus::standard().with_default_timezone(TimeZone::parse("EST").unwrap());
println!("{}", calc_tz.eval("07-08-2026").unwrap()); // 07-08-2026 EST

// 7. Custom Weekend / Workweek Definition (Middle East, Sunday-only, etc.)
let calc_gulf = Abacus::standard().with_weekend(WeekendDays::FridaySaturday);
println!("{}", calc_gulf.eval("06-08-2026 + 1 business day").unwrap()); // 09-08-2026 (skips Fri/Sat)

// 8. Configurable Max Recursion Depth (stack protection)
let calc_depth = Abacus::standard().with_max_recursion_depth(32);

// 9. Toggle Implicit Multiplication (require explicit * operator)
let calc_explicit = Abacus::standard().with_implicit_multiplication(false);
assert!(calc_explicit.eval("2(3)").is_err()); // UnexpectedToken

// 10. Configurable Significant Figures (fixed or input-following)
let calc_sig = Abacus::standard().with_significant_figures(3);
println!("{}", calc_sig.eval("12.3456 m").unwrap()); // 12.3 m

let calc_follow = Abacus::standard().with_follow_significant_figures(true);
println!("{}", calc_follow.eval("12.3 * 4.567").unwrap()); // 56.2 (3 sig figs)

// 11. Toggle Automatic Derived SI Unit Reduction
let calc_raw = Abacus::standard().with_auto_derived_units(false);
println!("{}", calc_raw.eval("10 N * 5 m").unwrap()); // 50 N*m (not reduced to J)

// 12. World Currencies & Live Exchange Rates
let calc_curr = Abacus::standard()
    .with_currencies(true)
    .with_live_rates(true)
    .with_currency_rate("EUR", 0.50); // optional custom rate override
println!("{}", calc_curr.eval("$100 in EUR").unwrap()); // 50 EUR

// 13. Currency Cache Configuration (daily disk caching for Frankfurter API rates)
let calc_cache = Abacus::standard().with_currency_cache("/tmp/currency_rates.json");

// 14. Number Scales & Multipliers
let calc_scales = Abacus::standard().with_number_scales(true);
println!("{}", calc_scales.eval("$3 million").unwrap()); // $3000000

// 15. Variables & Mathematical Constants (pi, e, tau, phi)
let mut calc_vars = Abacus::standard();
println!("{}", calc_vars.eval("2 * pi * 5").unwrap()); // 31.41592653589793
calc_vars.set_variable("radius", calc_vars.eval("5 m").unwrap());
println!("{}", calc_vars.eval("pi * radius^2").unwrap()); // 78.53981633974483 m^2
calc_vars.eval_mut("height = 10 m").unwrap();
println!("{}", calc_vars.eval("radius * height").unwrap()); // 50 m^2

// 16. Unit Display Overrides (e.g. "mi/h" -> "mph", "km/h" -> "kmph")
let calc_speed = Abacus::standard().with_common_speed_overrides();
println!("{}", calc_speed.eval("60 miles per hour").unwrap()); // 60 mph
println!("{}", calc_speed.eval("100 km / 1 h").unwrap()); // 100 kmph
```

---

## Examples

### Physical Units & Conversions

|      Expression      |         Result         | Description                              |
| :------------------: | :--------------------: | :--------------------------------------- |
|     `5 m + 3 m`      |         `8 m`          | Basic addition with identical units      |
| `(5 m + 20 cm) as m` |        `5.2 m`         | Mixed prefix conversion                  |
|     `10 N * 5 m`     |         `50 J`         | Automatic reduction to derived SI unit   |
|    `100 W * 5 s`     |        `500 J`         | Power $\times$ time energy reduction     |
|     `12 V * 2 A`     |         `24 W`         | Voltage $\times$ current power reduction |
|     `5 km to m`      |        `5000 m`        | Explicit unit conversion                 |
|   `1 m in inches`    | `39.37007874015748 in` | Metric to Imperial conversion            |
|      `5 km / m`      |         `5000`         | Dimensionless ratio cancellation         |
|    `100 °C to °F`    |        `212 °F`        | Affine temperature scale conversion      |

### World Currencies & Conversions

Abacus supports 30 world currencies with prefix symbol formatting (`$100`, `€50`), cross-currency conversions, financial decimal rounding, and live daily-cached exchange rates from the Frankfurter API:

| Expression           | Result      | Description                                           |
| :------------------- | :---------- | :---------------------------------------------------- |
| `$100`               | `$100`      | Prefix currency symbol rendering                      |
| `-$50.25`            | `-$50.25`   | Negative prefix currency                              |
| `$50 + $25`          | `$75`       | Currency addition                                     |
| `$100 / 3`           | `$33.33`    | Automatic 2-decimal rounding (cents)                  |
| `100 USD in EUR`     | `86.28 EUR` | Live/fixed cross-currency conversion                  |
| `$50 + €20 in USD`   | `$73.18`    | Mixed currency addition converted to USD              |
| `50 EUR in JPY`      | `9281 JPY`  | Automatic 0-decimal rounding for zero-cent currencies |
| `$100 / 2 hours`     | `50 $/h`    | Dimensional currency rate                             |
| `(10 EUR / L) * 5 L` | `50 EUR`    | Volumetric price cancellation                         |

### Number Scales, Natural Articles & Division ("per")

Write math in natural conversational English with scale words, articles, and `per`:

| Expression                              | Result       | Description                             |
| :-------------------------------------- | :----------- | :-------------------------------------- |
| `$3 million`                            | `$3000000`   | Number scale word folding               |
| `5 billion USD / 2 million`             | `$2500`      | Large scale word arithmetic             |
| `a million dollars`                     | `$1000000`   | Indefinite article `"a"` as numeral `1` |
| `a dozen`                               | `12`         | Dozen multiplier                        |
| `a dollar`                              | `$1`         | Singular currency unit                  |
| `an hour in minutes`                    | `60 min`     | Indefinite article `"an"` as `1`        |
| `a thousand dollars per day`            | `1000 $/d`   | `"per"` as division operator `/`        |
| `5 usd a second`                        | `5 $/s`      | `"a"` as rate division operator         |
| `60 miles an hour`                      | `60 mi/h`    | `"an"` as rate division operator        |
| `5 usd per second in 20 days`           | `$8640000`   | Rate accumulation over time duration    |
| `5 usd a second in 20 days`             | `$8640000`   | Accumulation with `"a"` rate syntax     |
| `$50 an hour in 40 hours`               | `$2000`      | Wage accumulation over work hours       |
| `(a thousand dollars per day) * 3 days` | `$3000`      | Time cancellation with rate             |
| `100 meters per second`                 | `100 m/s`    | Physical speed with `per`               |
| `10 meters a second in 5 seconds`       | `50 m`       | Distance accumulation from speed        |
| `60 miles per hour in km/h`             | `96.56 km/h` | Speed unit conversion with `per`        |
| `$50 per hour * 8 hours`                | `$400`       | Wage calculation                        |
| `100 per 4`                             | `25`         | Dimensionless ratio                     |

### Conversational Percentage Engine

Write percentages naturally with everyday financial, discount, tax, tip, ratio, and scaling expressions:

| Expression                         | Result    | Description                                    |
| :--------------------------------- | :-------- | :--------------------------------------------- |
| `20% off $120`                     | `$96`     | Percentage discount operator (`off`)           |
| `15% off 200 EUR`                  | `170 EUR` | Currency discount                              |
| `$100 - 20% off`                   | `$80`     | Trailing `"off"` modifier                      |
| `$85 + 18% tip`                    | `$100.30` | Restaurant tip modifier                        |
| `$50 after 15% tax`                | `$57.50`  | After-tax markup                               |
| `$50 after 15%`                    | `$57.50`  | General after-percentage markup                |
| `$100 after 20% discount`          | `$80`     | After-discount reduction                       |
| `40 as a % of 200`                 | `20%`     | Proportional percentage with articles          |
| `40 out of 200 as %`               | `20%`     | Conversational ratio division with `%` target  |
| `40 out of 200`                    | `0.2`     | `"out of"` natural division                    |
| `3 out of 5 in %`                  | `60%`     | Natural proportion conversion                  |
| `% change from 50 to 75`           | `+50%`    | Signed relative percentage change              |
| `% change from 100 to 80`          | `-20%`    | Negative percentage change                     |
| `percent change from $80 to $100`  | `+25%`    | Percentage difference with units               |
| `30% more than 50 kg`              | `65 kg`   | Relative scaling ($X \times (1 + P)$)          |
| `5 kg more than 50 kg`             | `55 kg`   | Additive scaling ($X + Y$)                     |
| `15% less than 2 hours in minutes` | `102 min` | Relative reduction with target unit conversion |
| `15% less than 2 hours`            | `1.7 h`   | Relative reduction                             |
| `50% + 50%`                        | `100%`    | Direct percentage addition (percentage points) |
| `(% change from 50 to 70) + 50%`   | `90%`     | Arithmetic combining % change and % points     |

### Variables & Mathematical Constants

Evaluate standard mathematical constants and manage programmatic variables:

| Expression / Code             | Result                  | Description                             |
| :---------------------------- | :---------------------- | :-------------------------------------- |
| `2 * pi * 5`                  | `31.41592653589793`     | Standard constant `pi` / `PI`           |
| `e^2`                         | `7.38905609893065`      | Euler's number `e` / `E`                |
| `tau`                         | `6.283185307179586`     | Circle constant $\tau = 2\pi$           |
| `phi`                         | `1.618033988749895`     | Golden ratio $\phi = (1 + \sqrt{5})/2$  |
| `calc.set_variable("r", 5 m)` | —                       | Programmatic variable definition        |
| `calc.eval("pi * r^2")`       | `78.53981633974483 m^2` | Variable evaluation with physical units |
| `calc.eval_mut("w = 10 m")`   | `10 m`                  | Inline assignment syntax                |

### Unit Display Overrides

Abacus provides a display alias system so composite units and rates render using colloquial abbreviations (e.g. `"mi/h"` as `"mph"`, `"km/h"` as `"kmph"`):

```rust
let calc = Abacus::standard()
    .with_unit_display_override("mi/h", "mph")
    .with_unit_display_override("km/h", "kmph");
// Or enable standard speed overrides with a single method:
let calc = Abacus::standard().with_common_speed_overrides();
```

| Expression                   | Standard Display     | With Overrides     | Description                                    |
| :--------------------------- | :------------------- | :----------------- | :--------------------------------------------- |
| `60 miles per hour`          | `60 mi/h`            | `60 mph`           | Idiomatic US/UK speed notation                 |
| `100 km / 1 h`               | `100 km/h`           | `100 kmph`         | Metric road speed abbreviation                 |
| `60 miles per hour in km/h`  | `96.56 km/h`         | `96.56 kmph`       | Speed conversion with target override          |
| `[50 mi/h, 70 mi/h]`         | `[50 mi/h, 70 mi/h]` | `[50 mph, 70 mph]` | Interval endpoints formatting                  |
| `a thousand dollars per day` | `1000 $/d`           | `1000 $/day`       | Custom rate override (`"$/d"` $\to$ `"$/day"`) |

### Interval Arithmetic

Intervals support both bracket syntax `[a, b]` and range syntax `a..b`:

```text
[1 m, 2 m] + [3 m, 4 m]  -> [4 m, 6 m]
1..10 + 5                -> 6..15
[10 m, 20 m] - [2 m, 5 m]-> [5 m, 18 m]
[9.8 m, 10.2 m] / 2 s    -> [4.9 m/s, 5.1 m/s]
[-2 m, 4 m] * [1 s, 3 s] -> [-6 m*s, 12 m*s]
```

### Dates, Relative Time & Business Calendars

Abacus includes a comprehensive date/time calculation engine:

```text
// Date arithmetic & formatting
07-08-2026 + 3 weeks               -> 28-08-2026
07-08-2026 + 5 business days       -> 14-08-2026
10-08-2026 - 1 business day        -> 07-08-2026
workdays(07-08-2026, 14-08-2026)   -> 5 workdays

// Relative time & natural language
now + 2 hours
today at 12 to 1
last thursday at 3pm + 2 weeks
3 days ago
in 45 minutes

// Timezones & AM/PM
15:30:00 EST to PST                -> 12:30:00 PST
3:30 PM + 45 minutes               -> 04:15:00 PM
```

### Event Calculations & Conversational Dates

Abacus natively resolves calendar events, recurring annual holidays, ordinal weekday occurrences, quarter boundaries, and countdown intervals:

| Expression                                            | Example Result | Description                                             |
| :---------------------------------------------------- | :------------- | :------------------------------------------------------ |
| `days until christmas`                                | `115 d`        | Calendar countdown to upcoming annual holiday           |
| `business days until end of quarter`                  | `21 bdays`     | Business days remaining in the current quarter          |
| `days until end of quarter`                           | `29 d`         | Total calendar days until current quarter closes        |
| `third thursday of november 2026`                     | `19-11-2026`   | Nth occurrence of a weekday in a month                  |
| `last friday of october 2026`                         | `30-10-2026`   | Last occurrence of a weekday in a month                 |
| `first monday of january 2027`                        | `04-01-2027`   | 1st occurrence of a weekday in a month                  |
| `days until third thursday of november 2026`          | `79 d`         | Calendar duration to specific ordinal event             |
| `business days until third thursday of november 2026` | `57 bdays`     | Business days to specific ordinal event                 |
| `end of quarter`                                      | `30-09-2026`   | Current quarter end date (Q1: Mar 31, Q2: Jun 30, etc.) |
| `start of quarter`                                    | `01-07-2026`   | Current quarter start date                              |
| `end of next quarter`                                 | `31-12-2026`   | Next quarter closing date                               |
| `end of q1 2027`                                      | `31-03-2027`   | Specific quarter closing date                           |
| `end of month`                                        | `30-09-2026`   | Last day of the current month                           |
| `end of year`                                         | `31-12-2026`   | Last day of the current calendar year                   |
| `thanksgiving 2026`                                   | `26-11-2026`   | 4th Thursday of November                                |
| `black friday 2026`                                   | `27-11-2026`   | Day following Thanksgiving                              |
| `cyber monday 2026`                                   | `30-11-2026`   | Monday following Thanksgiving                           |
| `easter 2026`                                         | `05-04-2026`   | Computus algorithm for Easter Sunday                    |
| `until christmas`                                     | `115 d`        | Prefix `until` defaulting to calendar days              |
| `(days until christmas) in weeks`                     | `16.43 weeks`  | Unit conversion of event countdown duration             |

### Statistics & Probability Distributions

```text
// Descriptive statistics on ranges or arguments
mean(1..10)                        -> 5.5
median(10, 20, 30, 40, 50)         -> 30
std(1..9..2)                       -> 2.8284271247461903
iqr(1..100)                        -> 50

// Distributions
normpdf(0, 0, 1)                   -> 0.3989422804014327
normcdf(1.96, 0, 1)                -> 0.9750021048517796
invnorm(0.975, 0, 1)               -> 1.959963984540054
poissonpmf(3, 2)                   -> 0.22404180765538775
poissonpmf(200, 180)               -> (evaluated in log-space without overflow)
```

### Hypothesis Testing & Confidence Intervals

TI-84-style hypothesis tests return structured hashes containing test statistics, p-values, degrees of freedom, and sample estimates:

```text
// Z-Test on sample data
ZTest(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)

// Output Hash:
{
    df: 3,
    mean: 13 m,
    p_value: 0.0026997960632601965,
    z: 3
}

// Access individual properties via dot notation:
ZTest(10 m, 2 m, 12 m, 14 m, 10 m, 16 m).p_value
```

Supported tests and intervals:

- **Tests**: `ZTest`, `TTest`, `2-SampZTest`, `2-SampTTest`, `1-PropZTest`, `2-PropZTest`, `Chi2Test`
- **Confidence Intervals**: `ZInterval`, `TInterval`, `1-PropZInt`, `2-SampZInt`, `2-SampTInt`, `2-PropZInt`

### Dimension-Aware Linear Regression

`linreg` calculates linear regression while maintaining physical dimensions for slopes, intercepts, standard errors, and predictions:

```text
linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)

// Output:
{
    intercept: 0 m,
    mean_x: 2.5 s,
    mean_y: 25 m,
    r: 1,
    r2: 1,
    se: 0 m,
    slope: 10 m/s
}

// Extrapolate directly:
linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 10 s -> 100 m
```

---

## Cargo Features

Abacus is modular and allows disabling features you do not need:

```toml
[dependencies]
abacus = { version = "0.1", default-features = false, features = ["units"] }
```

|     Feature     | Description                                                                               | Default |
| :-------------: | :---------------------------------------------------------------------------------------- | :------ |
|     `units`     | Full physical unit registry and conversions                                               | **Yes** |
|  `currencies`   | 30 global currencies, prefix formatting, offline fallback rates, and conversions          | **Yes** |
|  `live-rates`   | Live exchange rate sync from Frankfurter API with daily disk caching                      | **Yes** |
| `number-scales` | Natural scale words (`million`, `billion`) and indefinite article evaluation              | **Yes** |
|     `stats`     | Descriptive statistics, dispersion, and linear regression                                 | **Yes** |
| `distributions` | Continuous & discrete probability distributions                                           | **Yes** |
|     `date`      | Date parsing, relative time, and business day arithmetic                                  | **Yes** |
|   `financial`   | Financial functions (`pmt`, `npv`, `irr`, `fv`, `pv`)                                     | **Yes** |
|     `repl`      | Interactive CLI REPL with command history and syntax highlighting                         | No      |
|     `serde`     | Enables `Serialize` and `Deserialize` on core types (`Value`, `Interval`, `Date`, `Hash`) | No      |

---

## Benchmarks

Run the Criterion benchmark suite:

```sh
cargo bench --bench eval_bench
```

Or execute benchmark smoke tests:

```sh
cargo bench --bench eval_bench -- --test
```

---

## REPL

Launch the interactive REPL:

```sh
cargo run --bin repl --features=repl
```

The REPL supports persistent cross-platform history, ANSI syntax formatting in TTY environments, and live expression inspection.

---

## Testing & Quality

Run the test suite and doctests:

```sh
# Run all unit and integration tests
cargo test --all-targets --all-features

# Run executable doctests
cargo test --doc

# Run clippy
cargo clippy --all-targets --all-features
```

## License

MIT
