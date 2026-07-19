use crate::board::state_info::PartialKeys;
use crate::board::zobrist::ZobristKey;
use crate::types::{Color, EnteringKingRule, File, HandPiece, Piece, PieceType, Square};
use std::convert::TryFrom;
use std::fmt;

/// USIプロトコルで使用される手数型
pub type Ply = u16;

/// 盤面の1マスを1バイトで表現する型
///
/// エンコーディング:
/// - bit 0-3: 駒種（PieceType: 0=なし, 1=歩, 2=香, 3=桂, 4=銀, 5=角, 6=飛, 7=金, 8=玉, 9-14=成駒）
/// - bit 4: 先後（0=先手, 1=後手）
/// - bit 5: 成りフラグ
/// - bit 6-7: 予約（0固定）
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct PackedPiece(u8);

const PIECE_TYPE_FROM_INDEX: [PieceType; PieceType::COUNT] = [
    PieceType::NONE,
    PieceType::PAWN,
    PieceType::LANCE,
    PieceType::KNIGHT,
    PieceType::SILVER,
    PieceType::BISHOP,
    PieceType::ROOK,
    PieceType::GOLD,
    PieceType::KING,
    PieceType::PRO_PAWN,
    PieceType::PRO_LANCE,
    PieceType::PRO_KNIGHT,
    PieceType::PRO_SILVER,
    PieceType::HORSE,
    PieceType::DRAGON,
    // GOLDSは盤上に置かれないが、配列サイズ互換のため保持する。
    PieceType::GOLD_LIKE,
];

impl PackedPiece {
    /// 空のマス
    pub const EMPTY: Self = Self(0);

    /// 内部表現（1バイト）を取得
    ///
    /// `PackedPiece` は newtype として内部表現を隠蔽する方針だが、
    /// デバッグやテストなどでの確認用に最小限の読み出し API を提供する。
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// `PackedPiece` を作成
    #[inline]
    #[must_use]
    pub fn new(piece_type: PieceType, color: Color, promoted: bool) -> Self {
        if piece_type == PieceType::NONE {
            return Self::EMPTY;
        }

        let mut value = u8::try_from(piece_type.to_index()).expect("piece type index fits in u8");
        if color == Color::WHITE {
            value |= 0x10; // bit 4に先後
        }
        if promoted {
            value |= 0x20; // bit 5に成りフラグ
        }
        Self(value)
    }

    /// Pieceから変換
    #[inline]
    #[must_use]
    pub fn from_piece(piece: Piece) -> Self {
        if piece == Piece::NONE {
            return Self::EMPTY;
        }

        let piece_type = piece.piece_type();
        let color = piece.color();

        // 駒種の値から成り駒かどうかを判定
        let promoted = matches!(
            piece_type,
            PieceType::PRO_PAWN
                | PieceType::PRO_LANCE
                | PieceType::PRO_KNIGHT
                | PieceType::PRO_SILVER
                | PieceType::HORSE
                | PieceType::DRAGON
        );

        // 成り駒の元の駒種を取得
        let base_piece_type = if promoted {
            match piece_type {
                PieceType::PRO_PAWN => PieceType::PAWN,
                PieceType::PRO_LANCE => PieceType::LANCE,
                PieceType::PRO_KNIGHT => PieceType::KNIGHT,
                PieceType::PRO_SILVER => PieceType::SILVER,
                PieceType::HORSE => PieceType::BISHOP,
                PieceType::DRAGON => PieceType::ROOK,
                _ => piece_type,
            }
        } else {
            piece_type
        };
        Self::new(base_piece_type, color, promoted)
    }

    /// Pieceへ変換
    #[inline]
    #[must_use]
    pub fn to_piece(self) -> Piece {
        if self == Self::EMPTY {
            return Piece::NONE;
        }

        let piece_type = self.piece_type();
        let color = self.color();

        if self.is_promoted() {
            // 成り駒の場合
            let promoted_type = match piece_type {
                PieceType::PAWN => PieceType::PRO_PAWN,
                PieceType::LANCE => PieceType::PRO_LANCE,
                PieceType::KNIGHT => PieceType::PRO_KNIGHT,
                PieceType::SILVER => PieceType::PRO_SILVER,
                PieceType::BISHOP => PieceType::HORSE,
                PieceType::ROOK => PieceType::DRAGON,
                _ => piece_type, // 金・玉は成らない
            };
            Piece::from_parts(color, promoted_type)
        } else {
            Piece::from_parts(color, piece_type)
        }
    }

