//! Special functions used by the probability distributions.
//!
//! Everything here targets close to full double precision. Iterative routines
//! (series and continued fractions) size their iteration budget from their
//! arguments and report non-convergence as `None` instead of returning a
//! partially converged value.

use std::f64::consts::{FRAC_2_SQRT_PI, PI, SQRT_2};

const LN_SQRT_2PI: f64 = 0.918_938_533_204_672_8;
const TINY: f64 = 1e-300;
const EPS: f64 = 1e-16;

/// Computes n! as f64. Returns 1.0 for n=0 and `inf` once the result overflows.
#[must_use]
pub fn factorial(n: u64) -> f64 {
    if n > 170 {
        return f64::INFINITY;
    }
    (1..=n).fold(1.0, |acc, x| acc * (x as f64))
}

// ── Error function ──

/// erf(x) for |x| < 2.5 via the non-alternating series
/// `erf(x) = 2/√π · e^{-x²} · Σ (2x²)^n x / (1·3·…·(2n+1))`, which has no cancellation.
fn erf_series(x: f64) -> f64 {
    let x2 = x * x;
    let mut term = x;
    let mut sum = x;
    let mut n = 0.0;
    while term.abs() > sum.abs() * EPS {
        n += 1.0;
        term *= 2.0 * x2 / (2.0 * n + 1.0);
        sum += term;
    }
    FRAC_2_SQRT_PI * (-x2).exp() * sum
}

/// erfc(x) for x ≥ 0.5 via the Laplace continued fraction (modified Lentz).
fn erfc_cf(x: f64) -> f64 {
    // erfc(x) = e^{-x²}/√π · 1/(x + (1/2)/(x + 1/(x + (3/2)/(x + 2/(x + …)))))
    let mut f = x;
    let mut c = x;
    let mut d = 0.0;
    for n in 1..5000 {
        let an = f64::from(n) * 0.5;
        d = x + an * d;
        d = if d.abs() < TINY { 1.0 / TINY } else { 1.0 / d };
        c = x + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < EPS {
            break;
        }
    }
    (-x * x).exp() / (f * PI.sqrt())
}

/// Error function, accurate to a few ulps.
#[must_use]
pub fn erf(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x.abs() < 2.5 {
        erf_series(x)
    } else if x > 0.0 {
        1.0 - erfc_cf(x)
    } else {
        erfc_cf(-x) - 1.0
    }
}

/// Complementary error function `1 - erf(x)`, accurate in the far tail.
#[must_use]
pub fn erfc(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x < 0.5 {
        1.0 - erf(x)
    } else if x > 27.3 {
        0.0
    } else {
        erfc_cf(x)
    }
}

/// Standard normal CDF Φ(z), computed via `erfc` so both tails keep relative precision.
#[must_use]
pub fn norm_cdf(z: f64) -> f64 {
    0.5 * erfc(-z / SQRT_2)
}

/// Standard normal survival function `1 - Φ(z)`.
#[must_use]
pub fn norm_sf(z: f64) -> f64 {
    0.5 * erfc(z / SQRT_2)
}

/// Standard normal density.
#[must_use]
pub fn norm_pdf(z: f64) -> f64 {
    (-0.5 * z * z - LN_SQRT_2PI).exp()
}

