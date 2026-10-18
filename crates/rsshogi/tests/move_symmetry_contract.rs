//! `Move` / `Move32` の反転が盤面の先後反転と一致することを検査する。

use rsshogi::board::{Move32List, Position, generate_legal_all_move32, hirate_position};

const POSITIONS: &[&str] = &[
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K7/L3r+s2L w BS2N5Pb 1",
    "6n1l/2+S1k4/2lp4p/1np1B2b1/3PP4/1N1S3rP/1P2+pPP+p1/1p1G5/3KG2r1 b GSN2L4Pgs2p 1",
    "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1",
    "R8/2K1S1SSk/4B4/9/9/9/9/9/1L1L1L3 b RBGSNLP3g3n17p 1",
    "1n2g1k1p/2r2p2P/l+B1pp1p1n/p6P1/LN3g2L/3PG1Pp1/S3GS+s2/1P5R1/+b1KS3N1 b L3P3p 155",
];

fn usi_set(list: &Move32List) -> Vec<String> {
    let mut out: Vec<String> = list.iter().map(|mv| mv.to_usi()).collect();
    out.sort_unstable();
    out
}

#[test]
fn flipping_every_legal_move_matches_the_flipped_position() {
    for sfen in POSITIONS {
        let pos = Position::from_sfen(sfen).expect("valid fixture");
        let flipped =
            Position::from_sfen(&pos.to_sfen_flipped(None)).expect("valid flipped fixture");

        let mut original = Move32List::new();
        generate_legal_all_move32(&pos, &mut original);
        let mut expected = Move32List::new();
        generate_legal_all_move32(&flipped, &mut expected);

        let mut mapped = Move32List::new();
        for &mv in original.iter() {
            mapped.push(mv.flip());
        }

        assert_eq!(usi_set(&mapped), usi_set(&expected), "flip mismatch for {sfen}");

        for &mv in original.iter() {
            let back = mv.flip().flip();
            assert_eq!(back, mv, "flip is not an involution for {}", mv.to_usi());
            assert_eq!(
                mv.flip().piece_after_move().color(),
                mv.piece_after_move().color().flip(),
                "flip must invert the piece color of {}",
                mv.to_usi()
            );
        }
    }
}

#[test]
fn mirroring_a_file_symmetric_position_permutes_its_legal_moves() {
    // 平手は飛角があるため左右非対称。鏡像の対応を見るには対称な局面を使う。
    let pos = Position::from_sfen("lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/9/LNSGKGSNL b - 1")
        .expect("valid symmetric fixture");
    let mut legal = Move32List::new();
    generate_legal_all_move32(&pos, &mut legal);

    let mut mirrored = Move32List::new();
    for &mv in legal.iter() {
        let m = mv.mirror_file();
        assert_eq!(m.mirror_file(), mv, "mirror_file is not an involution");
        assert_eq!(m.piece_after_move(), mv.piece_after_move());
        mirrored.push(m);
    }

    assert_eq!(usi_set(&mirrored), usi_set(&legal));
}

#[test]
fn mirroring_is_an_involution_on_every_legal_move() {
    let pos = hirate_position();
    let mut legal = Move32List::new();
    generate_legal_all_move32(&pos, &mut legal);
    for &mv in legal.iter() {
        assert_eq!(mv.mirror_file().mirror_file(), mv, "{}", mv.to_usi());
        assert_eq!(mv.mirror_file().piece_after_move(), mv.piece_after_move());
    }
}
