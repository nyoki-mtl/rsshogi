//! 棋譜ツリーの局面付き走査
//!
//! [`Record`] を DFS で走査し、各ノードで指し手適用後の局面を計算する
//! ヘルパーを提供する。
//!
//! # Examples
//!
//! ```
//! use rsshogi::records::formats::{kif, traversal};
//! use rsshogi::records::record::RecordEntry;
//!
//! let kif_text = "\
//! 手合割：平手
//! 手数----指手---------消費時間--
//!    1 ７六歩(77)
//!    2 ３四歩(33)
//! まで2手で中断
//! ";
//!
//! let record = kif::parse_kif_str(kif_text).unwrap();
//! let mut plies = Vec::new();
//! traversal::traverse_with_position(&record, |node| {
//!     if let RecordEntry::Move(_) = node.entry {
//!         plies.push(node.ply);
//!     }
//!     true
//! }).unwrap();
//! assert_eq!(plies, vec![1, 2]);
//! ```

use crate::board::{Position, SfenError};
use crate::records::record::{Record, RecordEntry, RecordNodeId};
use crate::types::RepetitionState;
use thiserror::Error;

/// 棋譜走査中に発生するエラー。
#[derive(Debug, Error)]
pub enum TraversalError {
    /// 初期局面 SFEN の解析に失敗した。
    #[error("invalid initial position SFEN: {0}")]
    InvalidInitPosition(#[from] SfenError),
    /// 走査中に不正な指し手を検出した。
    #[error("illegal move at node {node_id} (ply {ply})")]
    IllegalMove {
        /// 不正手が見つかったノードの内部インデックス
        node_id: usize,
        /// 初期局面からの手数（半手）
        ply: usize,
    },
}

/// 棋譜走査中のノード情報。
///
/// [`traverse_with_position`] のコールバックに渡される。
pub struct TraversalNode<'a> {
    /// 現在のノードID
    pub node_id: RecordNodeId,
    /// 親ノードID
    pub parent_id: RecordNodeId,
    /// ノードが保持する記録
    pub entry: &'a RecordEntry,
    /// 指し手適用後の局面（終局ノードの場合は直前局面）
    pub position: &'a Position,
    /// 初期局面からの手数（半手）
    pub ply: usize,
}

struct Frame {
    node_id: RecordNodeId,
    next_child_index: usize,
    applied_move: Option<crate::types::Move32>,
    ply: usize,
}