/// Inverse of the lower-tail normal CDF for `q ≤ 0.5` (result ≤ 0).
fn norm_ppf_lower(q: f64) -> f64 {
    // Acklam's rational approximation (relative error ~1.15e-9) ...
    const A: [f64; 6] = [
        -3.969_683_028_665_376e1,
        2.209_460_984_245_205e2,
        -2.759_285_104_469_687e2,
        1.383_577_518_672_69e2,
        -3.066_479_806_614_716e1,
        2.506_628_277_459_239,
    ];
    const B: [f64; 5] = [
        -5.447_609_879_822_406e1,
        1.615_858_368_580_409e2,
        -1.556_989_798_598_866e2,
        6.680_131_188_771_972e1,
        -1.328_068_155_288_572e1,
    ];
    const C: [f64; 6] = [
        -7.784_894_002_430_293e-3,
        -3.223_964_580_411_365e-1,
        -2.400_758_277_161_838,
        -2.549_732_539_343_734,
        4.374_664_141_464_968,
        2.938_163_982_698_783,
    ];
    const D: [f64; 4] = [
        7.784_695_709_041_462e-3,
        3.224_671_290_700_398e-1,
        2.445_134_137_142_996,
        3.754_408_661_907_416,
    ];
    let mut x = if q < 0.02425 {
        let r = (-2.0 * q.ln()).sqrt();
        (((((C[0] * r + C[1]) * r + C[2]) * r + C[3]) * r + C[4]) * r + C[5])
            / ((((D[0] * r + D[1]) * r + D[2]) * r + D[3]) * r + 1.0)
    } else {
        let s = q - 0.5;
        let r = s * s;
        (((((A[0] * r + A[1]) * r + A[2]) * r + A[3]) * r + A[4]) * r + A[5]) * s
            / (((((B[0] * r + B[1]) * r + B[2]) * r + B[3]) * r + B[4]) * r + 1.0)
    };
    // ... refined to full precision with Halley steps against the accurate CDF.
    for _ in 0..3 {
        let pdf = norm_pdf(x);
        if pdf == 0.0 {
            break;
        }
        let u = (norm_cdf(x) - q) / pdf;
        let step = u / (1.0 + 0.5 * x * u);
        x -= step;
        if step.abs() <= 1e-15 * x.abs() {
            break;
        }
    }
    x
}

/// Inverse standard normal CDF (quantile function). Valid for `0 < p < 1`.
#[must_use]
pub fn norm_ppf(p: f64) -> f64 {
    if !(0.0..=1.0).contains(&p) {
        return f64::NAN;
    }
    if p == 0.0 {
        return f64::NEG_INFINITY;
    }
    if p == 1.0 {
        return f64::INFINITY;
    }
    if p <= 0.5 {
        norm_ppf_lower(p)
    } else {
        // 1 - p is exact for p ≥ 0.5 (Sterbenz), so the upper tail keeps full precision.
        -norm_ppf_lower(1.0 - p)
    }
}

/// Inverse error function, derived from the normal quantile.
#[must_use]
pub fn erfinv(x: f64) -> f64 {
    if x <= -1.0 {
        return f64::NEG_INFINITY;
    }
    if x >= 1.0 {
        return f64::INFINITY;
    }
    norm_ppf(0.5 * (x + 1.0)) / SQRT_2
}

// ── Gamma function family ──

/// Stirling-series correction `ln Γ(x) - [(x - ½) ln x - x + ½ ln 2π]`, for x ≥ 10
/// (seven terms; the truncation error is below 1e-16 there).
fn stirling_correction(x: f64) -> f64 {
    const C: [f64; 7] = [
        1.0 / 12.0,
        -1.0 / 360.0,
        1.0 / 1260.0,
        -1.0 / 1680.0,
        1.0 / 1188.0,
        -691.0 / 360_360.0,
        1.0 / 156.0,
    ];
    let inv_x2 = 1.0 / (x * x);
    C.iter().rev().fold(0.0, |acc, &c| acc * inv_x2 + c) / x
}

