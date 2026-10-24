#![cfg(feature = "records")]

use rsshogi::board::STARTPOS_SFEN;
use rsshogi::records::formats::{csa, jkf, kif};
use rsshogi::records::{Record, RecordAnnotation, SpecialMove, SpecialMoveEntry};
use rsshogi::types::{Color, GameResult};

#[test]
fn illegal_results_survive_csa_and_jkf_through_kif() {
    for prefix in ["V2.2\nPI\n+\n", "V2.2\nPI\n+\n+7776FU\n"] {
        for marker in ["%ILLEGAL_MOVE", "%+ILLEGAL_ACTION", "%-ILLEGAL_ACTION"] {
            let source = csa::parse_csa_str(&format!("{prefix}{marker}\n")).unwrap();
            let via_jkf = jkf::parse_jkf_str(&jkf::export_jkf(&source).unwrap()).unwrap();
            for record in [&source, &via_jkf] {
                let text = kif::export_kif(record).unwrap();
                assert_eq!(kif::parse_kif_str(&text).unwrap().result(), source.result(), "{text}");
            }
        }
    }
}

#[test]
fn corrected_illegal_and_forfeit_results_survive_kif_with_elapsed_time() {
    for (moves, count) in [("▲２六歩", 1), ("▲２六歩 △３四歩", 2)] {
        for reason in ["反則勝ち", "反則負け", "不戦勝", "不戦敗"] {
            for winner in ["先手", "後手"] {
                let mut record = kif::parse_ki2_str(&format!(
                    "{moves} △{reason}\nまで{count}手で{winner}の勝ち"
                ))
                .unwrap();
                let terminal = record.main_terminal().unwrap().clone();
                let mut annotation = RecordAnnotation::new();
                annotation.set_elapsed_ms(Some(3000));
                record.set_main_terminal_with_annotation(terminal, annotation).unwrap();
                let text = kif::export_kif(&record).unwrap();
                let restored = kif::parse_kif_str(&text).unwrap();
                assert_eq!(restored.result(), record.result(), "{text}");
                assert_eq!(
                    restored.node(restored.main_terminal_node().unwrap()).time_ms(),
                    Some(3000)
                );
            }
        }
    }
}

#[test]
fn kif_rejects_winners_that_its_terminal_move_cannot_express() {
    for (kind, result) in [
        (SpecialMove::Resign, GameResult::BlackWin),
        (SpecialMove::Mate, GameResult::BlackWin),
        (SpecialMove::Timeout, GameResult::BlackWinByTimeout),
        (SpecialMove::WinByDeclaration, GameResult::WhiteWinByDeclaration),
    ] {
        let mut record = Record::new(STARTPOS_SFEN).unwrap();
        record.set_main_terminal(SpecialMoveEntry::new(kind, result)).unwrap();
        assert!(matches!(kif::export_kif(&record), Err(kif::KifError::InvalidLine(_))));
    }
}

#[test]
fn corrected_timeout_can_stay_in_ki2_but_cannot_silently_change_in_kif() {
    for (moves, count, winner) in [("▲２六歩", 1, "後手"), ("▲２六歩 △３四歩", 2, "先手")]
    {
        let source =
            kif::parse_ki2_str(&format!("{moves} △切れ負け\nまで{count}手で{winner}の勝ち"))
                .unwrap();
        assert!(kif::export_kif(&source).is_err());
        let text = kif::export_ki2(&source).unwrap();
        assert_eq!(kif::parse_ki2_str(&text).unwrap().result(), source.result());
    }
}

#[test]
fn mate_tokens_and_summaries_roundtrip_as_mate() {
    for summary in ["まで0手で詰み", "まで0手で詰"] {
        let record = kif::parse_ki2_str(summary).unwrap();
        assert_eq!(record.result(), GameResult::WhiteWin);
        assert_eq!(record.main_terminal().unwrap().kind(), &SpecialMove::Mate);
        for text in [kif::export_kif(&record).unwrap(), kif::export_ki2(&record).unwrap()] {
            let restored = if text.contains("手数----") {
                kif::parse_kif_str(&text).unwrap()
            } else {
                kif::parse_ki2_str(&text).unwrap()
            };
            assert_eq!(restored.main_terminal().unwrap().kind(), &SpecialMove::Mate);
            assert_eq!(restored.result(), record.result());
        }
    }
}

