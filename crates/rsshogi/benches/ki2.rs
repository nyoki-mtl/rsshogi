use criterion::{Criterion, black_box, criterion_group, criterion_main};
use rsshogi::board::{self, position_from_sfen};
use rsshogi::types::Move;

fn bench_ki2(c: &mut Criterion) {
    board::init();

    let ambiguous =
        position_from_sfen("lnsg1gsnl/1r3k1b1/ppppppppp/9/8P/9/PPPPPPPP1/1B5R1/LNSGKGSNL w - 4")
            .expect("valid KI2 ambiguity benchmark position");
    let ambiguous_move =
        ambiguous.move32_from_move(Move::from_usi("4a5a").expect("valid benchmark move"));
    c.bench_function("ki2/ambiguous_sideways", |b| {
        b.iter(|| black_box(ambiguous_move).to_ki2(black_box(&ambiguous)))
    });
    c.bench_function("ki2_notation/ambiguous_sideways", |b| {
        b.iter(|| black_box(ambiguous_move).to_ki2_notation(black_box(&ambiguous)))
    });

    let unique = board::hirate_position();
    let unique_move =
        unique.move32_from_move(Move::from_usi("7g7f").expect("valid benchmark move"));
    c.bench_function("ki2/unique", |b| {
        b.iter(|| black_box(unique_move).to_ki2(black_box(&unique)))
    });
    c.bench_function("ki2_notation/unique", |b| {
        b.iter(|| black_box(unique_move).to_ki2_notation(black_box(&unique)))
    });
}

criterion_group!(benches, bench_ki2);
criterion_main!(benches);