/// [`Record`] を DFS で走査し、各ノードの局面を計算してコールバックに渡す。
///
/// コールバックが `false` を返すと、そのサブツリーの走査を打ち切る。
/// ルートノードはスキップされ、実際の指し手／終局ノードのみが渡される。
///
/// # Errors
///
/// - 初期局面 SFEN が不正な場合: [`TraversalError::InvalidInitPosition`]
/// - 走査中に不正な指し手が見つかった場合: [`TraversalError::IllegalMove`]
pub fn traverse_with_position<F>(record: &Record, mut visitor: F) -> Result<(), TraversalError>
where
    F: FnMut(&TraversalNode<'_>) -> bool,
{
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    let mut stack =
        vec![Frame { node_id: record.root_id(), next_child_index: 0, applied_move: None, ply: 0 }];

    while let Some(frame) = stack.last_mut() {
        let children = record.children(frame.node_id);
        if frame.next_child_index >= children.len() {
            let finished = stack.pop().expect("frame exists");
            if let Some(mv32) = finished.applied_move {
                pos.undo_move32(mv32).map_err(|_| TraversalError::IllegalMove {
                    node_id: finished.node_id.raw(),
                    ply: finished.ply,
                })?;
            }
            continue;
        }

        let child_id = children[frame.next_child_index];
        frame.next_child_index += 1;

        let node = record.node(child_id);
        let Some(entry) = node.entry() else {
            continue;
        };

        let parent_id = frame.node_id;
        let frame_ply = frame.ply;
        match entry {
            RecordEntry::Move(mv_record) => {
                let mv16 = mv_record.mv();
                let current_ply = frame_ply + 1;
                if !pos.is_legal_move(mv16) {
                    return Err(TraversalError::IllegalMove {
                        node_id: child_id.raw(),
                        ply: current_ply,
                    });
                }

                let mv32 = pos.move32_from_move(mv16);
                pos.apply_move32(mv32);

                let event = TraversalNode {
                    node_id: child_id,
                    parent_id,
                    entry,
                    position: &pos,
                    ply: current_ply,
                };
                let descend = visitor(&event);
                if descend {
                    stack.push(Frame {
                        node_id: child_id,
                        next_child_index: 0,
                        applied_move: Some(mv32),
                        ply: current_ply,
                    });
                } else {
                    pos.undo_move32(mv32).map_err(|_| TraversalError::IllegalMove {
                        node_id: child_id.raw(),
                        ply: current_ply,
                    })?;
                }
            }
            RecordEntry::Special(_) => {
                let event = TraversalNode {
                    node_id: child_id,
                    parent_id,
                    entry,
                    position: &pos,
                    ply: frame_ply,
                };
                if visitor(&event) {
                    stack.push(Frame {
                        node_id: child_id,
                        next_child_index: 0,
                        applied_move: None,
                        ply: frame_ply,
                    });
                }
            }
        }
    }

    Ok(())
}

/// 指定ノードまでの局面を計算する。
///
/// ルートから `target` まで親を辿り、初期局面から順に指し手を適用して
/// `target` ノードの指し手適用後の局面を返す。
///
/// # Errors
///
/// - 初期局面 SFEN が不正な場合: [`TraversalError::InvalidInitPosition`]
/// - 経路上に不正な指し手がある場合: [`TraversalError::IllegalMove`]
pub fn position_at(record: &Record, target: RecordNodeId) -> Result<Position, TraversalError> {
    let mut path = Vec::new();
    let mut current = Some(target);
    while let Some(id) = current {
        if id == record.root_id() {
            break;
        }
        path.push(id);
        current = record.node(id).parent();
    }
    path.reverse();

    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    for (idx, &node_id) in path.iter().enumerate() {
        let node = record.node(node_id);
        if let Some(mv_record) = node.mv() {
            let mv16 = mv_record.mv();
            if !pos.is_legal_move(mv16) {
                return Err(TraversalError::IllegalMove { node_id: node_id.raw(), ply: idx + 1 });
            }
            pos.apply_move(mv16);
        }
    }
    Ok(pos)
}

/// ルートから `target` までの経路の千日手状態を返す。
pub fn path_repetition_state(
    record: &Record,
    target: RecordNodeId,
) -> Result<RepetitionState, TraversalError> {
    Ok(position_at(record, target)?.repetition_state())
}

/// ルートから `target` までの経路が `threshold` で千日手かどうかを返す。
pub fn path_is_repetition(
    record: &Record,
    target: RecordNodeId,
    threshold: u8,
) -> Result<bool, TraversalError> {
    Ok(position_at(record, target)?.is_repetition(threshold))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::formats::kif;
    use crate::records::record::{MoveEntry, RecordEntry, SpecialMove, SpecialMoveEntry};
    use crate::types::GameResult;
    use crate::types::Move;

    #[test]
    fn test_traverse_main_line() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
まで2手で中断
";
        let record = kif::parse_kif_str(kif).unwrap();
        let mut visited_plies = Vec::new();
        traverse_with_position(&record, |node| {
            if let RecordEntry::Move(_) = node.entry {
                visited_plies.push(node.ply);
            }
            true
        })
        .unwrap();
        assert_eq!(visited_plies, vec![1, 2]);
    }

    #[test]
    fn test_traverse_with_variations() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();
        let mv_alt = Move::from_usi("8c8d").unwrap();

        let mut record = Record::new(sfen.to_string()).unwrap();
        let ids = record.extend_main_line(vec![MoveEntry::new(mv1), MoveEntry::new(mv2)]).unwrap();
        let parent_of_var = ids[0]; // mv1 適用後のノード
        record.add_variation_line(parent_of_var, vec![MoveEntry::new(mv_alt)]).unwrap();

        let mut visited = Vec::new();
        traverse_with_position(&record, |node| {
            if let RecordEntry::Move(mv_record) = node.entry {
                visited.push((node.ply, mv_record.mv().to_usi()));
            }
            true
        })
        .unwrap();

        // 本譜: 7g7f（手数 1）、3c3d（手数 2）
        // 手数 1 からの変化: 8c8d（手数 2）
        assert_eq!(visited.len(), 3);
        assert!(visited.contains(&(1, "7g7f".to_string())));
        assert!(visited.contains(&(2, "3c3d".to_string())));
        assert!(visited.contains(&(2, "8c8d".to_string())));
    }

    #[test]
    fn test_traverse_invalid_init_position() {
        let record = Record::new("invalid_sfen".to_string()).unwrap();
        let result = traverse_with_position(&record, |_| true);
        assert!(matches!(result, Err(TraversalError::InvalidInitPosition(_))));
    }

    #[test]
    fn test_position_at_main_line() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
