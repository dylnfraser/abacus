# Changelog

## Unreleased

Results of a full correctness, performance and robustness audit. Every fix has a regression test (`tests/audit_*_tests.rs`), and every README example is now checked by `tests/readme_examples_tests.rs`.

### Breaking / behaviour changes

**Units**
- `pound`/`pounds`/`lbs` now mean mass (lb). Use `GBP`, `£`, `quid` or `pound sterling` for the currency.
- `gal`, `gallon`, `qt`, `pt`, `gi`, `fl oz`/`floz` are now **US customary**. Imperial measures are `imp_gal`, `imp_qt`, `imp_pt`, `imp_gi`, `imp_fl_oz` (or `imperial gallon`, …).
- `ton` is the short ton (2000 lb). The long ton is `long_ton` / `long ton`; `t` / `tonne` / `metric ton` is 1000 kg.
- The typographic point displays as `point` (keys `point`, `typ_pt`), since `pt` is the pint.
- `as` is always the conversion keyword and is no longer the attosecond (use `attosecond`).
- A unit you typed is kept as written (`1 atm`, `3 GHz`, `1 mA`). Only composite results are reduced to derived units. Ambiguous reductions (Gy, Sv, Bq, lm, lx, and Hz for `rad/s`) are no longer applied automatically.
- `A` is always the ampere. `a`/`an` act as `1` or "per" only in conversational positions (`a million`, `5 usd a second`).

**Parsing**
- Adjacent bare numbers (`1 000`, `2 3`, `1.2.3`) are errors. Same-dimension compound measurements are summed (`5 ft 3 in` = 5.25 ft, `1 h 30 min` = 1.5 h).
- `3--2` = 5 and `2++3` = 5. `++`/`--` only increment/decrement before an operand or at the end of an expression.
- `%` is modulo exactly when an operand follows, regardless of spacing (`10%(3)` = 1). Modulo is **floored**: `-7 % 3` = 2.
- Variables shadow units consistently, with or without a space (`2h`). Conversion targets always refer to units.
- Conversions that don't make dimensional sense are errors (`5 m/s in s`, `1 kHz in ms`).
- `1e999` and `4e` are `InvalidNumber` errors; use `4 e` for 4·e.
- Compound range endpoints inside function calls must be parenthesised (`max(1..(2*3))`).
- `with_max_recursion_depth` is clamped to 256.
- `Token::DotProperty` now borrows `&str`.

**Numbers & display**
- Values below `1e-12` no longer display as `0`. Very small and very large magnitudes use scientific notation (`1.6e-19 C`, `1e300 m`).
- Float noise in the last couple of bits is hidden (`0.1 + 0.2` → `0.3`, `5 lb to kg` → `2.26796185 kg`). Interval bounds still print exactly.
- Denominator-only units display (`1/(2 m)` → `0.5 1/m`). Powers simplify (`(4 m^2)^0.5` → `2 m`, `m^9 * m` → `m^10`).
- Rounding options (decimal places, significant figures, currency decimals) only affect display. Stored variables keep full precision: `x = 1 USD / 3; x*3` → `$1`.
- Interval bounds are rounded outward when inexact. `[-2,3]^2` → `[0, 9]`, and `per` division by an interval containing zero → `[-inf, inf]`.

**Temperatures**
- `-10 °C` and `-40 °F` are accepted.
- `°C − °C` gives a K difference.
- `°C + °C` and powers of °C are errors.
- In statistics, spreads (std, var, range, mad, iqr, moe, …) of temperatures are reported in K; locations (mean, median, CI bounds) stay in the input unit.

**Dates & times**
- `now` is the current local instant and `today` is local midnight. The local offset, including DST, comes from `TZ` or `/etc/localtime`, falling back to UTC.
- A configured default timezone converts the instant instead of relabelling UTC.
- `month(s)`/`mo` and `year(s)`/`yr` are durations: calendar months when added to dates, Gregorian averages when converted.
- Invalid date literals (`2026-02-30`, `32-01-2026`) are errors instead of being evaluated as subtraction.
- `Date - Date` and `a to b` give whole days in `d` when exact.
- `3 pm` / `5 am` after a bare number are times.
- `11pm to 1am` = 2 h. Explicit AM/PM is respected, and spans roll over midnight.
- With `DateFormat::MMDDYYYY`, ambiguous `NN-NN-YYYY` input is month-first.
- Dates compare by instant.
- Timezone offsets are range-checked (±14:00), and `±HHMM` is accepted.

