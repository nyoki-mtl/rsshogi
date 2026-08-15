use criterion::{Criterion, criterion_group, criterion_main};
use rsshogi::board::{self, position_from_sfen};
use rsshogi::mate::solve_mate_in_one;

fn bench_mate_in_one(c: &mut Criterion) {
    board::init();

    let mate = position_from_sfen(
        "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
    )
    .expect("valid mate benchmark position");
    c.bench_function("mate_in_one/found", |b| {
        b.iter(|| {
            let result = solve_mate_in_one(std::hint::black_box(&mate));
            assert!(result.is_some());
            std::hint::black_box(result)
        });
    });

    let no_mate = board::hirate_position();
    c.bench_function("mate_in_one/none_startpos", |b| {
        b.iter(|| {
            let result = solve_mate_in_one(std::hint::black_box(&no_mate));
            assert!(result.is_none());
            std::hint::black_box(result)
        });
    });
}

criterion_group!(benches, bench_mate_in_one);
criterion_main!(benches);