    /// 駒種を取得
    #[inline]
    #[must_use]
    pub fn piece_type(self) -> PieceType {
        let idx = usize::from(self.0 & 0x0F);
        PIECE_TYPE_FROM_INDEX.get(idx).copied().unwrap_or(PieceType::NONE)
    }

    /// 先後を取得
    #[inline]
    #[must_use]
    pub const fn color(self) -> Color {
        if (self.0 & 0x10) != 0 {
            // bit 4をチェック
            Color::WHITE
        } else {
            Color::BLACK
        }
    }

    /// 成り判定
    #[inline]
    #[must_use]
    pub const fn is_promoted(self) -> bool {
        (self.0 & 0x20) != 0
    }

    /// 空マス判定
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// 盤面の81マスを保持する配列
///
/// `Piece` を直接格納することで、`piece_on()` 時の変換オーバーヘッドを排除する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BoardArray([Piece; Square::COUNT]);

impl BoardArray {
    /// 指定した駒で埋めた盤面を作成
    #[must_use]
    pub const fn filled(piece: Piece) -> Self {
        Self([piece; Square::COUNT])
    }

    /// 空の盤面を作成
    #[must_use]
    pub const fn empty() -> Self {
        Self::filled(Piece::NONE)
    }

    /// 指定マスに駒を設定
    #[inline]
    pub fn set(&mut self, sq: Square, piece: Piece) {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        self.0[sq.to_board_index()] = piece;
    }

    /// 指定マスの駒を取得
    #[inline]
    #[must_use]
    pub fn get(&self, sq: Square) -> Piece {
        debug_assert!(sq.is_on_board(), "Invalid square: {sq:?}");
        self.0[sq.to_board_index()]
    }

    /// 指定マスの駒を取得（境界チェックなし）
    ///
    /// # Safety
    /// `sq` が盤面内であることを呼び出し側が保証すること。
    #[inline]
    #[must_use]
    pub unsafe fn get_unchecked(&self, sq: Square) -> Piece {
        unsafe {
            // SAFETY: 呼び出し側が `sq` を盤面内に保つことを保証するため、board index は `0..Square::COUNT` の範囲内に収まる。
            *self.0.get_unchecked(sq.to_board_index())
        }
    }

    /// イテレータを返す
    pub fn iter(&self) -> impl Iterator<Item = (Square, Piece)> + '_ {
        self.0.iter().enumerate().map(|(idx, &piece)| (Square::from_index(idx), piece))
    }
}

impl Default for BoardArray {
    fn default() -> Self {
        Self::empty()
    }
}

/// `MoveDelta32::Board` で使う捕獲駒差分。
///
/// `hand_count_before` は手を適用する前の、`piece` が成り戻しされた持ち駒種の枚数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapturedPieceDelta {
    pub piece: Piece,
    pub hand_count_before: u32,
}

/// `Position` の apply/undo で得られる eval 非依存の盤面差分。
///
/// `undo_move32_with_delta()` が返す値も、元の `do_move` を表す順方向 delta である。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveDelta32 {
    Drop {
        color: Color,
        piece_type: PieceType,
        to: Square,
        hand_count_before: u32,
    },
    Board {
        color: Color,
        from: Square,
        to: Square,
        moved_piece_before: Piece,
        moved_piece_after: Piece,
        captured: Option<CapturedPieceDelta>,
    },
}

impl MoveDelta32 {
    #[must_use]
    pub const fn drop(
        color: Color,
        piece_type: PieceType,
        to: Square,
        hand_count_before: u32,
    ) -> Self {
        Self::Drop { color, piece_type, to, hand_count_before }
    }

    #[must_use]
    pub const fn board(
        color: Color,
        from: Square,
        to: Square,
        moved_piece_before: Piece,
        moved_piece_after: Piece,
        captured: Option<CapturedPieceDelta>,
    ) -> Self {
        Self::Board { color, from, to, moved_piece_before, moved_piece_after, captured }
    }
}

