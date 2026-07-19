//! 外部実装との定数値互換性を検証する。

use rsshogi::types::*;

#[cfg(test)]
mod color_constants {
    use super::*;

    #[test]
    fn test_color_values() {
        assert_eq!(Color::BLACK.raw(), 0);
        assert_eq!(Color::WHITE.raw(), 1);
        assert_eq!(Color::ZERO.raw(), 0);
        assert_eq!(Color::COUNT, 2);
    }
}

#[cfg(test)]
mod file_rank_constants {
    use super::*;

    #[test]
    fn test_file_values() {
        // File::FILE_1, File::FILE_2, ..., File::FILE_9, File::COUNT, File::FILE_ZERO = 0
        assert_eq!(File::FILE_1.raw(), 0);
        assert_eq!(File::FILE_2.raw(), 1);
        assert_eq!(File::FILE_3.raw(), 2);
        assert_eq!(File::FILE_4.raw(), 3);
        assert_eq!(File::FILE_5.raw(), 4);
        assert_eq!(File::FILE_6.raw(), 5);
        assert_eq!(File::FILE_7.raw(), 6);
        assert_eq!(File::FILE_8.raw(), 7);
        assert_eq!(File::FILE_9.raw(), 8);
        assert_eq!(File::COUNT, 9);
        assert_eq!(File::ZERO.raw(), 0);
    }

    #[test]
    fn test_rank_values() {
        // Rank::RANK_1, Rank::RANK_2, ..., Rank::RANK_9, Rank::COUNT, Rank::RANK_ZERO = 0
        assert_eq!(Rank::RANK_1.raw(), 0);
        assert_eq!(Rank::RANK_2.raw(), 1);
        assert_eq!(Rank::RANK_3.raw(), 2);
        assert_eq!(Rank::RANK_4.raw(), 3);
        assert_eq!(Rank::RANK_5.raw(), 4);
        assert_eq!(Rank::RANK_6.raw(), 5);
        assert_eq!(Rank::RANK_7.raw(), 6);
        assert_eq!(Rank::RANK_8.raw(), 7);
        assert_eq!(Rank::RANK_9.raw(), 8);
        assert_eq!(Rank::COUNT, 9);
        assert_eq!(Rank::ZERO.raw(), 0);
    }
}

#[cfg(test)]
mod square_constants {
    use super::*;

    #[test]
    fn test_square_values() {
        // SQ_11 = 0, SQ_12, ..., SQ_99 = 80
        // Square::COUNT = 81, Square::COUNT_WITH_NONE = 82, SQ_NONE = 81, SQ_ZERO = 0
        assert_eq!(SQ_11.raw(), 0);
        assert_eq!(SQ_12.raw(), 1);
        assert_eq!(SQ_19.raw(), 8);
        assert_eq!(SQ_21.raw(), 9);
        assert_eq!(SQ_91.raw(), 72);
        assert_eq!(SQ_99.raw(), 80);
        assert_eq!(Square::COUNT, 81);
        assert_eq!(Square::COUNT_WITH_NONE, 82);
        assert_eq!(SQ_NONE.raw(), 81);
        assert_eq!(SQ_ZERO.raw(), 0);
    }

    #[test]
    fn test_square_directions() {
        // DIRECT_D = +1, DIRECT_U = -1, DIRECT_R = -9, DIRECT_L = +9
        assert_eq!(SQ_D, 1); // 下（段を増やす）
        assert_eq!(SQ_U, -1); // 上（段を減らす）
        assert_eq!(SQ_R, -9); // 右（筋を減らす）
        assert_eq!(SQ_L, 9); // 左（筋を増やす）
        assert_eq!(SQ_RU, -10); // 右上
        assert_eq!(SQ_RD, -8); // 右下
        assert_eq!(SQ_LU, 8); // 左上
        assert_eq!(SQ_LD, 10); // 左下
    }
}

#[cfg(test)]
mod piece_type_constants {
    use super::*;