/// Natural log of |Γ(x)|. Returns `inf` at the poles (0, -1, -2, …).
#[must_use]
pub fn lgamma(x: f64) -> f64 {
    if x.is_nan() {
        return f64::NAN;
    }
    if x <= 0.0 && x == x.floor() {
        return f64::INFINITY;
    }
    if x < 0.5 {
        // Reflection: Γ(x)Γ(1-x) = π / sin(πx)
        return PI.ln() - (PI * x).sin().abs().ln() - lgamma(1.0 - x);
    }
    if x >= 10.0 {
        return (x - 0.5) * x.ln() - x + LN_SQRT_2PI + stirling_correction(x);
    }
    const P: [f64; 8] = [
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    let z = x - 1.0;
    let mut base = 0.999_999_999_999_809_9;
    for (i, &val) in P.iter().enumerate() {
        base += val / (z + (i + 1) as f64);
    }
    let t = z + 7.5;
    LN_SQRT_2PI + (z + 0.5) * t.ln() - t + base.ln()
}

/// `ln(1 + u) - u`, accurate in absolute terms for all u > -1.
fn log1pmx(u: f64) -> f64 {
    if u.abs() < 1e-4 {
        let u2 = u * u;
        -u2 / 2.0 + u2 * u / 3.0 - u2 * u2 / 4.0
    } else {
        u.ln_1p() - u
    }
}

/// `ln(x^a e^{-x} / Γ(a))`, the common prefactor of the incomplete gamma functions.
fn gamma_log_prefactor(a: f64, x: f64) -> f64 {
    if a >= 10.0 {
        // Avoids cancelling the huge `a ln x`, `x`, and `ln Γ(a)` terms against each other.
        a * log1pmx((x - a) / a) + 0.5 * (a / (2.0 * PI)).ln() - stirling_correction(a)
    } else {
        a * x.ln() - x - lgamma(a)
    }
}

fn iteration_budget(scale: f64) -> u64 {
    // Convergent cases need far fewer iterations than this even at scale 1e15; the cap only
    // bounds hopeless inputs (e.g. `fcdf(1e30, 1e30, 1)`) to ~0.1 s before reporting failure.
    (500.0 + 50.0 * scale.max(0.0).sqrt()).min(1e7) as u64
}

/// Series for P(a, x), used when x < a + 1.
fn gamma_p_series(a: f64, x: f64) -> Option<f64> {
    let mut ap = a;
    let mut sum = 1.0 / a;
    let mut del = sum;
    for _ in 0..iteration_budget(a.max(x)) {
        ap += 1.0;
        del *= x / ap;
        sum += del;
        if del.abs() < sum.abs() * EPS {
            return Some(sum * gamma_log_prefactor(a, x).exp());
        }
    }
    None
}

/// Continued fraction for Q(a, x), used when x ≥ a + 1.
fn gamma_q_cf(a: f64, x: f64) -> Option<f64> {
    let mut b = x + 1.0 - a;
    let mut c = 1.0 / TINY;
    let mut d = 1.0 / b;
    let mut h = d;
    for i in 1..iteration_budget(a.max(x)) {
        let i = i as f64;
        let an = -i * (i - a);
        b += 2.0;
        d = an * d + b;
        if d.abs() < TINY {
            d = TINY;
        }
        c = b + an / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            return Some(gamma_log_prefactor(a, x).exp() * h);
        }
    }
    None
}

/// Shape above which the incomplete gamma functions use Temme's uniform asymptotic expansion
/// (the series/continued fraction would need O(√a) iterations).
const TEMME_THRESHOLD: f64 = 1e9;

/// Temme's uniform asymptotic expansion for Q(a, x), accurate to ~1e-16 for a ≥ 1e9:
/// `Q ≈ ½·erfc(η·√(a/2)) + e^{−aη²/2} / √(2πa) · c₀(η)`, with `λ = x/a`,
/// `η = sign(λ−1)·√(2(λ − 1 − ln λ))` and `c₀(η) = 1/(λ−1) − 1/η`.
/// Returns `(P, Q)`, each computed directly so that both tails keep relative precision.
fn gamma_pq_temme(a: f64, x: f64) -> (f64, f64) {
    let lambda = x / a;
    let half_eta2 = -log1pmx(lambda - 1.0); // λ − 1 − ln λ ≥ 0
    let eta = (2.0 * half_eta2).sqrt().copysign(lambda - 1.0);
    let c0 = if eta.abs() < 1e-3 {
        -1.0 / 3.0 + eta / 12.0 - 2.0 * eta * eta / 135.0 + eta * eta * eta / 864.0
    } else {
        1.0 / (lambda - 1.0) - 1.0 / eta
    };
    let r = (-a * half_eta2).exp() / (2.0 * PI * a).sqrt() * c0;
    let s = eta * (a / 2.0).sqrt();
    (0.5 * erfc(-s) - r, 0.5 * erfc(s) + r)
}

/// Regularized lower incomplete gamma P(a, x). `None` if the expansion fails to converge.
#[must_use]
pub fn gamma_p(a: f64, x: f64) -> Option<f64> {
    if a.is_nan() || a <= 0.0 || x.is_nan() {
        return None;
    }
    if x <= 0.0 {
        return Some(0.0);
    }
    if x.is_infinite() {
        return Some(1.0);
    }
    if a >= TEMME_THRESHOLD {
        return Some(gamma_pq_temme(a, x).0);
    }
    if x < a + 1.0 {
        gamma_p_series(a, x)
    } else {
        gamma_q_cf(a, x).map(|q| 1.0 - q)
    }
}