/// `Position::apply_move32...` が hot path 中に把握している指し手適用結果。
///
/// Search / eval が post-move board を再読せずに、search stack site、捕獲情報、
/// post-move key、eval rollback 用 delta を共有するための境界型。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveApplyFacts {
    pub delta: MoveDelta32,
    pub mv: crate::types::Move32,
    pub side_moved: Color,
    pub side_to_move_after: Color,
    pub from: Option<Square>,
    pub to: Square,
    pub moved_piece_before: Piece,
    pub moved_piece_after: Piece,
    pub captured_piece: Piece,
    pub captured_hand_count_before: Option<u32>,
    pub dropped_piece_type: Option<PieceType>,
    pub drop_hand_count_before: Option<u32>,
    pub promoted: bool,
    pub moved_king: bool,
    pub gives_check: bool,
    pub board_key_after: ZobristKey,
    pub hand_key_after: ZobristKey,
    pub key_after: ZobristKey,
    pub partial_keys_after: PartialKeys,
}

impl MoveApplyFacts {
    #[must_use]
    pub fn from_delta(
        delta: MoveDelta32,
        mv: crate::types::Move32,
        gives_check: bool,
        board_key_after: ZobristKey,
        hand_key_after: ZobristKey,
        key_after: ZobristKey,
        partial_keys_after: PartialKeys,
    ) -> Self {
        match delta {
            MoveDelta32::Drop { color, piece_type, to, hand_count_before } => Self {
                delta,
                mv,
                side_moved: color,
                side_to_move_after: color.flip(),
                from: None,
                to,
                moved_piece_before: Piece::NONE,
                moved_piece_after: Piece::from_parts(color, piece_type),
                captured_piece: Piece::NONE,
                captured_hand_count_before: None,
                dropped_piece_type: Some(piece_type),
                drop_hand_count_before: Some(hand_count_before),
                promoted: false,
                moved_king: false,
                gives_check,
                board_key_after,
                hand_key_after,
                key_after,
                partial_keys_after,
            },
            MoveDelta32::Board {
                color,
                from,
                to,
                moved_piece_before,
                moved_piece_after,
                captured,
            } => {
                let captured_piece = captured.map_or(Piece::NONE, |captured| captured.piece);
                Self {
                    delta,
                    mv,
                    side_moved: color,
                    side_to_move_after: color.flip(),
                    from: Some(from),
                    to,
                    moved_piece_before,
                    moved_piece_after,
                    captured_piece,
                    captured_hand_count_before: captured.map(|captured| captured.hand_count_before),
                    dropped_piece_type: None,
                    drop_hand_count_before: None,
                    promoted: moved_piece_before != moved_piece_after,
                    moved_king: moved_piece_before.piece_type() == PieceType::KING,
                    gives_check,
                    board_key_after,
                    hand_key_after,
                    key_after,
                    partial_keys_after,
                }
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn search_stack_site(self) -> MoveStackSiteFacts {
        MoveStackSiteFacts { moved_piece: self.moved_piece_after, to: self.to }
    }

    #[inline]
    #[must_use]
    pub const fn captured_piece(self) -> Piece {
        self.captured_piece
    }

    #[inline]
    #[must_use]
    pub fn is_capture(self) -> bool {
        !self.captured_piece.is_empty()
    }

    #[inline]
    #[must_use]
    pub const fn is_drop(self) -> bool {
        self.from.is_none()
    }
}

/// サーチスタックの継続サイトに必要な指し手適用後の情報。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveStackSiteFacts {
    pub moved_piece: Piece,
    pub to: Square,
}

/// エラー型定義
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    NoStateInfo,
    StackUnderflow,
    /// 探索で allocation なしに利用できる state slot がない。
    StateCapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    NoKing(Color),
    TwoKings(Color),
    DoublePawn(File, Color),
    InvalidHandCount { piece: HandPiece, count: u32 },
    InvalidPlacement(Square, PieceType),
}

/// 盤面検証で検出される個別の問題
///
/// [`ValidationError`] が first-error 契約であるのに対し、
/// `ValidationIssue` は [`ValidationReport`] と組み合わせて
/// すべての問題を一度に収集するために使用する。
///
/// エディタや局面修正ツール向けの full-report API。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationIssue {
    /// 玉が盤上にない（駒落ち・詰将棋では正常な場合がある）
    NoKing(Color),
    /// 玉が2枚以上ある
    TwoKings(Color),
    /// 同筋に同色の歩が複数ある（二歩）
    DoublePawn(File, Color),
    /// 持ち駒数が上限を超えている
    InvalidHandCount { piece: HandPiece, count: u32 },
    /// 行き場のない駒の配置（歩・香・桂の段制限違反）
    InvalidPlacement(Square, PieceType),
}

