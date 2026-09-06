use abacus::{Abacus, eval};
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
            let res = eval(black_box("ZTest(10 m, 2 m, 12 m, 14 m, 10 m, 16 m)"))
                .expect("evaluation succeeds");
            black_box(res);
        })
    });
    group.bench_function("relative_date", |b| {
        b.iter(|| {
            let res =
                eval(black_box("last thursday at 3pm + 2 weeks")).expect("evaluation succeeds");
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

criterion_group!(
    benches,
    bench_tokenizer,
    bench_basic_arithmetic,
    bench_complex_expressions,
    bench_derived_units
);
criterion_main!(benches);
