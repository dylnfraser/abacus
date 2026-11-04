# abacus

Abacus is a calculator engine for Rust that keeps track of units. It evaluates expressions with physical dimensions, unit conversions, intervals, dates and business days, statistics and probability distributions, hypothesis tests, linear regression and financial functions.

## Features

- Parsing: operator precedence, implicit multiplication (`5(2 + 3)`), compound measurements (`5 ft 3 in`), property access with `.`, and conversions with `as`, `in` or `to`. An expression can be compiled once and evaluated many times. Errors point at the part of the input that caused them. Lexing is linear in the input length, and recursion depth is limited by a configurable maximum and a check on remaining stack.
- Units: SI, US customary, imperial, astronomical, nautical, CGS, engineering, storage, typographic and historical units. Dimension exponents are stored as fixed-point integers (`[i32; 9]`, scaled by 27720, so any fraction with a denominator up to 12 is exact). The arithmetic is checked, so an overflow gives an error instead of a wrong answer.
- Derived units: composite results reduce to named SI units (`10 N * 5 m` gives `50 J`). A unit you typed yourself, such as `1 atm`, is kept as written.
- Intervals: `[1, 10]` or `1..10`, with or without units. Bounds are rounded outward, so each result contains the exact result of the operation. Dividing by an interval that contains zero gives `[-inf, inf]`.
- Dates and times: dates, times, timezones, calendar months and years, and relative phrases such as `last thursday at 3pm + 2 weeks`. `now` and `today` use the local timezone, read from `TZ` or the system zoneinfo.
- Business days: `07-08-2026 + 5 business days`, `workdays(start, end)`, configurable weekends, and workday/weekend predicates.
- Statistics: descriptive statistics, covariance, correlation, skewness, kurtosis, z-scores and quantiles, plus the Normal, Student's t, chi-squared, F, binomial, Poisson, geometric, hypergeometric, exponential and uniform distributions, computed to near full double precision.
- Hypothesis tests and confidence intervals in the style of the TI-84, with separate functions for raw data and summary statistics.
- Linear regression: `linreg` keeps units and returns the slope, intercept, `r`, `r2`, standard errors and means.
- Financial functions: `pmt`, `fv`, `pv`, `npv`, `irr` and `compound`.
- The library has no required dependencies. Each subsystem is behind a Cargo feature (see [Cargo Features](#cargo-features)); the `live-rates` feature runs the system `curl` to fetch exchange rates.

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
    // One-off evaluation via the global cached engine:
    let res = eval("10 N * 5 m").unwrap();
    println!("{res}"); // 50 J

    let res = eval("100 km/h in m/s").unwrap();
    println!("{res}"); // 27.77777777777778 m/s

    // Interval arithmetic (bounds are rounded outward)
    let res = eval("1..10 + 5").unwrap();
    println!("{res}"); // 6..15

    let res = eval("[9.8 m, 10.2 m] / [1.9 s, 2.1 s]").unwrap();
    println!("{res}"); // [4.666666666666666 m/s, 5.36842105263158 m/s]

    // Dates and business days
    let res = eval("07-08-2026 + 5 business days").unwrap();
    println!("{res}"); // August 14, 2026

    // Currencies (bundled fallback rates unless live rates are enabled), number words & 'per'
    let res = eval("$50 + €20 in USD").unwrap();
    println!("{res}"); // $72.43

    let res = eval("a thousand dollars per day * 3 days").unwrap();
    println!("{res}"); // $3000

    let res = eval("$3 million in EUR").unwrap();
    println!("{res}"); // 2674980 EUR

    // Dimension-aware linear regression
    let res = eval("linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 5 s").unwrap();
    println!("{res}"); // 50 m
}
```

Numbers print in their shortest round-trip form, with float noise in the last couple of bits hidden, so `0.1 + 0.2` prints `0.3`. Interval bounds always print exactly, so the outward rounding stays visible: `[0.1, 0.1] + [0.2, 0.2]` is `[0.3, 0.30000000000000004]`.

### Compiled Expressions & Diagnostics

`compile` parses an expression once and `eval_compiled` evaluates it against the engine's current variables and settings. This is several times faster than evaluating the text again:

```rust
use abacus::Abacus;

let mut calc = Abacus::standard().with_variable("t", 0.0);
let height = calc.compile("20 m/s * t - 0.5 * 9.81 m/s^2 * t^2").unwrap();
for t in [0.0, 1.0, 2.0] {
    calc.set_variable("t", abacus::Value::new(t, calc.units().unit("s").unwrap()));
    println!("{}", calc.eval_compiled(&height).unwrap()); // 0 m, 15.095 m, 20.38 m
}
```

A compiled expression always gives the same result as `eval` on its source. The source is parsed again only if it contains date literals, or if the engine changed in a way that affects parsing: a variable starts or stops shadowing a unit name the expression uses, or the units, operators or input settings changed. Changing variable values, or defining unrelated variables, does not cause a re-parse.

`eval_detailed` and `eval_mut_detailed` return a `Diagnostic` whose `Display` shows where the error is:

```rust
use abacus::Abacus;

let err = Abacus::standard().eval_detailed("5 m + 3 s").unwrap_err();
println!("{err}");
// cannot perform operation on incompatible dimensions
//   5 m + 3 s
//       ^
//   note: m + s
```

### Engine Configuration

Settings live on the `Abacus` instance. Each one has a consuming builder `with_*`, an in-place `set_*` that returns `&mut Self`, and a getter named after the setting (`calc.angle_mode()`). Optional settings take a value or `None`: `with_decimal_places(2)`, `set_decimal_places(None)`. `Abacus::new()` and `Abacus::default()` return the standard engine; `Abacus::empty()` starts with empty registries.

```rust
use abacus::{Abacus, AngleMode};

let mut calc = Abacus::standard().with_angle_mode(AngleMode::Degrees);
calc.set_decimal_places(2).set_strict_dimensions(true);
assert_eq!(calc.decimal_places(), Some(2));
```

```rust
use abacus::{Abacus, AngleMode, DateFormat, IntervalStyle, Notation, TimeZone, WeekendDays};

// 1. Angle Mode (Degrees, Radians, Gradians)
let calc_deg = Abacus::standard().with_angle_mode(AngleMode::Degrees);
println!("{}", calc_deg.eval("sin(90)").unwrap());  // 1
println!("{}", calc_deg.eval("asin(1)").unwrap()); // 90 deg

// 2. Strict Dimensional Safety (disallow unitless promotion)
let calc_strict = Abacus::standard().with_strict_dimensions(true);
assert!(calc_strict.eval("5 m + 5").is_err()); // IncompatibleDimensions

// 3. Fixed Decimal Places (applied when displaying; stored values keep full precision)
let calc_dec = Abacus::standard().with_decimal_places(2);
println!("{}", calc_dec.eval("10 / 3").unwrap()); // 3.33
println!("{}", calc_dec.format_result(&calc_dec.eval("5").unwrap())); // 5.00

// 4. Interval Style Preference (Bracket [a, b] vs Range a..b)
let calc_range = Abacus::standard().with_interval_style(IntervalStyle::Range);
println!("{}", calc_range.eval("[1 m, 5 m] + 2 m").unwrap()); // 3 m..7 m

// 5. Scientific & Engineering Notation (exponents multiple of 3 aligned with SI).
//    Notation is a rendering setting: it applies through `format_result` / `eval_to_string`,
//    not to `eval(..).to_string()`.
let calc_eng = Abacus::standard().with_notation(Notation::Engineering);
println!("{}", calc_eng.eval_to_string("45000").unwrap()); // 45e3
println!("{}", calc_eng.format_result(&calc_eng.eval("0.045").unwrap())); // 45e-3

// 6. Default Timezone (bare dates/times are in this zone; `now` is converted into it)
let calc_tz = Abacus::standard().with_default_timezone(TimeZone::parse("EST").unwrap());
println!("{}", calc_tz.eval("07-08-2026 10:00").unwrap()); // August 7, 2026 10:00:00 EST

// 7. Custom Weekend / Workweek Definition
let calc_gulf = Abacus::standard().with_weekend(WeekendDays::FridaySaturday);
println!("{}", calc_gulf.eval("06-08-2026 + 1 business day").unwrap()); // August 9, 2026 (skips Fri/Sat)

// 8. Max Recursion Depth (clamped to a safe maximum of 256)
let calc_depth = Abacus::standard().with_max_recursion_depth(32);

// 9. Toggle Implicit Multiplication (require explicit * operator)
let calc_explicit = Abacus::standard().with_implicit_multiplication(false);
assert!(calc_explicit.eval("2(3)").is_err()); // UnexpectedToken

// 10. Significant Figures (fixed or input-following)
let calc_sig = Abacus::standard().with_significant_figures(3);
println!("{}", calc_sig.eval("12.3456 m").unwrap()); // 12.3 m

let calc_follow = Abacus::standard().with_follow_significant_figures(true);
println!("{}", calc_follow.eval("12.3 * 4.567").unwrap()); // 56.2 (3 sig figs)

// 11. Toggle Automatic Derived SI Unit Reduction
let calc_raw = Abacus::standard().with_auto_derived_units(false);
println!("{}", calc_raw.eval("10 N * 5 m").unwrap()); // 50 N*m (not reduced to J)

// 12. Currencies with a custom rate override (rate = units per 1 USD)
let calc_curr = Abacus::standard()
    .with_currencies(true)
    .with_currency_rate("EUR", 0.50);
println!("{}", calc_curr.eval("$100 in EUR").unwrap()); // 50 EUR

// 13. Live exchange rates (needs the `live-rates` feature; see "Exchange Rates" below)
let calc_live = Abacus::standard()
    .with_currency_cache("/path/to/currency_rates.json")
    .with_live_rates(true);

// 14. Number Scales & Multipliers
let calc_scales = Abacus::standard().with_number_scales(true);
println!("{}", calc_scales.eval("$3 million").unwrap()); // $3000000

// 15. Variables & Mathematical Constants (pi, e, tau, phi)
let mut calc_vars = Abacus::standard();
println!("{}", calc_vars.eval("2 * pi * 5").unwrap()); // 31.41592653589793
let radius = calc_vars.eval("5 m").unwrap();
calc_vars.set_variable("radius", radius);
println!("{}", calc_vars.eval("pi * radius^2").unwrap()); // 78.53981633974483 m^2
calc_vars.eval_mut("height = 10 m").unwrap();
println!("{}", calc_vars.eval("radius * height").unwrap()); // 50 m^2

// 16. Unit Display Overrides (e.g. "mi/h" -> "mph", "km/h" -> "kmph")
let calc_speed = Abacus::standard().with_common_speed_overrides();
println!("{}", calc_speed.eval("60 miles per hour").unwrap()); // 60 mph
println!("{}", calc_speed.eval("100 km / 1 h").unwrap()); // 100 kmph

// 17. Date output format (MMDDYYYY also makes ambiguous NN-NN-YYYY input month-first)
let calc_us = Abacus::standard().with_date_format(DateFormat::MMDDYYYY);
println!("{}", calc_us.eval("08-07-2026").unwrap()); // 08-07-2026 (August 7)
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
|   `1 m in inches`    | `39.37007874015748 in` | Metric to imperial conversion            |
|      `5 km / m`      |         `5000`         | Dimensionless ratio cancellation         |
|     `5 ft 3 in`      |       `5.25 ft`        | Compound measurement (summed)            |
|    `100 °C to °F`    |        `212 °F`        | Affine temperature scale conversion      |
|    `-40 °F in °C`    |        `-40 °C`        | Negative temperatures                    |
|   `20 °C - 15 °C`    |         `5 K`          | Temperature difference                   |
|     `1 kWh in J`     |      `3600000 J`       | Prefixed compound units                  |

#### Unit naming conventions

Some names mean different things in different countries. Abacus uses one meaning and gives the others their own names:

| Name                                    | Meaning                                         | Alternatives                                                        |
| :-------------------------------------- | :---------------------------------------------- | :------------------------------------------------------------------ |
| `pound`, `pounds`, `lb`, `lbs`          | Mass (`5 pounds to kg` → `2.26796185 kg`)       | Currency: `GBP`, `£`, `pound sterling`, `quid`                      |
| `gal`, `qt`, `pt`, `gi`, `fl oz`, `cup` | US customary (`1 gal in L` → `3.785411784 L`)   | Imperial: `imp_gal` (`4.54609 L`), `imp_qt`, `imp_pt`, `imp_gi`, `imp_fl_oz`, `imperial gallon` |
| `ton`                                   | Short ton, 2000 lb (`907.18474 kg`)             | `long_ton` / `long ton` (`1016.0469088 kg`); `t` / `tonne` / `metric ton` (1000 kg) |
| `pt`                                    | Pint                                            | Typographic point: `point` / `typ_pt`                               |
| `cal`                                   | Thermochemical calorie (4.184 J)                | `cal_IT`; `kcal` / `Cal`                                            |
| `hp`                                    | Mechanical horsepower (`745.6998715822702 W`)   | `hp_metric`                                                         |
| `as`                                    | Always the conversion keyword                   | Attoseconds: `attosecond`                                           |

### World Currencies & Conversions

Abacus has 30 currencies. Symbols print as prefixes (`$100`), and each currency shows its usual number of decimals. Results below use the bundled fallback rates, dated 2026-10-20; see [Exchange Rates](#exchange-rates) for live rates.

| Expression                  | Result      | Description                                           |
| :-------------------------- | :---------- | :---------------------------------------------------- |
| `$100`                      | `$100`      | Prefix currency symbol rendering                      |
| `-$50.25`                   | `-$50.25`   | Negative prefix currency                              |
| `$50 + $25`                 | `$75`       | Currency addition                                     |
| `$100 / 3`                  | `$33.33`    | 2-decimal display (cents); full precision is kept     |
| `100 USD in EUR`            | `89.17 EUR` | Cross-currency conversion                             |
| `$50 + €20 in USD`          | `$72.43`    | Mixed currency addition converted to USD              |
| `50 EUR in JPY`             | `8867 JPY`  | 0-decimal display for zero-cent currencies            |
| `20 pounds sterling in USD` | `$26.69`    | Multi-word currency names                             |
| `$100 / 2 hours`            | `50 $/h`    | Dimensional currency rate                             |
| `(10 EUR / L) * 5 L`        | `50 EUR`    | Volumetric price cancellation                         |

### Number Scales, Natural Articles & Division ("per")

Scale words, `a`/`an` and `per` are understood:

| Expression                              | Result          | Description                             |
| :-------------------------------------- | :-------------- | :-------------------------------------- |
| `$3 million`                            | `$3000000`      | Number scale word folding               |
| `5 billion USD / 2 million`             | `$2500`         | Large scale word arithmetic             |
| `a million dollars`                     | `$1000000`      | Indefinite article `"a"` as numeral `1` |
| `a dozen`                               | `12`            | Dozen multiplier                        |
| `a dollar`                              | `$1`            | Singular currency unit                  |
| `an hour in minutes`                    | `60 min`        | Indefinite article `"an"` as `1`        |
| `a thousand dollars per day`            | `1000 $/d`      | `"per"` as division operator `/`        |
| `5 usd a second`                        | `5 $/s`         | `"a"` as rate division operator         |
| `60 miles an hour`                      | `60 mi/h`       | `"an"` as rate division operator        |
| `5 usd per second in 20 days`           | `$8640000`      | Rate accumulation over time duration    |
| `5 usd a second in 20 days`             | `$8640000`      | Accumulation with `"a"` rate syntax     |
| `$50 an hour in 40 hours`               | `$2000`         | Wage accumulation over work hours       |
| `(a thousand dollars per day) * 3 days` | `$3000`         | Time cancellation with rate             |
| `100 meters per second`                 | `100 m/s`       | Physical speed with `per`               |
| `10 meters a second in 5 seconds`       | `50 m`          | Distance accumulation from speed        |
| `60 miles per hour in km/h`             | `96.56064 km/h` | Speed unit conversion with `per`        |
| `$50 per hour * 8 hours`                | `$400`          | Wage calculation                        |
| `100 per 4`                             | `25`            | Dimensionless ratio                     |

`a` and `an` are read as `1` or as "per" only in positions like these. `A` is always the ampere (`10 V / A` → `10 Ω`).

### Percentages

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
| `50% of 200`                       | `100`     | Percentage of a value                          |
| `10 % 3`                           | `1`       | `%` followed by an operand is modulo (floored) |

### Variables & Mathematical Constants

| Expression / Code             | Result                  | Description                             |
| :---------------------------- | :---------------------- | :-------------------------------------- |
| `2 * pi * 5`                  | `31.41592653589793`     | Standard constant `pi` / `PI`           |
| `e^2`                         | `7.3890560989306495`    | Euler's number `e` / `E`                |
| `tau`                         | `6.283185307179586`     | Circle constant $\tau = 2\pi$           |
| `phi`                         | `1.618033988749895`     | Golden ratio $\phi = (1 + \sqrt{5})/2$  |
| `calc.set_variable("r", 5 m)` | —                       | Programmatic variable definition        |
| `calc.eval("pi * r^2")`       | `78.53981633974483 m^2` | Variable evaluation with physical units |
| `calc.eval_mut("w = 10 m")`   | `10 m`                  | Inline assignment syntax                |

A variable shadows a unit with the same name, so if `h` is defined, both `2h` and `2 h` use the variable. Conversion targets after `in` or `to` always refer to units.

### Unit Display Overrides

Composite units can be displayed under a different name:

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
| `60 miles per hour in km/h`  | `96.56064 km/h`      | `96.56064 kmph`    | Speed conversion with target override          |
| `[50 mi/h, 70 mi/h]`         | `[50 mi/h, 70 mi/h]` | `[50 mph, 70 mph]` | Interval endpoints formatting                  |
| `a thousand dollars per day` | `1000 $/d`           | `1000 $/day`       | Custom rate override (`"$/d"` $\to$ `"$/day"`) |

### Interval Arithmetic

Write intervals as `[a, b]` or `a..b`:

```text
[1 m, 2 m] + [3 m, 4 m]   -> [4 m, 6 m]
1..10 + 5                 -> 6..15
[10 m, 20 m] - [2 m, 5 m] -> [5 m, 18 m]
[9.8 m, 10.2 m] / 2 s     -> [4.9 m/s, 5.1 m/s]
[-2 m, 4 m] * [1 s, 3 s]  -> [-6 m*s, 12 m*s]
[-2, 3]^2                 -> [0, 9]
1 / [-1, 2]               -> [-inf, inf]
[0.1, 0.1] + [0.2, 0.2]   -> [0.3, 0.30000000000000004]
```

For `+`, `-`, `*`, `/` and powers, each result contains the exact real-number result of that one operation on the input bounds; inexact bounds are rounded outward by one ulp. Operations don't know when two intervals are the same quantity, so `[1, 2] - [1, 2]` is `[-1, 1]`, not `[0, 0]`, and using a variable more than once widens the result. Functions of one argument are evaluated at the endpoints, which is only correct for monotonic functions. Inside function calls, put compound range endpoints in parentheses: `max(1..(2*3))`.

### Dates, Relative Time & Business Calendars

These outputs assume the local time is Tuesday, September 1, 2026 09:00. `Abacus::with_now` sets the clock.

```text
// Date arithmetic
07-08-2026 + 3 weeks               -> August 28, 2026
07-08-2026 + 5 business days       -> August 14, 2026
10-08-2026 - 1 business day        -> August 7, 2026
workdays(07-08-2026, 14-08-2026)   -> 5 workdays
2026-01-31 + 1 month               -> February 28, 2026
2024-02-29 + 1 year                -> February 28, 2025
17-08-2026 - 07-08-2026            -> 10 d
1 year in days                     -> 365.2425 d

// Relative time & natural language
now                                -> September 1, 2026 09:00:00
now + 2 hours                      -> September 1, 2026 11:00:00
today                              -> September 1, 2026
today at 12 to 1                   -> 1 h
11pm to 1am                        -> 2 h
last thursday at 3pm + 2 weeks     -> September 10, 2026 15:00:00
3 days ago                         -> August 29, 2026 09:00:00
in 45 minutes                      -> September 1, 2026 09:45:00
next friday                        -> September 4, 2026

// Timezones & AM/PM
15:30:00 EST to PST                -> September 1, 2026 12:30:00 PST
3:30 PM + 45 minutes               -> September 1, 2026 16:15:00
07-08-2026 10:00 AM EST to PST     -> August 7, 2026 07:00:00 PST
2026-08-07 10:00 +0530 in UTC      -> August 7, 2026 04:30:00 UTC
```

How dates are read:

- `now` is the current instant and `today` is local midnight. The local UTC offset, including DST, comes from the `TZ` environment variable or `/etc/localtime`, and falls back to UTC.
- `YYYY-MM-DD` is always year-month-day. `NN-NN-YYYY` is day-month-year by default, or month-day-year when the engine uses `DateFormat::MMDDYYYY`, so output can be read back in. Invalid dates such as `2026-02-30` are errors.
- Adding `month(s)`/`mo` or `year(s)`/`yr` to a date moves by calendar months, clamped to the end of the month. Converted to fixed units, they use Gregorian averages (year = 365.2425 d, month = year / 12). Subtracting two dates gives whole days in `d` when exact, otherwise `h`, `min` or `s`.
- `workdays(a, b)` counts business days after `a` up to and including `b`, matching `a + n business days`. It is negative when `b` is before `a`.
- `next X` and `last X` are the nearest occurrence strictly after or before today. `this X` is X in the current Monday-to-Sunday week.
- Timezone abbreviations are fixed offsets and DST is not inferred, so write `EDT` for US Eastern summer time. IST is India (+05:30), CST is US Central (−06:00) and BST is British Summer Time (+01:00). Numeric offsets (`+02:00`, `-0400`) must be within ±14:00.

### Events and Named Dates

Same clock as above:

| Expression                                            | Example Result          | Description                                     |
| :---------------------------------------------------- | :---------------------- | :---------------------------------------------- |
| `days until christmas`                                | `115 d`                 | Calendar countdown to upcoming annual holiday   |
| `business days until end of quarter`                  | `21 business_days`      | Business days remaining in the current quarter  |
| `days until end of quarter`                           | `29 d`                  | Calendar days until current quarter closes      |
| `third thursday of november 2026`                     | `November 19, 2026`     | Nth occurrence of a weekday in a month          |
| `last friday of october 2026`                         | `October 30, 2026`      | Last occurrence of a weekday in a month         |
| `first monday of january 2027`                        | `January 4, 2027`       | 1st occurrence of a weekday in a month          |
| `days until third thursday of november 2026`          | `79 d`                  | Calendar duration to specific ordinal event     |
| `business days until third thursday of november 2026` | `57 business_days`      | Business days to specific ordinal event         |
| `end of quarter`                                      | `September 30, 2026`    | Current quarter end date                        |
| `start of quarter`                                    | `July 1, 2026`          | Current quarter start date                      |
| `end of next quarter`                                 | `December 31, 2026`     | Next quarter closing date                       |
| `end of q1 2027`                                      | `March 31, 2027`        | Specific quarter closing date                   |
| `end of month`                                        | `September 30, 2026`    | Last day of the current month                   |
| `end of year`                                         | `December 31, 2026`     | Last day of the current calendar year           |
| `thanksgiving 2026`                                   | `November 26, 2026`     | 4th Thursday of November                        |
| `black friday 2026`                                   | `November 27, 2026`     | Day following Thanksgiving                      |
| `cyber monday 2026`                                   | `November 30, 2026`     | Monday following Thanksgiving                   |
| `easter 2026`                                         | `April 5, 2026`         | Computus algorithm for Easter Sunday            |
| `until christmas`                                     | `115 d`                 | Prefix `until` defaulting to calendar days      |
| `(days until christmas) in weeks`                     | `16.428571428571427 wk` | Unit conversion of event countdown duration     |

### Statistics & Probability Distributions

```text
// Descriptive statistics on ranges or arguments
mean(1..10)                        -> 5.5
median(10, 20, 30, 40, 50)         -> 30
std(1..9..2)                       -> 3.1622776601683795
std_p(1..9..2)                     -> 2.8284271247461903
iqr(1..100)                        -> 49.5
skew(1, 2, 3, 10)                  -> 1.7636326148038877
mean(10 degC, 20 degC)             -> 15 °C
std(10 degC, 20 degC)              -> 7.0710678118654755 K

// Distributions
normpdf(0, 0, 1)                   -> 0.39894228040143265
normcdf(1.96, 0, 1)                -> 0.9750021048517794
invnorm(0.975, 0, 1)               -> 1.9599639845400538
poissonpdf(3, 2)                   -> 0.22404180765538775
poissonpdf(200, 180)               -> 0.010556778614080506
binompdf(2000, 0.5, 1000)          -> 0.017839011145854337
chisqcdf(1e6, 1e6)                 -> 0.5001880631966019
```

- `std`/`var` are sample statistics; `std_p`/`var_p` are population statistics.
- `iqr` and quantiles use linear interpolation (Excel `QUARTILE.INC`). `skew`/`kurt` are the sample-adjusted forms used by Excel's `SKEW`/`KURT`.
- The spread of a set of temperatures (std, range and so on) is a temperature difference, so it is given in K for °C readings and °R for °F readings, as with `30 °F - 20 °F` = `10 °R`.
- Parameters that must be integers (counts, `n`, factorial and nCr arguments) are errors when they are not whole numbers. They are not rounded.
- Distribution functions have a relative error of about 1e-13 or better for ordinary parameters. The CDFs built on the incomplete beta and gamma functions (`fcdf`, `binomcdf`, `hypgeomcdf`, `poissoncdf`) lose precision slowly as parameters grow: about 1e-13 up to ~1e6, 1e-12 to 1e-10 around 1e8 to 1e12, and about 1e-8 at 1e15 (`binomcdf(1e15, 0.5, 5e14)`). Student's t (`tcdf`, `invt`, `tpdf`) and `chisqcdf` stay accurate at any degrees of freedom; the worst error observed is ~3e-13, for `chisqcdf` near 1e9. Inputs that cannot be evaluated return an error, not a wrong value.

### Hypothesis Testing & Confidence Intervals

Tests follow the TI-84 and return a hash with the test statistic, the two-sided p-value (`p` or `p_value`) and the sample estimates. Each function has one signature, so summary-statistics ("Stats") and raw-data ("Data") versions have different names.

| Function                          | Arguments                               | Notes                         |
| :-------------------------------- | :-------------------------------------- | :---------------------------- |
| `ZTest` = `ZTestStats`            | `(μ₀, x̄, σ, n)`                         |                               |
| `ZTestData`                       | `(μ₀, σ, x₁, x₂, …)`                    |                               |
| `TTest` = `TTestStats`            | `(μ₀, x̄, s, n)`                         |                               |
| `TTestData`                       | `(μ₀, x₁, x₂, …)`                       |                               |
| `1-PropZTest`                     | `(p₀, x, n)`                            |                               |
| `2-SampZTest`                     | `(σ₁, σ₂, x̄₁, n₁, x̄₂, n₂)`              |                               |
| `2-SampTTest`                     | `(x̄₁, s₁, n₁, x̄₂, s₂, n₂)`              | Welch's df                    |
| `2-PropZTest`                     | `(x₁, n₁, x₂, n₂)`                      | Pooled proportion             |
| `Chi2Test`                        | `(o₁, …, oₖ, e₁, …, eₖ)`                | Goodness of fit, df = k − 1   |
| `TInterval` = `TIntervalData`     | `(x₁, x₂, …, [C%])`                     |                               |
| `TIntervalStats`                  | `(x̄, s, n, [C])`                        |                               |
| `ZInterval` = `ZIntervalStats`    | `(x̄, σ, n, [C])`                        |                               |
| `ZIntervalData`                   | `(σ, x₁, x₂, …, [C%])`                  |                               |
| `1-PropZInt`                      | `(x, n, [C])`                           | Wald, as on the TI-84         |
| `2-SampTInt`                      | `(x̄₁, s₁, n₁, x̄₂, s₂, n₂, [C])`         | Welch's df                    |
| `2-SampZInt`                      | `(x̄₁, σ₁, n₁, x̄₂, σ₂, n₂, [C])`         |                               |
| `2-PropZInt`                      | `(x₁, n₁, x₂, n₂, [C])`                 | Wald                          |
| `moe`                             | `(x₁, x₂, …, [C%])`                     | t-based margin of error       |
| `tmoe` / `zmoe`                   | `(s, n, [C])` / `(σ, n, [C])`           |                               |

The confidence level defaults to 95%. Stats forms accept `0.9` or `90%`. Data forms accept only a percentage (`TInterval(10 m, 12 m, 11 m, 14 m, 90%)`), so a data value between 0 and 1 is never taken as a confidence level.

```text
ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)
  -> { mean: 13 m, n: 4, p: 0.0026997960632601905, p_value: 0.0026997960632601905, z: 3 }

ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m).p_value -> 0.0026997960632601905
TTest(100 m, 105 m, 15 m, 25).p                      -> 0.10858012302472296
TInterval(10 m, 12 m, 11 m, 14 m)                    -> [9.032469116203984 m, 14.467530883796016 m]
TInterval(10 m, 12 m, 11 m, 14 m, 90%)               -> [9.74043339576468 m, 13.75956660423532 m]
ZInterval(100 m, 15 m, 100)                          -> [97.06005402318992 m, 102.93994597681008 m]
1-PropZInt(45, 100)                                  -> [0.3524930229100607, 0.5475069770899393]
```

### Dimension-Aware Linear Regression

`linreg(x₁, …, xₙ, y₁, …, yₙ)` keeps units on the slope, intercept, standard errors and predictions. `se` is the residual standard error; `slope_se` and `intercept_se` are the standard errors of the coefficients.

```text
linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m)
  -> { intercept: 0 m, intercept_se: 0 m, mean_x: 2.5 s, mean_y: 25 m, r: 1, r2: 1, se: 0 m, slope: 10 m/s, slope_se: 0 m/s }

linreg(1 s, 2 s, 3 s, 4 s, 10 m, 20 m, 30 m, 40 m).slope * 10 s -> 100 m
predict(5, 1, 2, 3, 2, 4, 6)                                       -> 10
```

`predict(x_target, x…, y…)` takes the target first; `linreg_predict(x…, y…, x_target)` takes it last.

### Financial Functions

These use Excel's sign conventions. Like Excel's `NPV`, `npv` discounts the first cash flow.

```text
pmt(0.05/12, 360, 200000)          -> -1073.6432460242781
npv(0.1, -100, 50, 60)             -> -4.507888805409458
irr(-100, 60, 60)                  -> 0.13066238629180754
```

`irr` reports an error when the cash flows have no sign change or the solver does not converge; with several roots it returns the one reached from a 10% starting guess.

---

## Exchange Rates

With the `currencies` feature, conversions use bundled fallback rates dated `currency::FALLBACK_RATES_DATE`. `currency::is_using_fallback_rates(calc.units())` tells you whether they are still in use. Abacus does not use the network or disk unless you ask it to:

- `with_live_rates(true)` and `update_daily_rates()` fetch rates from the Frankfurter API (`https://api.frankfurter.dev/v1/latest?base=USD`) by running the system `curl` over HTTPS (`--fail --proto =https --tlsv1.2`, 4 s connect timeout, 8 s total). This needs the `live-rates` feature, which is off by default; without it they only load a cache file if it is fresh. The response must have a USD base and finite positive rates. The fetch blocks. `try_set_live_rates` returns errors instead of ignoring them.
- Fetched rates are cached on disk and reused while the file is less than 24 hours old and its ECB reference date is recent. The file is written to a temporary path and renamed into place. The cache path is the first of: the path passed to `with_currency_cache`, `ABACUS_CURRENCY_CACHE`, `$ABACUS_CACHE_DIR/currency_rates.json`, `$XDG_CACHE_HOME/abacus/currency_rates.json`, `~/.cache/abacus/currency_rates.json` (`%LOCALAPPDATA%\abacus\…` on Windows), and a private (`0700`) per-user directory under the system temp directory.
- `update_rates_from_json(json)` loads rates you fetched yourself. It writes to disk only if a cache path was set with `with_currency_cache`.

---

## Command-Line Usage

The `abacus` binary evaluates each argument, or each line of stdin when no arguments are given:

```sh
$ abacus "10 N * 5 m" "1 km in m"
50 J
1000 m

$ printf '2+2\n5 ft 3 in\n' | abacus
4
5.25 ft

$ abacus -- "-5 + 2"      # use -- before an expression starting with '-'
-3

$ abacus "1 m + 1 s"; echo $?
error: cannot perform operation on incompatible dimensions
  1 m + 1 s
      ^
  note: m + s
1
```

Variables assigned with `name = expr` carry over to later expressions. Options configure the engine:

```sh
$ abacus --degrees --decimals 3 "sin 40"
0.643
$ abacus --notation eng --date-format iso "45000 m" "07-08-2026 + 1 day"
45e3 m
2026-08-08
```

The options are `--degrees`/`--radians`/`--gradians`, `--decimals N`, `--sig-figs N`, `--notation standard|scientific|engineering`, `--timezone TZ`, `--date-format long|iso|us|eu`, `--strict`, `--no-implicit`, `--exact` (no cancellation snapping), `--demo` (prints a tour of the features), `--help` and `--version`.

### REPL

```sh
cargo run --bin repl --features=repl
```

The REPL takes the same options as `abacus` and saves its history. Highlighting and Tab completion use the engine's own parser, so a name is coloured as a unit, variable or function the same way it will be evaluated, and unknown names are underlined as you type. `ans` is the last result.

```text
› x = 5 m
5 m
› x + 3 s
    ^
error: cannot perform operation on incompatible dimensions
note: m + s
› :set decimals 2
› ans / 3
1.67 m
```

Commands: `:set` (show options) and `:set NAME VALUE` (`angle`, `decimals`, `sig-figs`, `notation`, `timezone`, `date-format`, `strict`, `implicit`, `exact`), `:vars [all]`, `:unset NAME`, `:reset`, `:clear`, `:help`, `:quit`. Ctrl-C clears the current line; Ctrl-D exits. Set `NO_COLOR` to turn colours off.

---

## Cargo Features

```toml
[dependencies]
abacus = { git = "https://github.com/dylnfraser/abacus", default-features = false, features = ["units"] }
```

|     Feature     | Description                                                                         | Default |
| :-------------: | :---------------------------------------------------------------------------------- | :------ |
|     `units`     | Physical unit registry and conversions (needed for currency and scale-word units)   | **Yes** |
|  `currencies`   | 30 world currencies, prefix formatting, bundled fallback rates, and conversions     | **Yes** |
|  `live-rates`   | Live exchange rates downloaded via the system `curl`, with daily disk caching       | No      |
| `number-scales` | Scale words (`million`, `billion`) and indefinite-article evaluation                | **Yes** |
|     `stats`     | Descriptive statistics, hypothesis tests, confidence intervals, linear regression   | **Yes** |
| `distributions` | Continuous & discrete probability distributions                                     | **Yes** |
|     `date`      | Date functions (`year()`, `workdays()`, `date()`, …)                                | **Yes** |
|   `financial`   | Financial functions (`pmt`, `npv`, `irr`, `fv`, `pv`, `compound`)                   | **Yes** |
|     `repl`      | Interactive CLI REPL with history and syntax highlighting (`rustyline`)             | No      |
|     `serde`     | `Serialize`/`Deserialize` on core types (`Value`, `Interval`, `Date`, `Hash`)       | No      |

---

## Benchmarks

```sh
cargo bench --bench eval_bench            # full Criterion run
cargo bench --bench eval_bench -- --test  # smoke test
```

The `release` profile uses `opt-level = 3`; a size-optimised `release-small` profile is also provided (`cargo build --profile release-small`).

---

## Testing & Quality

```sh
cargo test --all-features                               # unit, integration and doc tests
cargo test --no-default-features --features stats       # any feature subset also passes
cargo clippy --all-targets --all-features -- -D warnings
```

`tests/readme_examples_tests.rs` checks every example output in this README against the engine.

Fuzzing uses [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) on a nightly toolchain. There are two targets: `fuzz_eval` takes raw expressions and `fuzz_eval_config` also varies the engine settings. `fuzz/abacus.dict` lists expression keywords for the fuzzer:

```sh
cargo install cargo-fuzz
cd fuzz
cargo +nightly fuzz run fuzz_eval -- -dict=abacus.dict -max_len=256 -timeout=2
```

## License

MIT. See [LICENSE](LICENSE).
