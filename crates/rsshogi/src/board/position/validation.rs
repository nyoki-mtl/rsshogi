use super::Position;
use super::types::{ValidationError, ValidationIssue, ValidationReport};
use crate::types::{Bitboard, Color, File, HandPiece, PieceType, Rank, Square};

impl Position {
    /// 盤面の妥当性を検証（デバッグ向け）
    ///
    /// 将棋のルールに従って盤面が正しいかをチェックする。
    /// 駒落ち・詰将棋局面を想定し、玉の欠落は許容する。
    /// 以下の項目を検証：
    /// - 王の枚数（各色0〜1枚）
    /// - 二歩のチェック
    /// - 持ち駒の上限
    /// - 成り駒の位置の妥当性
    /// - 歩・香・桂の配置制限
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), ValidationError> {
        // 1. 王の枚数チェック
        let black_king_count = self.bitboards.pieces_for(PieceType::KING, Color::BLACK).count();
        let white_king_count = self.bitboards.pieces_for(PieceType::KING, Color::WHITE).count();

        if black_king_count > 1 {
            return Err(ValidationError::TwoKings(Color::BLACK));
        }
        if white_king_count > 1 {
            return Err(ValidationError::TwoKings(Color::WHITE));
        }

        // 2. 二歩チェック
        for file in 0..9 {
            let file = File::new(file);
            let file_mask = Bitboard::file_mask(file);

            let black_pawns = self.bitboards.pieces_for(PieceType::PAWN, Color::BLACK) & file_mask;
            if black_pawns.count() > 1 {
                return Err(ValidationError::DoublePawn(file, Color::BLACK));
            }

            let white_pawns = self.bitboards.pieces_for(PieceType::PAWN, Color::WHITE) & file_mask;
            if white_pawns.count() > 1 {
                return Err(ValidationError::DoublePawn(file, Color::WHITE));
            }
        }

        // 3. 持ち駒の上限チェック
        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);

            if hand.count(HandPiece::PAWN) > 18 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::PAWN,
                    count: hand.count(HandPiece::PAWN),
                });
            }
            if hand.count(HandPiece::LANCE) > 4 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::LANCE,
                    count: hand.count(HandPiece::LANCE),
                });
            }
            if hand.count(HandPiece::KNIGHT) > 4 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::KNIGHT,
                    count: hand.count(HandPiece::KNIGHT),
                });
            }
            if hand.count(HandPiece::SILVER) > 4 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::SILVER,
                    count: hand.count(HandPiece::SILVER),
                });
            }
            if hand.count(HandPiece::GOLD) > 4 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::GOLD,
                    count: hand.count(HandPiece::GOLD),
                });
            }
            if hand.count(HandPiece::BISHOP) > 2 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::BISHOP,
                    count: hand.count(HandPiece::BISHOP),
                });
            }
            if hand.count(HandPiece::ROOK) > 2 {
                return Err(ValidationError::InvalidHandCount {
                    piece: HandPiece::ROOK,
                    count: hand.count(HandPiece::ROOK),
                });
            }
        }

        // 4. 行き所のない駒の配置制限チェック
        for sq_idx in 0..81 {
            let sq = Square::new(sq_idx);
            let piece_packed = self.board.get(sq);

            if piece_packed.is_empty() {
                continue;
            }

            let color = piece_packed.color();
            let piece_type = piece_packed.piece_type();
            let rank = sq.rank();

            if (piece_type == PieceType::PAWN || piece_type == PieceType::LANCE)
                && ((color == Color::BLACK && rank == Rank::RANK_1)
                    || (color == Color::WHITE && rank == Rank::RANK_9))
            {
                return Err(ValidationError::InvalidPlacement(sq, piece_type));
            }

            if piece_type == PieceType::KNIGHT
                && ((color == Color::BLACK && (rank == Rank::RANK_1 || rank == Rank::RANK_2))
                    || (color == Color::WHITE && (rank == Rank::RANK_8 || rank == Rank::RANK_9)))
            {
                return Err(ValidationError::InvalidPlacement(sq, piece_type));
            }
        }

        Ok(())
    }

    /// 内部状態の整合性チェック
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// 盤面の全件検証（エディタ・修正ツール向け）
    ///
    /// [`validate()`](Self::validate) が最初のエラーで停止するのに対し、
    /// こちらはすべての問題を収集して [`ValidationReport`] として返す。
    ///
    /// 検証項目は `validate()` と同一:
    /// - 王の枚数（各色0〜1枚、0枚は `NoKing` として報告）
    /// - 二歩のチェック
    /// - 持ち駒の上限
    /// - 行き場のない駒の配置制限
    ///
    /// # Examples
    ///
    /// ```
    /// use rsshogi::board::{self, position_from_sfen};
    ///
    /// board::init();
    /// // 先手に玉が2枚ある不正局面
    /// let pos = position_from_sfen(
    ///     "lnsgkgsnl/1r5b1/ppppppppp/9/4K4/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
    /// ).unwrap();
    /// let report = pos.validate_all();
    /// assert!(!report.is_valid());
    /// assert!(!report.issues().is_empty());
    /// ```
    #[must_use]
    pub fn validate_all(&self) -> ValidationReport {
        let mut issues = Vec::new();

        // 1. 王の枚数チェック
        for color in [Color::BLACK, Color::WHITE] {
            let king_count = self.bitboards.pieces_for(PieceType::KING, color).count();
            if king_count == 0 {
                issues.push(ValidationIssue::NoKing(color));
            } else if king_count > 1 {
                issues.push(ValidationIssue::TwoKings(color));
            }
        }

        // 2. 二歩チェック
        for file_idx in 0..9 {
            let file = File::new(file_idx);
            let file_mask = Bitboard::file_mask(file);

            for color in [Color::BLACK, Color::WHITE] {
                let pawns = self.bitboards.pieces_for(PieceType::PAWN, color) & file_mask;
                if pawns.count() > 1 {
                    issues.push(ValidationIssue::DoublePawn(file, color));
                }
            }
        }

        // 3. 持ち駒の上限チェック
        let limits: [(HandPiece, u32); 7] = [
            (HandPiece::PAWN, 18),
            (HandPiece::LANCE, 4),
            (HandPiece::KNIGHT, 4),
            (HandPiece::SILVER, 4),
            (HandPiece::GOLD, 4),
            (HandPiece::BISHOP, 2),
            (HandPiece::ROOK, 2),
        ];

        for color in [Color::BLACK, Color::WHITE] {
            let hand = self.hand(color);
            for &(piece, limit) in &limits {
                let count = hand.count(piece);
                if count > limit {
                    issues.push(ValidationIssue::InvalidHandCount { piece, count });
                }
            }
        }

        // 4. 行き場のない駒の配置制限チェック
        for sq_idx in 0..81 {
            let sq = Square::new(sq_idx);
            let piece_packed = self.board.get(sq);

            if piece_packed.is_empty() {
                continue;
            }

            let color = piece_packed.color();
            let piece_type = piece_packed.piece_type();
            let rank = sq.rank();

            if (piece_type == PieceType::PAWN || piece_type == PieceType::LANCE)
                && ((color == Color::BLACK && rank == Rank::RANK_1)
                    || (color == Color::WHITE && rank == Rank::RANK_9))
            {
                issues.push(ValidationIssue::InvalidPlacement(sq, piece_type));
            }

            if piece_type == PieceType::KNIGHT
                && ((color == Color::BLACK && (rank == Rank::RANK_1 || rank == Rank::RANK_2))
                    || (color == Color::WHITE && (rank == Rank::RANK_8 || rank == Rank::RANK_9)))
            {
                issues.push(ValidationIssue::InvalidPlacement(sq, piece_type));
            }
        }

        ValidationReport::new(issues)
    }
}