    #[test]
    fn test_piece_type_values() {
        assert_eq!(PieceType::NONE.raw(), 0);
        assert_eq!(PieceType::PAWN.raw(), 1);
        assert_eq!(PieceType::LANCE.raw(), 2);
        assert_eq!(PieceType::KNIGHT.raw(), 3);
        assert_eq!(PieceType::SILVER.raw(), 4);
        assert_eq!(PieceType::BISHOP.raw(), 5);
        assert_eq!(PieceType::ROOK.raw(), 6);
        assert_eq!(PieceType::GOLD.raw(), 7);
        assert_eq!(PieceType::KING.raw(), 8);
        assert_eq!(PieceType::PRO_PAWN.raw(), 9);
        assert_eq!(PieceType::PRO_LANCE.raw(), 10);
        assert_eq!(PieceType::PRO_KNIGHT.raw(), 11);
        assert_eq!(PieceType::PRO_SILVER.raw(), 12);
        assert_eq!(PieceType::HORSE.raw(), 13);
        assert_eq!(PieceType::DRAGON.raw(), 14);
        assert_eq!(PieceType::GOLD_LIKE.raw(), 15); // 金相当（実際には盤上に存在しない）
        assert_eq!(PieceType::PIECE_TYPE_PROMOTE, 8);
        assert_eq!(PieceType::COUNT, 16);
        assert_eq!(PieceType::PIECE_HAND_ZERO.raw(), 1);
        assert_eq!(PieceType::HAND_TABLE_SIZE, 8);
    }
}

#[cfg(test)]
mod piece_constants {
    use super::*;

    #[test]
    fn test_piece_values() {
        assert_eq!(Piece::NONE.raw(), 0);

        let black_pieces = [
            (Piece::B_PAWN, 1),
            (Piece::B_LANCE, 2),
            (Piece::B_KNIGHT, 3),
            (Piece::B_SILVER, 4),
            (Piece::B_BISHOP, 5),
            (Piece::B_ROOK, 6),
            (Piece::B_GOLD, 7),
            (Piece::B_KING, 8),
            (Piece::B_PRO_PAWN, 9),
            (Piece::B_PRO_LANCE, 10),
            (Piece::B_PRO_KNIGHT, 11),
            (Piece::B_PRO_SILVER, 12),
            (Piece::B_HORSE, 13),
            (Piece::B_DRAGON, 14),
            (Piece::B_GOLD_LIKE, 15),
        ];
        for (piece, expected) in black_pieces {
            assert_eq!(piece.raw(), expected);
        }

        let white_pieces = [
            (Piece::W_PAWN, 17),
            (Piece::W_LANCE, 18),
            (Piece::W_KNIGHT, 19),
            (Piece::W_SILVER, 20),
            (Piece::W_BISHOP, 21),
            (Piece::W_ROOK, 22),
            (Piece::W_GOLD, 23),
            (Piece::W_KING, 24),
            (Piece::W_PRO_PAWN, 25),
            (Piece::W_PRO_LANCE, 26),
            (Piece::W_PRO_KNIGHT, 27),
            (Piece::W_PRO_SILVER, 28),
            (Piece::W_HORSE, 29),
            (Piece::W_DRAGON, 30),
            (Piece::W_GOLD_LIKE, 31),
        ];
        for (piece, expected) in white_pieces {
            assert_eq!(piece.raw(), expected);
        }

        // フラグ値
        assert_eq!(Piece::COLOR_BIT, 16);
        assert_eq!(Piece::PROMOTE_OFFSET, 8);
        assert_eq!(Piece::COUNT, 32);
    }
}

#[cfg(test)]
mod move_encoding {
    use super::*;

    #[test]
    fn test_move_special_values() {
        assert_eq!(Move::MOVE_NONE.raw(), 0);
        assert_eq!(Move::MOVE_NULL.raw(), (1 << 7) + 1);
        assert_eq!(Move::MOVE_RESIGN.raw(), (2 << 7) + 2);
        assert_eq!(Move::MOVE_WIN.raw(), (3 << 7) + 3);
    }

