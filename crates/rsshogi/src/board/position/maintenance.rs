use super::Position;
use crate::board::bitboard_set::BitboardSet;
use crate::types::{
    AperyMove32, Color, File, MOVE_END, MOVE_NONE, MOVE_NULL, MOVE_RESIGN, MOVE_WIN, Move, Move32,
    Piece, PieceType, Rank, Square,
};

fn square_from_csa(s: &str) -> Option<Square> {
    if s.len() != 2 {
        return None;
    }
    let mut chars = s.chars();
    let file_char = chars.next()?;
    let rank_char = chars.next()?;
    if !file_char.is_ascii_digit() || !rank_char.is_ascii_digit() {
        return None;
    }
    let file_digit = file_char.to_digit(10)?;
    let rank_digit = rank_char.to_digit(10)?;
    if !(1..=9).contains(&file_digit) || !(1..=9).contains(&rank_digit) {
        return None;
    }
    let file = File::new(i8::try_from(file_digit - 1).ok()?);
    let rank = Rank::new(i8::try_from(rank_digit - 1).ok()?);
    Some(Square::from_file_rank(file, rank))
}

fn piece_type_from_csa(code: &str) -> Option<PieceType> {
    match code {
        "FU" => Some(PieceType::PAWN),
        "KY" => Some(PieceType::LANCE),
        "KE" => Some(PieceType::KNIGHT),
        "GI" => Some(PieceType::SILVER),
        "KI" => Some(PieceType::GOLD),
        "KA" => Some(PieceType::BISHOP),
        "HI" => Some(PieceType::ROOK),
        "OU" => Some(PieceType::KING),
        "TO" => Some(PieceType::PRO_PAWN),
        "NY" => Some(PieceType::PRO_LANCE),
        "NK" => Some(PieceType::PRO_KNIGHT),
        "NG" => Some(PieceType::PRO_SILVER),
        "UM" => Some(PieceType::HORSE),
        "RY" => Some(PieceType::DRAGON),
        _ => None,
    }
}

impl Position {
    /// 置換表や定跡から取り出した `Move` を盤面依存の `Move32` に拡張
    #[must_use]
    pub fn move32_from_move(&self, mv: Move) -> Move32 {
        const MASK_7: u16 = 0x7f;
        let raw = mv.raw();
        if raw == Move::MOVE_NONE.raw() {
            return MOVE_NONE;
        }
        if raw == Move::MOVE_NULL.raw() {
            return MOVE_NULL;
        }
        if raw == Move::MOVE_RESIGN.raw() {
            return MOVE_RESIGN;
        }
        if raw == Move::MOVE_WIN.raw() {
            return MOVE_WIN;
        }
        if raw == Move::MOVE_END.raw() {
            return MOVE_END;
        }

        let to_raw = raw & MASK_7;
        if to_raw > 80 {
            return MOVE_NONE;
        }

        if (raw & Move::MOVE_DROP) != 0 {
            let pt_raw = (raw >> 7) & MASK_7;
            if !(1..=7).contains(&pt_raw) {
                return MOVE_NONE;
            }
            let piece_type = PieceType::new(pt_raw as i8);
            return Move32::drop(piece_type, Square::new(to_raw as i8), self.turn());
        }

        let from_raw = (raw >> 7) & MASK_7;
        if from_raw > 80 {
            return MOVE_NONE;
        }

        let from = Square::new(from_raw as i8);
        let to = Square::new(to_raw as i8);
        // SAFETY: 直上で `from_raw <= 80` を確認済みのため、`from` は盤上の有効な座標である。
        let piece = unsafe { self.piece_on_unchecked(from) };
        if piece == Piece::NONE || piece.color() != self.turn() {
            return MOVE_NONE;
        }

        if (raw & Move::MOVE_PROMOTE) != 0 {
            if !piece.piece_type().is_promotable() {
                return MOVE_NONE;
            }
            return Move32::promotion(from, to, piece);
        }

        Move32::normal(from, to, piece)
    }

    /// `Move` を cshogi / Apery 互換の 32bit 指し手に拡張する。
    #[must_use]
    pub fn apery_move32_from_move(&self, mv: Move) -> AperyMove32 {
        let apery = mv.to_apery();
        if !mv.is_normal() {
            return AperyMove32::from_raw(u32::from(apery.raw()));
        }
        if mv.is_drop() {
            // Apery/cshogi の駒打ちでは piece_type_before を使わない。打った駒種は
            // 下位 16bit（`from >= 81`）にエンコードされ、`piece_type_after()` がそこから導出する。
            return AperyMove32::from_raw(u32::from(apery.raw()));
        }

        let from = mv.from_sq();
        let to = mv.to_sq();
        let piece_type_before = self.piece_on(from).piece_type();
        let captured_piece_type = self.piece_on(to).piece_type();
        let raw = u32::from(apery.raw())
            | (u32::from(piece_type_before.raw() as u8 & 0xf) << 16)
            | (u32::from(captured_piece_type.raw() as u8 & 0xf) << 20);
        AperyMove32::from_raw(raw)
    }