**Statistics & hypothesis tests**
- Each test/interval name has one signature; summary and raw-data forms are separate:
  - `ZTest`/`ZTestStats(μ₀, x̄, σ, n)` and `ZTestData(μ₀, σ, x…)`.
  - `TTest`/`TTestStats(μ₀, x̄, s, n)` and `TTestData(μ₀, x…)`.
  - `TInterval`/`TIntervalData(x…, [C%])` and `TIntervalStats(x̄, s, n, [C])`.
  - `ZInterval`/`ZIntervalStats(x̄, σ, n, [C])` and `ZIntervalData(σ, x…, [C%])`.
- Data forms accept a confidence level only as a percentage (`90%`).
- `1-PropZInt` uses the Wald interval, as on the TI-84.
- `skew`/`kurt` use the sample-adjusted Excel `SKEW`/`KURT` formulas.
- `mode` of all-distinct data is an error.
- Non-integer counts (`factorial(2.5)`, `nCr(5, 2.5)`, `binompdf(10, .5, 2.5)`) are errors instead of being rounded.
- `asin(2)`, `acosh(0.5)` and `tan(90 deg)` are domain errors. `asinh`/`acosh`/`atanh` return plain numbers.
- `linreg` adds `slope_se` and `intercept_se`; `se` is the residual standard error.
- `predict(x_target, x…, y…)` has a fixed argument order.

**Currencies**
- `Abacus::standard()` never writes to disk on its own. `update_rates_from_json` writes only when a cache path was configured.
- Fallback rates are refreshed (2026-10-20). `FALLBACK_RATES_DATE` and `is_using_fallback_rates` are new.

**Tooling**
- The `abacus` binary evaluates its arguments or stdin lines and exits with status 1 on error. The old demo is behind `--demo`.
- The REPL supports assignment, accepts `help`/`.help`, and Ctrl-C clears the line.
- The release profile uses `opt-level = 3` (the `release-small` profile keeps `opt-level = "z"`). `panic = "abort"` was removed.
- `license = "MIT"` and a LICENSE file were added.

### Fixes

**Crashes and denial-of-service**
- Dimension exponent overflow (`1 m^250 * 1 m^250`) and `lcm` overflow no longer panic; they return errors.
- Lexing was O(n²); 80k characters went from ~5 s to ~6 ms.
- `poissoncdf`/`binomcdf`/`hypgeomcdf` with huge arguments no longer hang (they now use closed-form CDFs).
- Memory blow-ups are prevented: total range elements are capped per evaluation, and the exponent-unit cache is bounded.
- Deep nesting no longer overflows the stack (stack-headroom guard in the parser).
- `abacus --demo | head` no longer panics on a broken pipe.

**Numerical accuracy**
- `erf`/`erfc`, `normcdf` (including far tails) and `invnorm` are now accurate to near full double precision. `invnorm(0.975)` was 1.95905 and is now 1.959963984540054.
- Incomplete gamma/beta now converge for large arguments and report non-convergence (`chisqcdf(1e6, 1e6)` was 0.11, now 0.50019).
- `invt`/`invchisq` are robust in the tails.
- Binomial and hypergeometric are computed in log space, so they no longer overflow.
- `irr` uses a relative tolerance with a fallback solver.
- `fpdf`/`chisqpdf` are correct at x = 0.

**Silently wrong results**
- `mod` with mixed units (`mod(1 m, 30 cm)` = 0.1 m).
- Ambiguous `TTest`/`ZTest`/`TInterval` call forms.
- A trailing data value was taken as the confidence level.
- Percent arithmetic (`50% / 2 s`, `(5%)^2`, `10% - 3`).
- Fractional exponent round-trips (`((2 m)^(1/7))^7`).
- `m^NaN`, and affine bases in exponent units.
- Degenerate standard errors in z-tests.

**Dates**
- `today at 12 to 1`.
- `May 16 10:00` and `christmas 10:00`.
- Holiday "this year vs next" boundary.
- Standalone times use the anchor date.
- `+0530` offsets.
- Saturating casts for huge durations.
- Fractional business days.

**Performance**
- The operator index is cached: `eval("1+1")` is ~20× faster.
- Fewer allocations in arithmetic and display hot paths.
- The clock is read once per evaluation.

**Currency security**
- `curl` is run with `--fail --proto =https --tlsv1.2` and a size cap.
- Responses are validated.
- Cache writes are atomic.
- The temp-dir fallback is private.
- Freshness checks the data's date.

**Tests**
- Tests no longer write to the user's real currency cache.
- Date tests no longer depend on the wall clock.
- Tests pass under every feature combination.
- Loose tolerances were tightened (e.g. `invnorm` from 1e-3 to 1e-14).

**Documentation**
- The README is rewritten to match actual output, and its examples are now tested.
- Inaccurate claims are removed: zero-allocation, `[i16; 8]`/SIMD dimensions, zero-dependency.
- Interval guarantees, date semantics and unit-naming conventions are documented.
