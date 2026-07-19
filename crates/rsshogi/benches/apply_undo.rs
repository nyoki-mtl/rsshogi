use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rsshogi::board;
use rsshogi::types::{Move32, PieceType, Square};

fn bench_do_move(c: &mut Criterion) {
    c.bench_function("do_move_startpos", |b| {
        b.iter_batched(
            board::hirate_position,
            |mut pos| {
                let from = Square::from_usi("7g").unwrap();
                let to = Square::from_usi("7f").unwrap();
                let piece = pos.piece_on(from);
                let mv = Move32::normal(from, to, piece);
                pos.apply_move32(mv);
            },
            BatchSize::SmallInput,
        );
    });
}

fn bench_do_and_undo(c: &mut Criterion) {
    c.bench_function("do_undo_drop", |b| {
        b.iter_batched(
            || {
                let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b P 1";
                board::position_from_sfen(sfen).unwrap()
            },
            |mut pos| {
                let to = Square::from_usi("5e").unwrap();
                let mv = Move32::drop(PieceType::PAWN, to, pos.turn());
                pos.apply_move32(mv);
                pos.undo_move32(mv).unwrap();
            },
            BatchSize::SmallInput,
        );
    });
}

criterion_group!(core_benches, bench_do_move, bench_do_and_undo);
criterion_main!(core_benches);