impl fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoKing(color) => write!(f, "{color:?} king is missing"),
            Self::TwoKings(color) => write!(f, "{color:?} has two or more kings"),
            Self::DoublePawn(file, color) => {
                write!(f, "{color:?} has double pawn on file {file:?}")
            }
            Self::InvalidHandCount { piece, count } => {
                write!(f, "invalid hand count for {piece:?}: {count}")
            }
            Self::InvalidPlacement(sq, pt) => {
                write!(f, "invalid placement of {pt:?} on {sq:?}")
            }
        }
    }
}

/// 盤面検証の全件レポート
///
/// `Position::validate_all()`（`validation` feature）が返す構造体。
/// 検出されたすべての [`ValidationIssue`] を保持する。
///
/// # Examples
///
/// `validate_all()` は `validation` feature が必要である。
///
/// ```
/// # #[cfg(feature = "validation")] {
/// use rsshogi::board::{self, position_from_sfen};
///
/// board::init();
/// let pos = position_from_sfen(
///     "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
/// ).unwrap();
/// let report = pos.validate_all();
/// assert!(report.is_valid());
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationReport {
    issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    /// 問題がなければ `true`
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.issues.is_empty()
    }

    /// 検出された問題のスライス
    #[must_use]
    pub fn issues(&self) -> &[ValidationIssue] {
        &self.issues
    }

    /// 問題リストを消費して `Vec` で返す
    #[must_use]
    pub fn into_issues(self) -> Vec<ValidationIssue> {
        self.issues
    }

    // `ValidationReport` の唯一の constructor は validation impl（`validate_all`）専用。
    // validation feature off では caller が無いため gate して dead_code を避ける。
    #[cfg(feature = "validation")]
    pub(crate) fn new(issues: Vec<ValidationIssue>) -> Self {
        Self { issues }
    }
}

/// 入玉宣言の条件評価結果
///
/// `Position::evaluate_declaration()` が返す構造体。
/// 宣言勝ちが可能かどうかに加え、各条件の詳細を構造化して提供する。
///
/// # Examples
///
/// ```
/// use rsshogi::board::{self, position_from_sfen, Position};
/// use rsshogi::types::EnteringKingRule;
///
/// board::init();
/// let mut pos = position_from_sfen(
///     "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"
/// ).unwrap();
/// pos.set_entering_king_rule(EnteringKingRule::Point27);
/// let eval = pos.evaluate_declaration();
/// assert!(!eval.can_declare);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationEvaluation {
    /// 適用されたルール
    pub rule: EnteringKingRule,
    /// 宣言勝ちが可能か
    pub can_declare: bool,
    /// ルール別の詳細
    pub detail: DeclarationDetail,
}

/// 入玉宣言の条件詳細
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeclarationDetail {
    /// ルール未設定（`None` / `Unset`）
    NoRule,
    /// トライルール
    TryRule {
        /// 玉の現在位置（盤上にない場合は `None`）
        king_square: Option<Square>,
        /// トライ先のマス
        try_square: Square,
        /// トライ先に隣接しているか
        can_reach: bool,
        /// トライ先を占有できるか（自駒で塞がれていないか）
        ///
        /// 相手駒がいる場合は捕獲して進めるため `true` になりうる。
        can_occupy: bool,
        /// トライ先が相手の利きに晒されていないか
        is_safe: bool,
    },
    /// 点数ルール（Point24 / Point27 系）
    PointRule {
        /// 王手されていないか
        not_in_check: bool,
        /// 玉が敵陣三段目以内にあるか
        king_in_enemy_camp: bool,
        /// 敵陣にある自駒数（玉含む）
        pieces_in_camp: i32,
        /// 敵陣に10枚以上あるか（条件4）
        enough_pieces: bool,
        /// 現在の持点
        points: i32,
        /// 宣言に必要な点数
        required_points: i32,
    },
}