まで2手で中断
";
        let record = kif::parse_kif_str(kif).unwrap();
        let main_ids = record.main_line_ids();
        assert_eq!(main_ids.len(), 2);

        let pos = position_at(&record, main_ids[1]).unwrap();
        let sfen = pos.to_sfen(None);
        // 2 手（先手・後手）進めた後は再び先手番になる
        assert!(sfen.contains(" b "), "should be black's turn after 2 half-moves");
    }

    #[test]
    fn test_path_repetition_helpers_default_to_none() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mut record = Record::new(sfen.to_string()).unwrap();
        let ids = record.extend_main_line(vec![MoveEntry::new(mv1)]).unwrap();

        assert_eq!(path_repetition_state(&record, ids[0]).unwrap(), RepetitionState::None);
        assert!(!path_is_repetition(&record, ids[0], 3).unwrap());
    }

    #[test]
    fn test_traverse_stop_descend() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();

        let mut record = Record::new(sfen.to_string()).unwrap();
        record.extend_main_line(vec![MoveEntry::new(mv1), MoveEntry::new(mv2)]).unwrap();

        let mut visited_plies = Vec::new();
        traverse_with_position(&record, |node| {
            visited_plies.push(node.ply);
            false // 子孫への走査を打ち切る
        })
        .unwrap();
        assert_eq!(visited_plies, vec![1]); // 最初の指し手のみ訪問される
    }

    /// A11: 局面付き走査。各ノードで局面が正しく計算されることを検証する。
    #[test]
    fn test_traverse_position_correctness() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();

        let mut record = Record::new(sfen.to_string()).unwrap();
        record.extend_main_line(vec![MoveEntry::new(mv1), MoveEntry::new(mv2)]).unwrap();

        let mut sfens = Vec::new();
        traverse_with_position(&record, |node| {
            if let RecordEntry::Move(_) = node.entry {
                sfens.push(node.position.to_sfen(None));
            }
            true
        })
        .unwrap();

        // SFEN が手動適用結果と一致することを確認する
        let mut pos = Position::empty();
        pos.set_sfen(sfen).unwrap();
        pos.apply_move(mv1);
        assert_eq!(sfens[0], pos.to_sfen(None));
        pos.apply_move(mv2);
        assert_eq!(sfens[1], pos.to_sfen(None));
    }

    /// A11: 局面付き走査。変化手順の局面が本譜と独立していることを検証する。
    #[test]
    fn test_traverse_variation_position_independent() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv_main = Move::from_usi("3c3d").unwrap();
        let mv_var = Move::from_usi("8c8d").unwrap();

        let mut record = Record::new(sfen.to_string()).unwrap();
        let ids =
            record.extend_main_line(vec![MoveEntry::new(mv1), MoveEntry::new(mv_main)]).unwrap();
        record.add_variation_line(ids[0], vec![MoveEntry::new(mv_var)]).unwrap();

        let mut positions_at_ply2 = Vec::new();
        traverse_with_position(&record, |node| {
            if let RecordEntry::Move(mv_record) = node.entry
                && node.ply == 2
            {
                positions_at_ply2.push((mv_record.mv().to_usi(), node.position.to_sfen(None)));
            }
            true
        })
        .unwrap();

        assert_eq!(positions_at_ply2.len(), 2);
        // 手数 2 の 2 局面は異なる（手数 1 で異なる指し手を適用したため）
        assert_ne!(positions_at_ply2[0].1, positions_at_ply2[1].1);
    }

    /// A12: 走査エラーモデル。不正な指し手の検出を検証する。
    #[test]
    fn test_traverse_illegal_move_error() {
        // 手動で不正な指し手を含む棋譜を構築する
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let legal_mv = Move::from_usi("7g7f").unwrap();
        // 7g7f の後は合法だが、同じ局面に再適用すると不正手になる
        let bad_mv = Move::from_usi("7g7f").unwrap(); // 同一指し手の再適用 = 不正手

        let mut record = Record::new(sfen.to_string()).unwrap();
        record.extend_main_line(vec![MoveEntry::new(legal_mv), MoveEntry::new(bad_mv)]).unwrap();

        let result = traverse_with_position(&record, |_| true);
        assert!(matches!(result, Err(TraversalError::IllegalMove { ply: 2, .. })));
    }

    #[test]
    fn test_reject_append_under_special_node() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();

        let mut record = Record::new(sfen.to_string()).unwrap();
        record.extend_main_line(vec![MoveEntry::new(mv1)]).unwrap();
        let terminal = record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Invalid))
            .unwrap();
        let result = record.append_move(terminal, MoveEntry::new(mv2));

        assert!(matches!(result, Err(crate::records::RecordError::TerminalNode(_))));
    }
}