#[test]
fn ki2_rejects_inconsistent_mate_winners_from_records_and_inline_summaries() {
    for (moves, ply, side_to_move, winner) in
        [("▲２六歩", 1, Color::WHITE, "後手"), ("▲２六歩 △３四歩", 2, Color::BLACK, "先手")]
    {
        let parsed =
            kif::parse_ki2_str(&format!("{moves} △詰み\nまで{ply}手で{winner}の勝ち")).unwrap();
        assert_eq!(parsed.result().winner_color(), Some(side_to_move));
        assert_eq!(parsed.main_terminal().unwrap().kind(), &SpecialMove::Mate);
        assert!(matches!(kif::export_ki2(&parsed), Err(kif::Ki2Error::InvalidLine(_))));

        let mut record = parsed.clone();
        record
            .set_main_terminal(SpecialMoveEntry::new(
                SpecialMove::Mate,
                GameResult::win_from_color(side_to_move.flip()),
            ))
            .unwrap();
        let output = kif::export_ki2(&record).unwrap();
        let restored = kif::parse_ki2_str(&output).unwrap();
        assert_eq!(restored.result(), record.result());
        assert_eq!(restored.main_terminal().unwrap().kind(), &SpecialMove::Mate);

        record
            .set_main_terminal(SpecialMoveEntry::new(
                SpecialMove::Mate,
                GameResult::win_from_color(side_to_move),
            ))
            .unwrap();
        assert!(kif::export_ki2(&record).is_err());
    }
}

#[test]
fn exporters_reject_try_results_their_parsers_cannot_restore() {
    for (sfen, result) in [
        (STARTPOS_SFEN.to_string(), GameResult::BlackWinByTryRule),
        (STARTPOS_SFEN.replace(" b ", " w "), GameResult::WhiteWinByTryRule),
    ] {
        let mut record = Record::new(sfen).unwrap();
        record.set_main_terminal(SpecialMoveEntry::new(SpecialMove::Try, result)).unwrap();
        assert!(kif::export_ki2(&record).is_err());
        assert!(kif::export_kif(&record).is_err());

        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Try, GameResult::Paused))
            .unwrap();
        assert!(kif::export_ki2(&record).is_err());
        let text = kif::export_kif(&record).unwrap();
        let restored = kif::parse_kif_str(&text).unwrap();
        assert_eq!(restored.result(), GameResult::Paused);
        assert_eq!(restored.main_terminal().unwrap().kind(), &SpecialMove::Try);
    }
}

#[test]
fn csa_and_jkf_preserve_representable_winners_and_reject_other_winners() {
    for turn in [Color::BLACK, Color::WHITE] {
        let sfen = STARTPOS_SFEN.replace(" b ", if turn == Color::BLACK { " b " } else { " w " });
        for winner in [Color::BLACK, Color::WHITE] {
            for (kind, result, expected_winner) in [
                (SpecialMove::Resign, GameResult::win_from_color(winner), turn.flip()),
                (SpecialMove::Mate, GameResult::win_from_color(winner), turn.flip()),
                (SpecialMove::Timeout, GameResult::win_by_timeout_from_color(winner), turn.flip()),
                (
                    SpecialMove::WinByDeclaration,
                    GameResult::win_by_declaration_from_color(winner),
                    turn,
                ),
            ] {
                let mut record = Record::new(sfen.as_str()).unwrap();
                record.set_main_terminal(SpecialMoveEntry::new(kind.clone(), result)).unwrap();
                let csa = csa::export_csa(&record);
                let jkf = jkf::export_jkf(&record);
                if winner == expected_winner {
                    let csa = csa::parse_csa_str(&csa.unwrap()).unwrap();
                    let jkf = jkf::parse_jkf_str(&jkf.unwrap()).unwrap();
                    for restored in [csa, jkf] {
                        assert_eq!(restored.result(), result);
                        assert_eq!(restored.main_terminal().unwrap().kind(), &kind);
                    }
                } else {
                    assert!(csa.is_err(), "{turn:?} {kind:?} {result:?}");
                    assert!(jkf.is_err(), "{turn:?} {kind:?} {result:?}");
                }
            }
        }
    }
}

