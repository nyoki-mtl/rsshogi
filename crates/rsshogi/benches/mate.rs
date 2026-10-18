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

    // 王手中は board-first の候補列を通るため、非王手局面とは別経路になる。
    let in_check_mate = position_from_sfen(
        "6gnb/9/P5p1l/l1k1p2p1/1PSPPp1PP/1sKR1P3/+ppG1+nG2L/s2N3l1/B4+n2R w GSP4p 180",
    )
    .expect("valid in-check mate benchmark position");
    assert!(in_check_mate.is_in_check());
    c.bench_function("mate_in_one/in_check_found", |b| {
        b.iter(|| {
            let result = solve_mate_in_one(std::hint::black_box(&in_check_mate));
            assert!(result.is_some());
            std::hint::black_box(result)
        });
    });

    let in_check_none = position_from_sfen(
        "1n2g1k1p/2r2p2P/l+B1pp1p1n/p6P1/LN3g2L/3PG1Pp1/S3GS+s2/1P5R1/+b1KS3N1 b L3P3p 155",
    )
    .expect("valid in-check non-mate benchmark position");
    assert!(in_check_none.is_in_check());
    c.bench_function("mate_in_one/in_check_none", |b| {
        b.iter(|| {
            let result = solve_mate_in_one(std::hint::black_box(&in_check_none));
            assert!(result.is_none());
            std::hint::black_box(result)
        });
    });
}

criterion_group!(benches, bench_mate_in_one);
criterion_main!(benches);
