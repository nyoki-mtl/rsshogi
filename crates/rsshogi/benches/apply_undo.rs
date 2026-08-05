use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use rsshogi::board::{self, LegalAll, MoveList, Position, generate_moves};
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

/// 持ち駒キーの差分更新は捕獲と駒打ちにしか現れないため、1 手だけの計測では感度が出ない。
/// 捕獲と駒打ちを優先して選んだ系列を往復させ、差分更新のコストを増幅して測る。
///
/// 手を USI で固定せず局面から構成するのは、指し手生成や合法性判定の変更で
/// bench が壊れないようにするため。
fn sequence_moves(pos: &Position, len: usize) -> Vec<Move32> {
    let mut probe = pos.clone();
    let mut moves = Vec::with_capacity(len);

    for _ in 0..len {
        let mut list = MoveList::new();
        generate_moves::<LegalAll>(&probe, &mut list);

        let legal: Vec<Move32> = list
            .iter()
            .map(|&mv| probe.move32_from_move(mv))
            .filter(|&mv| probe.is_legal_move32(mv))
            .collect();
        assert!(!legal.is_empty(), "bench position must have legal moves");

        // 捕獲 > 駒打ち > その他 の順に選び、持ち駒キーの更新経路を必ず踏む。
        let chosen = legal
            .iter()
            .find(|&&mv| !probe.piece_on(mv.to_sq()).is_empty())
            .or_else(|| legal.iter().find(|&&mv| mv.is_drop()))
            .copied()
            .unwrap_or(legal[0]);

        probe.apply_move32(chosen);
        moves.push(chosen);
    }

    moves
}

fn bench_capture_drop_sequence(c: &mut Criterion) {
    let sfen = "lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11";
    let base = board::position_from_sfen(sfen).unwrap();
    let moves = sequence_moves(&base, 6);

    c.bench_function("do_undo_capture_drop_sequence", |b| {
        b.iter_batched(
            || base.clone(),
            |mut pos| {
                for &mv in &moves {
                    pos.apply_move32(mv);
                }
                for &mv in moves.iter().rev() {
                    pos.undo_move32(mv).unwrap();
                }
            },
            BatchSize::SmallInput,
        );
    });
}

/// `apply_move32` を通さないキーの読み出し経路を測る。
///
/// キーの真実点が `StateHot` に移り、`Position::key()` が state stack 経由の
/// 依存ロード 2 回になったため、読み出し側の退行を単独で検出できるようにする。
fn bench_key_probe(c: &mut Criterion) {
    let sfen = "lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11";
    let base = board::position_from_sfen(sfen).unwrap();
    let moves = sequence_moves(&base, 6);

    let mut pos = base.clone();
    for &mv in &moves {
        pos.apply_move32(mv);
    }

    c.bench_function("key_probe", |b| {
        b.iter(|| std::hint::black_box(std::hint::black_box(&pos).key()));
    });

    c.bench_function("key_after_probe", |b| {
        let mv = moves[0];
        let probe = base.clone();
        b.iter(|| std::hint::black_box(std::hint::black_box(&probe).key_after(mv)));
    });
}

/// perft を apply / undo の総合指標として測る。
///
/// 単発の apply / undo は数百 ns しかなく、マシンのノイズに埋もれやすい。
/// perft は同じ経路を数十万回踏むため、差分更新の退行を安定して検出できる。
fn bench_perft(c: &mut Criterion) {
    let mut group = c.benchmark_group("perft");
    group.sample_size(20);

    let hirate = board::hirate_position();
    group.bench_function("startpos_depth4", |b| {
        b.iter(|| {
            let result = board::perft::perft(std::hint::black_box(&hirate), 4).unwrap();
            std::hint::black_box(result.nodes)
        });
    });

    // 持ち駒が多く、駒打ちが手生成の大半を占める局面。
    let sfen = "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2S2/1PPb2P1P/P5GS1/R8/LN4bKL w RGSNLPbsnl3p 1";
    let with_hands = board::position_from_sfen(sfen).unwrap();
    group.bench_function("many_drops_depth3", |b| {
        b.iter(|| {
            let result = board::perft::perft(std::hint::black_box(&with_hands), 3).unwrap();
            std::hint::black_box(result.nodes)
        });
    });

    group.finish();
}

criterion_group!(
    core_benches,
    bench_do_move,
    bench_do_and_undo,
    bench_capture_drop_sequence,
    bench_key_probe,
    bench_perft
);
criterion_main!(core_benches);
