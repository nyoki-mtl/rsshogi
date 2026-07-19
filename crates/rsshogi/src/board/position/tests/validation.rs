use super::*;
use crate::types::{Color, File, HandPiece, PieceType};

#[test]
// 初期局面がis_validで正常判定されることを確認
fn test_valid_initial_position() {
    let pos = crate::board::hirate_position();

    match pos.validate() {
        Ok(()) => {}
        Err(e) => panic!("Initial position should be valid, but got error: {e:?}"),
    }
}

#[test]
// 玉が二枚ある局面を弾くことを確認（先手側）
fn test_two_kings_is_invalid() {
    let sfen = "k8/9/9/9/9/9/9/9/K7K b - 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            assert_eq!(e, ValidationError::TwoKings(Color::BLACK));
        }
    }
}

#[test]
// 玉が欠けた局面を許容することを検証
fn test_no_king_is_allowed() {
    let sfen = "lnsg1gsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSG1GSNL b - 1";

    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    assert!(pos.is_valid());
}

#[test]
// 玉が1枚だけの局面は許容されることを確認
fn test_single_king_position_is_allowed() {
    let sfen = "K8/9/9/9/9/9/9/9/9 b - 1";

    let pos = crate::board::position_from_sfen(sfen).expect("valid sfen");
    assert!(pos.is_valid());
}

#[test]
// 同一筋の二歩を検出できることを確認
fn test_double_pawn_is_invalid() {
    let sfen = "lnsgkgsnP/1r5b1/ppppppppp/9/9/9/PPPPPPPP1/1B5RP/LNSGKGSNL b - 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
        if let Err(e) = result {
            match e {
                ValidationError::DoublePawn(file, color) => {
                    assert_eq!(file, File::FILE_1);
                    assert_eq!(color, Color::BLACK);
                }
                _ => panic!("Expected DoublePawn error"),
            }
        }
    }
}

#[test]
// 持ち駒の枚数超過を検出することを確認
fn test_invalid_hand_count_is_rejected() {
    let sfen = "lnsgkgsnl/1r5b1/9/9/9/9/9/1B5R1/LNSGKGSNL b 19P 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
    }
}

#[test]
// 最奥段の歩と香を不正として扱うことを確認
fn test_pawn_and_lance_on_last_rank_are_invalid() {
    let sfen = "P8/9/9/9/9/9/9/9/K8 b - 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
    }
}

#[test]
// 桂馬が最奥2段以内にある場合を不正と判定できるか検証
fn test_knight_on_last_two_ranks_is_invalid() {
    let sfen = "k8/N8/9/9/9/9/9/9/K8 b - 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
    }
}

#[test]
// 駒総数超過の異常局面を検出することを確認
fn test_excessive_total_piece_count_is_invalid() {
    let sfen = "ppppppppp/ppppppppp/9/9/9/9/9/9/K8 b 2P 1";

    if let Ok(pos) = crate::board::position_from_sfen(sfen) {
        let result = pos.validate();
        assert!(result.is_err());
    }
}

// --- validate_all() tests ---

#[test]
fn test_validate_all_valid_position() {
    let pos = crate::board::hirate_position();
    let report = pos.validate_all();
    assert!(report.is_valid());
    assert!(report.issues().is_empty());
}

#[test]
fn test_validate_all_reports_no_king() {
    let sfen = "lnsg1gsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSG1GSNL b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let report = pos.validate_all();
    // validate() は NoKing を許容するが、validate_all() は報告する
    assert_eq!(
        report.issues().iter().filter(|i| matches!(i, ValidationIssue::NoKing(_))).count(),
        2
    ); // 先手・後手両方
}

#[test]
fn test_validate_all_collects_multiple_issues() {
    // 先手玉が2枚 + 1段目に先手歩（行き場なし）
    let sfen = "K6PK/9/9/9/9/9/9/9/k8 b - 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let report = pos.validate_all();
    assert!(!report.is_valid());
    // TwoKings(BLACK) + InvalidPlacement (歩が1段目)
    assert!(report.issues().len() >= 2);

    let has_two_kings =
        report.issues().iter().any(|i| matches!(i, ValidationIssue::TwoKings(Color::BLACK)));
    assert!(has_two_kings);

    let has_invalid_placement = report
        .issues()
        .iter()
        .any(|i| matches!(i, ValidationIssue::InvalidPlacement(_, PieceType::PAWN)));
    assert!(has_invalid_placement);
}

#[test]
fn test_validate_all_hand_count_overflow() {
    let sfen = "lnsgkgsnl/1r5b1/9/9/9/9/9/1B5R1/LNSGKGSNL b 19P 1";
    let pos = crate::board::position_from_sfen(sfen).unwrap();
    let report = pos.validate_all();
    assert!(!report.is_valid());

    assert!(
        report
            .issues()
            .iter()
            .any(|i| matches!(i, ValidationIssue::InvalidHandCount { piece: HandPiece::PAWN, .. }))
    );
}

#[test]
fn test_validate_all_into_issues() {
    let pos = crate::board::hirate_position();
    let report = pos.validate_all();
    let issues = report.into_issues();
    assert!(issues.is_empty());
}

#[test]
fn test_validation_issue_display() {
    let issue = ValidationIssue::NoKing(Color::BLACK);
    let s = format!("{issue}");
    assert!(s.contains("king"));
}