/// Regularized upper incomplete gamma Q(a, x) = 1 - P(a, x), accurate in the upper tail.
#[must_use]
pub fn gamma_q(a: f64, x: f64) -> Option<f64> {
    if a.is_nan() || a <= 0.0 || x.is_nan() {
        return None;
    }
    if x <= 0.0 {
        return Some(1.0);
    }
    if x.is_infinite() {
        return Some(0.0);
    }
    if a >= TEMME_THRESHOLD {
        return Some(gamma_pq_temme(a, x).1);
    }
    if x < a + 1.0 {
        gamma_p_series(a, x).map(|p| 1.0 - p)
    } else {
        gamma_q_cf(a, x)
    }
}

// ── Beta function family ──

/// `ln(x^a y^b / B(a, b))` with `y = 1 - x`, the prefactor of the incomplete beta continued
/// fraction. Taking `y` separately lets callers pass an exactly computed complement.
fn beta_log_prefactor(a: f64, b: f64, x: f64, y: f64) -> f64 {
    if a >= 10.0 && b >= 10.0 {
        // Expansion around the mode x0 = a / (a + b); see `gamma_log_prefactor`.
        let ab = a + b;
        let x0 = a / ab;
        let y0 = b / ab;
        let dx = if x <= 0.5 { x - x0 } else { y0 - y };
        a * log1pmx(dx / x0) + b * log1pmx(-dx / y0) + 0.5 * (a * b / (2.0 * PI * ab)).ln()
            - stirling_correction(a)
            - stirling_correction(b)
            + stirling_correction(ab)
    } else {
        let ln_x = if x <= 0.5 { x.ln() } else { (-y).ln_1p() };
        let ln_y = if y <= 0.5 { y.ln() } else { (-x).ln_1p() };
        a * ln_x + b * ln_y - ln_beta(a, b)
    }
}

/// ln B(a, b).
#[must_use]
pub fn ln_beta(a: f64, b: f64) -> f64 {
    let (small, big) = if a < b { (a, b) } else { (b, a) };
    if big >= 10.0 {
        // ln Γ(big) − ln Γ(big + small) via Stirling, avoiding the cancellation of two huge
        // log-gamma values: −(big − ½)·ln(1 + small/big) − small·ln(big + small) + small.
        let diff = -(big - 0.5) * (small / big).ln_1p() - small * (big + small).ln()
            + small
            + stirling_correction(big)
            - stirling_correction(big + small);
        lgamma(small) + diff
    } else {
        lgamma(a) + lgamma(b) - lgamma(a + b)
    }
}

/// Continued fraction for the incomplete beta (Numerical Recipes `betacf`), modified Lentz.
fn beta_cf(a: f64, b: f64, x: f64) -> Option<f64> {
    let qab = a + b;
    let qap = a + 1.0;
    let qam = a - 1.0;
    let mut c = 1.0;
    let mut d = 1.0 - qab * x / qap;
    if d.abs() < TINY {
        d = TINY;
    }
    d = 1.0 / d;
    let mut h = d;
    for m in 1..iteration_budget(a.max(b)) {
        let m = m as f64;
        let m2 = 2.0 * m;
        let aa = m * (b - m) * x / ((qam + m2) * (a + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        h *= d * c;
        let aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2));
        d = 1.0 + aa * d;
        if d.abs() < TINY {
            d = TINY;
        }
        c = 1.0 + aa / c;
        if c.abs() < TINY {
            c = TINY;
        }
        d = 1.0 / d;
        let del = d * c;
        h *= del;
        if (del - 1.0).abs() < EPS {
            return Some(h);
        }
    }
    None
}

/// Regularized incomplete beta function `I_x(a, b)`. `None` if it fails to converge.
///
/// Accuracy: ~1e-13 relative or better while `a` and `b` are below ~1e6. The continued
/// fraction accumulates rounding error for larger parameters (~1e-12–1e-10 around 1e8–1e12,
/// ~1e-8 at 1e15). A port of TOMS 708 (DiDonato & Morris, as used by R's `pbeta`) would fix
/// this regime; it was judged not worth the risk for parameters this extreme.
#[must_use]
pub fn beta_inc(a: f64, b: f64, x: f64) -> Option<f64> {
    beta_inc_xy(a, b, x, 1.0 - x)
}