    #[test]
    fn test_move_flags() {
        assert_eq!(Move::MOVE_DROP, 1 << 14);
        assert_eq!(Move::MOVE_PROMOTE, 1 << 15);
    }

    #[test]
    fn test_move_encoding_structure() {
        // 通常移動: to(7bit) | from(7bit)
        // 駒打ち: to(7bit) | piece(4bit) | DROP_FLAG
        // 成り: ... | PROMOTE_FLAG

        // 通常移動のテスト (7g7f)
        let from = SQ_77; // 7g (7七)
        let to = SQ_76; // 7f (7六)
        let m = Move::normal(from, to);
        assert_eq!(m.to_sq(), to);
        assert_eq!(m.from_sq(), from);
        assert!(!m.is_drop());
        assert!(!m.is_promotion());

        // 駒打ちのテスト (P*5e)
        let to = SQ_55; // 5e (5五)
        let m = Move::drop(PieceType::PAWN, to);
        assert_eq!(m.to_sq(), to);
        assert_eq!(m.dropped_piece(), Some(PieceType::PAWN));
        assert!(m.is_drop());
        assert!(!m.is_promotion());

        // 成りのテスト (2b3c+)
        let from = SQ_22; // 2b (2二)
        let to = SQ_33; // 3c (3三)
        let m = Move::promotion(from, to);
        assert_eq!(m.to_sq(), to);
        assert_eq!(m.from_sq(), from);
        assert!(!m.is_drop());
        assert!(m.is_promotion());
    }
}

#[cfg(test)]
mod hand_constants {
    use super::*;

    #[test]
    fn test_hand_piece_values() {
        assert_eq!(HandPiece::PAWN.raw(), 0);
        assert_eq!(HandPiece::LANCE.raw(), 1);
        assert_eq!(HandPiece::KNIGHT.raw(), 2);
        assert_eq!(HandPiece::SILVER.raw(), 3);
        assert_eq!(HandPiece::BISHOP.raw(), 4);
        assert_eq!(HandPiece::ROOK.raw(), 5);
        assert_eq!(HandPiece::GOLD.raw(), 6);
        assert_eq!(HandPiece::COUNT, 7);
    }

    #[test]
    fn test_hand_encoding() {
        // 歩(5bit) | 香(3bit) | 桂(3bit) | 銀(3bit) | 角(2bit) | 飛(2bit) | 金(3bit)
        // ビット位置: 0-4: 歩, 8-10: 香, 12-14: 桂, 16-18: 銀, 20-21: 角, 24-25: 飛, 28-30: 金

        let mut hand = Hand::ZERO;

        // 各駒を1枚ずつ追加
        hand.add(HandPiece::PAWN, 1);
        assert_eq!(hand.count(HandPiece::PAWN), 1);

        hand.add(HandPiece::LANCE, 1);
        assert_eq!(hand.count(HandPiece::LANCE), 1);

        hand.add(HandPiece::KNIGHT, 1);
        assert_eq!(hand.count(HandPiece::KNIGHT), 1);

        hand.add(HandPiece::SILVER, 1);
        assert_eq!(hand.count(HandPiece::SILVER), 1);

        hand.add(HandPiece::GOLD, 1);
        assert_eq!(hand.count(HandPiece::GOLD), 1);

        hand.add(HandPiece::BISHOP, 1);
        assert_eq!(hand.count(HandPiece::BISHOP), 1);

        hand.add(HandPiece::ROOK, 1);
        assert_eq!(hand.count(HandPiece::ROOK), 1);
    }