    /// `Move32` を cshogi / Apery 互換の 32bit 指し手に拡張する。
    ///
    /// `Move32` には取得駒種が入っていないため、局面情報から補完する。
    #[must_use]
    pub fn apery_move32_from_move32(&self, mv: Move32) -> AperyMove32 {
        self.apery_move32_from_move(mv.to_move())
    }

    /// cshogi / Apery 互換の 32bit 指し手を局面依存の `Move32` に復元する。
    #[must_use]
    pub fn move32_from_apery_move32(&self, mv: AperyMove32) -> Move32 {
        let base = mv.to_move();
        if !base.is_normal() {
            return Move32::from_raw(u32::from(base.raw()));
        }
        self.move32_from_move(base)
    }

    /// センチネルチェックなしの高速 Move → Move32 変換。
    ///
    /// # Safety
    /// - `mv` が合法手生成で生成された有効な移動手または駒打ち手であること。
    /// - センチネル（`MOVE_NONE`/`NULL`/`RESIGN`/`WIN`/`END`）を渡してはならない。
    /// - `from` マスが盤上の有効な座標であること（`piece_on_unchecked` を呼ぶため）。
    #[inline]
    pub(crate) unsafe fn move32_from_move_fast(&self, mv: Move) -> Move32 {
        unsafe {
            const MASK_7: u16 = 0x7f;
            let raw = mv.raw();
            let to = Square::new((raw & MASK_7) as i8);
            debug_assert!(to.is_on_board(), "move32_from_move_fast: to must be on board");

            if (raw & Move::MOVE_DROP) != 0 {
                let pt_raw = (raw >> 7) & MASK_7;
                let piece_type = PieceType::new(pt_raw as i8);
                return Move32::drop(piece_type, to, self.turn());
            }

            let from = Square::new(((raw >> 7) & MASK_7) as i8);
            debug_assert!(from.is_on_board(), "move32_from_move_fast: from must be on board");
            // SAFETY: 呼び出し側が `mv` を合法手生成で得た盤上の指し手であることを保証する。
            // デバッグビルドでは debug_assert が盤上座標の前提条件を文書化する。
            let piece = self.piece_on_unchecked(from);

            if (raw & Move::MOVE_PROMOTE) != 0 {
                Move32::promotion(from, to, piece)
            } else {
                Move32::normal(from, to, piece)
            }
        }
    }

    /// CSA形式の文字列から `Move32` を生成
    ///
    /// 先頭の手番記号（'+' / '-'）が含まれていても受け付ける。
    #[must_use]
    pub fn move_from_csa(&self, csa: &str) -> Move32 {
        let mut csa = csa;
        if csa.len() == 7 {
            let sign = match csa.chars().next() {
                Some(sign) if sign == '+' || sign == '-' => sign,
                _ => return MOVE_NONE,
            };
            let side = self.turn();
            if (sign == '+' && side != Color::BLACK) || (sign == '-' && side != Color::WHITE) {
                return MOVE_NONE;
            }
            csa = &csa[1..];
        }
        if csa.len() != 6 {
            return MOVE_NONE;
        }

        let (Some(from_str), Some(to_str), Some(piece_str)) =
            (csa.get(0..2), csa.get(2..4), csa.get(4..6))
        else {
            return MOVE_NONE;
        };
        let Some(to) = square_from_csa(to_str) else {
            return MOVE_NONE;
        };
        let Some(piece_type_after) = piece_type_from_csa(piece_str) else {
            return MOVE_NONE;
        };

        if from_str == "00" {
            let drop_pt = piece_type_after.demote();
            if drop_pt.raw() < PieceType::PAWN.raw() || drop_pt.raw() > PieceType::GOLD.raw() {
                return MOVE_NONE;
            }
            return Move32::drop(drop_pt, to, self.turn());
        }

        let Some(from) = square_from_csa(from_str) else {
            return MOVE_NONE;
        };
        let piece = self.piece_on(from);
        if piece == Piece::NONE || piece.color() != self.turn() {
            return MOVE_NONE;
        }

        let mover_pt = piece.piece_type();
        let mover_base = mover_pt.demote();
        let after_base = piece_type_after.demote();
        if mover_base != after_base {
            return MOVE_NONE;
        }
        if mover_pt.is_promoted() && !piece_type_after.is_promoted() {
            return MOVE_NONE;
        }
        if !mover_pt.is_promoted() && piece_type_after.is_promoted() {
            return Move32::promotion(from, to, piece);
        }

        Move32::normal(from, to, piece)
    }

    /// ビットボードを盤面配列から再構築
    pub(in crate::board) fn rebuild_bitboards(&mut self) {
        self.bitboards = BitboardSet::new();
        let mut king_squares = [Square::NONE; Color::COUNT];

        for (sq, piece) in self.board.iter() {
            if !piece.is_empty() {
                let piece_type = piece.piece_type();
                let color = piece.color();
                self.bitboards.set_piece(sq, piece_type, color);
                if piece_type == PieceType::KING {
                    king_squares[color.to_index()] = sq;
                }
            }
        }

        for color in [Color::BLACK, Color::WHITE] {
            self.set_king_square(color, king_squares[color.to_index()]);
        }
        self.recompute_caches();
    }
}