/// `I_x(a, b)` given both `x` and `y = 1 - x`. Supplying an exactly computed `y` (e.g.
/// `t²/(ν + t²)` for Student's t, or `p` for the binomial) avoids the rounding of `1 - x`,
/// which matters when a or b is large.
#[must_use]
pub fn beta_inc_xy(a: f64, b: f64, x: f64, y: f64) -> Option<f64> {
    if a.is_nan() || a <= 0.0 || b.is_nan() || b <= 0.0 || x.is_nan() || y.is_nan() {
        return None;
    }
    if x <= 0.0 {
        return Some(0.0);
    }
    if y <= 0.0 {
        return Some(1.0);
    }
    if x < (a + 1.0) / (a + b + 2.0) {
        let front = beta_log_prefactor(a, b, x, y).exp();
        beta_cf(a, b, x).map(|h| front * h / a)
    } else {
        let front = beta_log_prefactor(b, a, y, x).exp();
        beta_cf(b, a, y).map(|h| 1.0 - front * h / b)
    }
}

// ── Saddle-point (Loader) machinery for discrete pmfs ──

/// `ln(n!) - [(n + ½) ln n - n + ½ ln 2π]` for n > 0.
///
/// Computing this directly cancels large terms, so for small n it is obtained from the
/// asymptotic series at n + m via the exact recurrence
/// `stirlerr(j) = stirlerr(j + 1) + (j + ½)·ln(1 + 1/j) − 1`.
fn stirlerr(n: f64) -> f64 {
    let mut j = n;
    let mut acc = 0.0;
    while j < 15.0 {
        acc += (j + 0.5) * (1.0 / j).ln_1p() - 1.0;
        j += 1.0;
    }
    acc + stirling_correction(j)
}

/// Deviance term `x ln(x / np) + np - x`, computed without cancellation.
fn bd0(x: f64, np: f64) -> f64 {
    if (x - np).abs() < 0.1 * (x + np) {
        let v = (x - np) / (x + np);
        let mut s = (x - np) * v;
        let mut ej = 2.0 * x * v;
        let v2 = v * v;
        let mut j = 1.0;
        loop {
            ej *= v2;
            let s1 = s + ej / (2.0 * j + 1.0);
            if s1 == s {
                return s1;
            }
            s = s1;
            j += 1.0;
        }
    }
    x * (x / np).ln() + np - x
}

/// Binomial probability mass P(X = x) for X ~ Bin(n, p) with q = 1 - p (Loader 2000).
#[must_use]
pub fn binom_pmf_raw(x: f64, n: f64, p: f64, q: f64) -> f64 {
    if p == 0.0 {
        return if x == 0.0 { 1.0 } else { 0.0 };
    }
    if q == 0.0 {
        return if x == n { 1.0 } else { 0.0 };
    }
    if x == 0.0 {
        if n == 0.0 {
            return 1.0;
        }
        let lc = if p < 0.1 {
            -bd0(n, n * q) - n * p
        } else {
            n * q.ln()
        };
        return lc.exp();
    }
    if x == n {
        let lc = if q < 0.1 {
            -bd0(n, n * p) - n * q
        } else {
            n * p.ln()
        };
        return lc.exp();
    }
    if x < 0.0 || x > n {
        return 0.0;
    }
    let lc = stirlerr(n) - stirlerr(x) - stirlerr(n - x) - bd0(x, n * p) - bd0(n - x, n * q);
    let lf = (2.0 * PI).ln() + x.ln() + (-x / n).ln_1p();
    (lc - 0.5 * lf).exp()
}

/// Poisson probability mass P(X = x) for X ~ Pois(λ) (Loader 2000).
#[must_use]
pub fn poisson_pmf_raw(x: f64, lambda: f64) -> f64 {
    if lambda == 0.0 {
        return if x == 0.0 { 1.0 } else { 0.0 };
    }
    if x == 0.0 {
        return (-lambda).exp();
    }
    if x < 0.0 {
        return 0.0;
    }
    (-stirlerr(x) - bd0(x, lambda)).exp() / (2.0 * PI * x).sqrt()
}

/// ln C(n, k) via `lgamma`.
#[must_use]
pub fn ln_choose(n: f64, k: f64) -> f64 {
    lgamma(n + 1.0) - lgamma(k + 1.0) - lgamma(n - k + 1.0)
}