    #[test]
    fn test_hand_bit_masks() {
        assert_eq!(
            HAND_BIT_MASK,
            0x1f | (0x7 << 8) | (0x7 << 12) | (0x7 << 16) | (0x3 << 20) | (0x3 << 24) | (0x7 << 28)
        );
        // 各駒種の借位ビット位置
        // 歩(bit0-4)→bit5, 香(bit8-10)→bit11, 桂(bit12-14)→bit15,
        // 銀(bit16-18)→bit19, 角(bit20-21)→bit22, 飛(bit24-25)→bit26, 金(bit28-30)→bit31
        assert_eq!(
            HAND_BORROW_MASK,
            (1u32 << 5)
                | (1u32 << 11)
                | (1u32 << 15)
                | (1u32 << 19)
                | (1u32 << 22)
                | (1u32 << 26)
                | (1u32 << 31)
        );
    }
}

#[cfg(test)]
mod sfen_compatibility {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_piece_sfen_strings() {
        let black_cases = [
            (Piece::B_PAWN, "P"),
            (Piece::B_LANCE, "L"),
            (Piece::B_KNIGHT, "N"),
            (Piece::B_SILVER, "S"),
            (Piece::B_GOLD, "G"),
            (Piece::B_BISHOP, "B"),
            (Piece::B_ROOK, "R"),
            (Piece::B_KING, "K"),
            (Piece::B_PRO_PAWN, "+P"),
            (Piece::B_PRO_LANCE, "+L"),
            (Piece::B_PRO_KNIGHT, "+N"),
            (Piece::B_PRO_SILVER, "+S"),
            (Piece::B_HORSE, "+B"),
            (Piece::B_DRAGON, "+R"),
        ];
        for (piece, expected) in black_cases {
            assert_eq!(piece.to_string(), expected);
        }

        let white_cases = [
            (Piece::W_PAWN, "p"),
            (Piece::W_LANCE, "l"),
            (Piece::W_KNIGHT, "n"),
            (Piece::W_SILVER, "s"),
            (Piece::W_GOLD, "g"),
            (Piece::W_BISHOP, "b"),
            (Piece::W_ROOK, "r"),
            (Piece::W_KING, "k"),
            (Piece::W_PRO_PAWN, "+p"),
            (Piece::W_PRO_LANCE, "+l"),
            (Piece::W_PRO_KNIGHT, "+n"),
            (Piece::W_PRO_SILVER, "+s"),
            (Piece::W_HORSE, "+b"),
            (Piece::W_DRAGON, "+r"),
        ];
        for (piece, expected) in white_cases {
            assert_eq!(piece.to_string(), expected);
        }
    }

    #[test]
    fn test_square_usi_strings() {
        assert_eq!(SQ_11.to_string(), "1a"); // 1一 = File(0), Rank(0)
        assert_eq!(SQ_55.to_string(), "5e"); // 5五 = File(4), Rank(4)
        assert_eq!(SQ_99.to_string(), "9i"); // 9九 = File(8), Rank(8)

        // パース
        assert_eq!(Square::from_str("1a").unwrap(), SQ_11); // 1一
        assert_eq!(Square::from_str("5e").unwrap(), SQ_55); // 5五
        assert_eq!(Square::from_str("9i").unwrap(), SQ_99); // 9九
    }

    #[test]
    fn test_move_usi_strings() {
        // 通常移動
        let m = Move::normal(SQ_77, SQ_76);
        assert_eq!(m.to_usi(), "7g7f");

        // 駒打ち
        let m = Move::drop(PieceType::PAWN, SQ_55);
        assert_eq!(m.to_usi(), "P*5e");

        // 成り
        let m = Move::promotion(SQ_22, SQ_33);
        assert_eq!(m.to_usi(), "2b3c+"); // SQ_22=2二 → "2b", SQ_33=3三 → "3c"

        // パース
        assert_eq!(Move::from_usi("7g7f").unwrap().to_usi(), "7g7f");
        assert_eq!(Move::from_usi("P*5e").unwrap().to_usi(), "P*5e");
        assert_eq!(Move::from_usi("2b3c+").unwrap().to_usi(), "2b3c+");
    }
}
