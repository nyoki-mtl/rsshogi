use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use rsshogi::board::{
    self, CapturePlusPro, Captures, Evasions, MoveList, NonEvasionsAll, generate_moves,
};

/// 参照実装のベンチマーク局面。
const BENCHMARK_POSITIONS: &[(&str, &str)] = &[
    ("startpos", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
    ("near_startpos", "lnsgkgsnl/1r7/p1ppp1bpp/1p3pp2/7P1/2P6/PP1PPPP1P/1B3S1R1/LNSGKG1NL b - 9"),
    ("complex_1", "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1"),
    (
        "complex_2",
        "6n1l/2+S1k4/2lp4p/1np1B2b1/3PP4/1N1S3rP/1P2+pPP+p1/1p1G5/3KG2r1 b GSN2L4Pgs2p 1",
    ),
    ("movegen_fest", "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1"),
    // yasai ベンチマーク由来の合法手数最大局面
    ("max_moves", "R8/2K1S1SSk/4B4/9/9/9/9/9/1L1L1L3 b RBGSNLP3g3n17p 1"),
];

fn bench_all_moves(c: &mut Criterion) {
    let mut group = c.benchmark_group("all_moves");

    for &(name, sfen) in BENCHMARK_POSITIONS {
        group.bench_with_input(BenchmarkId::from_parameter(name), &sfen, |b, &sfen| {
            let pos = board::position_from_sfen(sfen).unwrap();

            b.iter(|| {
                let mut moves = MoveList::new();
                generate_moves::<NonEvasionsAll>(&pos, &mut moves);
                black_box(moves.len())
            });
        });
    }

    group.finish();
}

fn bench_captures(c: &mut Criterion) {
    let mut group = c.benchmark_group("captures");

    for &(name, sfen) in BENCHMARK_POSITIONS {
        group.bench_with_input(BenchmarkId::from_parameter(name), &sfen, |b, &sfen| {
            let pos = board::position_from_sfen(sfen).unwrap();

            b.iter(|| {
                let mut moves = MoveList::new();
                generate_moves::<Captures>(&pos, &mut moves);
                black_box(moves.len())
            });
        });
    }

    group.finish();
}

fn bench_capture_plus_pro(c: &mut Criterion) {
    let mut group = c.benchmark_group("capture_plus_pro");

    for &(name, sfen) in BENCHMARK_POSITIONS {
        group.bench_with_input(BenchmarkId::from_parameter(name), &sfen, |b, &sfen| {
            let pos = board::position_from_sfen(sfen).unwrap();

            b.iter(|| {
                let mut moves = MoveList::new();
                generate_moves::<CapturePlusPro>(&pos, &mut moves);
                black_box(moves.len())
            });
        });
    }

    group.finish();
}

fn bench_evasions(c: &mut Criterion) {
    let mut group = c.benchmark_group("evasions");

    // 手番側が王手されている局面のみ使用する
    let evasion_positions =
        &[("check_pos_1", "lnsgkg1nl/1r5s1/pppppp1pp/6p2/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w Bb 1")];

    for &(name, sfen) in evasion_positions {
        let pos = board::position_from_sfen(sfen).unwrap();

        // 実際に王手されている場合のみベンチマークする
        if !pos.checkers().is_empty() {
            group.bench_with_input(BenchmarkId::from_parameter(name), &sfen, |b, &sfen| {
                let pos = board::position_from_sfen(sfen).unwrap();

                b.iter(|| {
                    let mut moves = MoveList::new();
                    generate_moves::<Evasions>(&pos, &mut moves);
                    black_box(moves.len())
                });
            });
        }
    }

    group.finish();
}

fn bench_pinned_throughput(c: &mut Criterion) {
    let pinned_positions = [
        (
            "rook_line_pressure",
            "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
        ),
        (
            "pin_checker_test",
            "ln3gsn1/7kl/3+B1p1p1/p4s2p/2P6/P2B3PP/1PNP+rPP2/2G3SK1/L4G1NL b G3Prs3p 65",
        ),
    ];

    let mut group = c.benchmark_group("pinned_throughput");

    for &(name, sfen) in &pinned_positions {
        group.bench_with_input(BenchmarkId::from_parameter(name), &sfen, |b, &sfen| {
            let pos = board::position_from_sfen(sfen).unwrap();

            b.iter(|| {
                let mut moves = MoveList::new();
                generate_moves::<NonEvasionsAll>(&pos, &mut moves);
                black_box(moves.len())
            });
        });
    }

    group.finish();
}

criterion_group!(
    movegen_benches,
    bench_all_moves,
    bench_captures,
    bench_capture_plus_pro,
    bench_evasions,
    bench_pinned_throughput
);
criterion_main!(movegen_benches);
