use abacus::{Abacus, Date, eval};
use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};

fn bench_tokenizer(c: &mut Criterion) {
    let abacus = Abacus::standard();
    let expr = "100 km/h + 50 m/s - sqrt(14 N * 5 m) / 2.5 s";
    let token_count = abacus.tokenize(expr).expect("valid expression").len() as u64;

    let mut group = c.benchmark_group("tokenizer");
    group.throughput(Throughput::Elements(token_count));
    group.bench_function("tokenize_expression", |b| {
        b.iter(|| {
            let tokens = abacus
                .tokenize(black_box(expr))
                .expect("tokenization succeeds");
            black_box(tokens);
        })
    });
    group.finish();
}

fn bench_basic_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_arithmetic");
    group.bench_function("add_meters", |b| {
        b.iter(|| {
            let res = eval(black_box("10 m + 5 m")).expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.bench_function("nested_parens", |b| {
        b.iter(|| {
            let res = eval(black_box("((10 + 5) * 2) / (3 - 1)")).expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.finish();
}

fn bench_complex_expressions(c: &mut Criterion) {
    let mut group = c.benchmark_group("complex_expressions");
    group.bench_function("z_test", |b| {
        b.iter(|| {
            let res = eval(black_box("ZTestData(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)"))
                .expect("evaluation succeeds");
            black_box(res);
        })
    });
    // Anchored so the measured work doesn't depend on the wall clock.
    let anchored = Abacus::standard().with_now(
        "2026-10-21 09:30:00"
            .parse::<Date>()
            .expect("valid anchor date"),
    );
    group.bench_function("relative_date", |b| {
        b.iter(|| {
            let res = anchored
                .eval(black_box("last thursday at 3pm + 2 weeks"))
                .expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.finish();
}

fn bench_derived_units(c: &mut Criterion) {
    let mut group = c.benchmark_group("derived_units");
    group.bench_function("joules_reduction", |b| {
        b.iter(|| {
            let res = eval(black_box("10 N * 5 m")).expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.bench_function("conversion_overhead", |b| {
        b.iter(|| {
            let res = eval(black_box("100 km/h in m/s")).expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.finish();
}

/// A representative mix of expressions, each evaluated and rendered (`eval` + `to_display`),
/// covering units, conversions, intervals, functions, dates, currencies and compound units.
const MIXED_EXPRESSIONS: &[(&str, &str)] = &[
    ("plain", "1+1"),
    ("derived_unit", "10 N * 5 m"),
    ("conversion", "100 km/h in m/s"),
    ("interval_division", "[9.8 m, 10.2 m] / [1.9 s, 2.1 s]"),
    ("trig_degrees", "sqrt(2)*sin(30 deg)"),
    ("stats_mean", "mean(1,2,3,4,5)"),
    ("business_days", "2026-08-07 + 5 business days"),
    ("currency", "$50 + €20 in USD"),
    ("compound_measure", "5 ft 3 in to cm"),
    ("regression", "linreg(1,2,3,4,10,20,30,40).slope"),
];

fn bench_mixed_expressions(c: &mut Criterion) {
    let abacus = Abacus::standard();
    let mut group = c.benchmark_group("eval_and_display");
    for &(name, expr) in MIXED_EXPRESSIONS {
        group.bench_function(name, |b| {
            b.iter(|| {
                let res = abacus.eval(black_box(expr)).expect("evaluation succeeds");
                black_box(res.to_display());
            })
        });
    }
    group.finish();
}

fn bench_construction(c: &mut Criterion) {
    // Warm the shared standard tables so this measures per-instance construction.
    black_box(Abacus::standard());
    let mut group = c.benchmark_group("construction");
    group.bench_function("abacus_standard", |b| {
        b.iter(|| black_box(Abacus::standard()));
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_tokenizer,
    bench_basic_arithmetic,
    bench_complex_expressions,
    bench_derived_units,
    bench_mixed_expressions,
    bench_construction
);
criterion_main!(benches);