#[test]
fn csa_and_jkf_accept_generic_winners_but_reject_conflicting_reasons() {
    for turn in [Color::BLACK, Color::WHITE] {
        let sfen = STARTPOS_SFEN.replace(" b ", if turn == Color::BLACK { " b " } else { " w " });
        for (kind, expected) in [
            (SpecialMove::Timeout, GameResult::win_by_timeout_from_color(turn.flip())),
            (
                SpecialMove::WinByIllegalMove,
                GameResult::win_by_illegal_move_from_color(turn.flip()),
            ),
            (
                SpecialMove::LoseByIllegalMove,
                GameResult::win_by_illegal_move_from_color(turn.flip()),
            ),
            (SpecialMove::WinByDeclaration, GameResult::win_by_declaration_from_color(turn)),
        ] {
            let mut record = Record::new(sfen.as_str()).unwrap();
            record
                .set_main_terminal(SpecialMoveEntry::new(
                    kind.clone(),
                    GameResult::win_from_color(expected.winner_color().unwrap()),
                ))
                .unwrap();
            for restored in [
                csa::parse_csa_str(&csa::export_csa(&record).unwrap()).unwrap(),
                jkf::parse_jkf_str(&jkf::export_jkf(&record).unwrap()).unwrap(),
            ] {
                assert_eq!(restored.result(), expected, "{turn:?} {kind:?}");
            }
            record
                .set_main_terminal(SpecialMoveEntry::new(
                    kind.clone(),
                    GameResult::win_from_color(expected.winner_color().unwrap().flip()),
                ))
                .unwrap();
            if matches!(kind, SpecialMove::WinByIllegalMove | SpecialMove::LoseByIllegalMove) {
                for restored in [
                    csa::parse_csa_str(&csa::export_csa(&record).unwrap()).unwrap(),
                    jkf::parse_jkf_str(&jkf::export_jkf(&record).unwrap()).unwrap(),
                ] {
                    assert_eq!(restored.result(), GameResult::win_by_illegal_move_from_color(turn));
                }
            } else {
                assert!(csa::export_csa(&record).is_err());
                assert!(jkf::export_jkf(&record).is_err());
            }
        }
        let mut record = Record::new(sfen.as_str()).unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(
                SpecialMove::Resign,
                GameResult::win_by_timeout_from_color(turn.flip()),
            ))
            .unwrap();
        assert!(csa::export_csa(&record).is_err());
        assert!(jkf::export_jkf(&record).is_err());
    }
}

#[test]
fn csa_and_jkf_reject_forfeit_and_try_instead_of_changing_reason() {
    for text in [
        "▲２六歩 △不戦勝\nまで1手で先手の勝ち",
        "▲２六歩 △不戦敗\nまで1手で先手の勝ち",
        "手合割：香落ち\n△投了\nまで0手で上手の勝ち",
        "▲２六歩 △切れ負け\nまで1手で後手の勝ち",
    ] {
        let record = kif::parse_ki2_str(text).unwrap();
        assert!(csa::export_csa(&record).is_err(), "{text}");
        assert!(jkf::export_jkf(&record).is_err(), "{text}");
    }
    for result in [GameResult::BlackWinByTryRule, GameResult::WhiteWinByTryRule, GameResult::Paused]
    {
        let mut record = Record::new(STARTPOS_SFEN).unwrap();
        record.set_main_terminal(SpecialMoveEntry::new(SpecialMove::Try, result)).unwrap();
        assert!(csa::export_csa(&record).is_err());
        assert!(jkf::export_jkf(&record).is_err());
    }
}

#[test]
fn raw_terminal_markers_cannot_override_recorded_winners() {
    let mut record = csa::parse_csa_str("V2.2\nPI\n+\n%+ILLEGAL_ACTION\n").unwrap();
    record
        .set_main_terminal(
            SpecialMoveEntry::new(SpecialMove::WinByIllegalMove, GameResult::BlackWinByIllegalMove)
                .with_raw(Some("%+ILLEGAL_ACTION".into())),
        )
        .unwrap();
    assert!(csa::export_csa(&record).is_err());
    assert!(jkf::export_jkf(&record).is_err());
}

#[test]
fn jkf_checks_terminal_results_in_variations() {
    use rsshogi::records::MoveEntry;
    use rsshogi::types::Move;
    let mut record = Record::new(STARTPOS_SFEN).unwrap();
    record.append_move(record.root_id(), MoveEntry::new(Move::from_usi("7g7f").unwrap())).unwrap();
    record
        .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
        .unwrap();
    let variation = record
        .append_move(record.root_id(), MoveEntry::new(Move::from_usi("2g2f").unwrap()))
        .unwrap();
    for result in [GameResult::BlackWin, GameResult::WhiteWinByTimeout] {
        let mut record = record.clone();
        record
            .append_special(variation, SpecialMoveEntry::new(SpecialMove::Timeout, result))
            .unwrap();
        assert_eq!(jkf::export_jkf(&record).is_ok(), result == GameResult::BlackWin);
    }
}