/// Binomial coefficient C(n, k) as f64; `inf` if it overflows.
#[must_use]
pub fn n_cr(n: u64, k: u64) -> f64 {
    if k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    if k == 0 {
        return 1.0;
    }
    if k <= 60 {
        // Exact-as-possible multiplicative form for small k.
        let mut c = 1.0;
        for i in 0..k {
            c = c * (n - i) as f64 / (i + 1) as f64;
        }
        return c;
    }
    let ln_c = ln_choose(n as f64, k as f64);
    if ln_c > f64::MAX.ln() {
        f64::INFINITY
    } else {
        ln_c.exp().round()
    }
}

// ── Root finding ──

/// Solves `f(x) = 0` for a function increasing in x on `(lo, hi)`, using Newton steps with
/// derivative `df` safeguarded by bisection. The bracket is widened geometrically from
/// `guess` as needed. Returns `None` if no bracket or convergence is found.
pub fn solve_increasing(
    f: impl Fn(f64) -> Option<f64>,
    df: impl Fn(f64) -> f64,
    guess: f64,
    domain_lo: f64,
    domain_hi: f64,
) -> Option<f64> {
    let fg = f(guess)?;
    if fg == 0.0 {
        return Some(guess);
    }
    // Find a bracket [lo, hi] with f(lo) < 0 < f(hi).
    let (mut lo, mut hi) = (guess, guess);
    let mut step = guess.abs().max(1.0);
    if fg < 0.0 {
        loop {
            hi = (hi + step).min(domain_hi);
            if f(hi)? >= 0.0 {
                break;
            }
            if hi >= domain_hi || !hi.is_finite() {
                return None;
            }
            lo = hi;
            step *= 2.0;
        }
    } else {
        loop {
            lo = if domain_lo.is_finite() && lo - step <= domain_lo {
                // Approach a finite lower boundary geometrically.
                domain_lo + (lo - domain_lo) / 2.0
            } else {
                lo - step
            };
            if f(lo)? <= 0.0 {
                break;
            }
            if !lo.is_finite() || (lo - domain_lo).abs() < f64::MIN_POSITIVE {
                return None;
            }
            hi = lo;
            step *= 2.0;
        }
    }

    let mut x = guess.clamp(lo, hi);
    for _ in 0..400 {
        let fx = f(x)?;
        if fx == 0.0 {
            return Some(x);
        }
        if fx < 0.0 {
            lo = x;
        } else {
            hi = x;
        }
        let d = df(x);
        let mut next = if d > 0.0 && d.is_finite() {
            x - fx / d
        } else {
            f64::NAN
        };
        if !(next > lo && next < hi) {
            next = 0.5 * (lo + hi);
        }
        if (next - x).abs() <= 4.0 * f64::EPSILON * x.abs().max(f64::MIN_POSITIVE)
            || hi - lo <= 4.0 * f64::EPSILON * x.abs().max(f64::MIN_POSITIVE)
        {
            return Some(next);
        }
        x = next;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temme_expansion_agrees_with_series_and_continued_fraction() {
        // At a = 1e8 both methods are valid; Temme's truncation error is ~1e-15 there.
        let a = 1e8;
        for &x in &[0.9995e8, 0.99999e8, 1e8, 1.00001e8, 1.0004e8] {
            let (p_t, q_t) = gamma_pq_temme(a, x);
            let (p_s, q_s) = if x < a + 1.0 {
                let p = gamma_p_series(a, x).unwrap();
                (p, 1.0 - p)
            } else {
                let q = gamma_q_cf(a, x).unwrap();
                (1.0 - q, q)
            };
            let small_t = p_t.min(q_t);
            let small_s = p_s.min(q_s);
            assert!(
                ((small_t - small_s) / small_s).abs() < 1e-11,
                "x = {x}: temme {small_t:e} vs series {small_s:e}"
            );
        }
    }

    #[test]
    fn stirlerr_matches_lgamma_definition() {
        for &n in &[0.5, 1.0, 2.0, 7.5, 14.0, 15.0, 16.0, 40.0] {
            let direct = lgamma(n + 1.0) - (n + 0.5) * f64::ln(n) + n - LN_SQRT_2PI;
            assert!((stirlerr(n) - direct).abs() < 1e-13, "n = {n}");
        }
    }
}
