# SFEN パーサ

> **前提知識**: [Position 構造体](../position/index.md)（局面の内部表現）

## このページの要点

- SFEN はチェスの FEN を将棋向けに拡張した**局面記述フォーマット**で、USI プロトコルの標準
- 持ち駒の記述順序に正規化ルール（R B G S N L P 順）があるが、パーサは**任意の順序を受け入れる**べき
- 生成時は常に正規化、パース時は寛容に。これが相互運用性の鉄則
- 局面の合法性検証（玉の存在、二歩、相手玉への王手なし等）はパース後に実施する

## なぜ SFEN が標準か

2000 年代初頭、チェスエンジンの通信プロトコル UCI（Universal Chess Interface）が広く普及していました。
USI（Universal Shogi Interface）はこれを将棋向けに適応したもので、SFEN は FEN の将棋版として自然に採用されました。

SFEN が標準となった理由：
- **ASCII のみ**で表現でき、プロトコル通信に適している
- **1 行で局面を完全記述**でき、テストケースやログに便利
- **パース・生成が容易**で、実装コストが低い
- エンジン・GUI 間の**共通言語**として機能する

SFEN（Shogi Forsyth-Edwards Notation）は、将棋の局面をテキストで表現するフォーマットです。
チェスのFEN（Forsyth-Edwards Notation）を将棋向けに拡張したもので、USI プロトコルで局面を表現する際に使用されます。

## SFEN フォーマットの概要

### 基本構造

SFEN 文字列は、以下の4つのフィールドをスペースで区切って表現します：

```text
盤面 手番 持ち駒 手数
```

例：平手初期局面

```text
lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

### 各フィールドの詳細

#### 1. 盤面（Board）

盤面は1段目から9段目へ、各段を `/` で区切って表現します。

駒の表記：

| 駒 | 表記 | 成駒 | 表記 |
|----|------|------|------|
| 歩 | `p` | と | `+p` |
| 香 | `l` | 成香 | `+l` |
| 桂 | `n` | 成桂 | `+n` |
| 銀 | `s` | 成銀 | `+s` |
| 金 | `g` | - | - |
| 角 | `b` | 馬 | `+b` |
| 飛 | `r` | 龍 | `+r` |
| 玉 | `k` | - | - |

- **小文字**: 後手の駒
- **大文字**: 先手の駒
- **数字**: 連続する空マスの数

例：

```text
1段目: lnsgkgsnl → 後手の香桂銀金玉金銀桂香
2段目: 1r5b1 → 空・飛・空5つ・角・空
3段目: ppppppppp → 歩9枚
```

#### 2. 手番（Side to Move）

- `b`: 先手（Black）の手番
- `w`: 後手（White）の手番

#### 3. 持ち駒（Hand）

持ち駒を表記します。
持ち駒がない場合は `-` を使用します。

表記ルール：

- 駒の種類を表す文字（大文字=先手、小文字=後手）
- 枚数が2枚以上の場合、数字を前置

例：

```text
-           # 持ち駒なし
P           # 先手の歩1枚
2P          # 先手の歩2枚
RBG3P       # 先手の飛・角・金・歩3枚
2r3p       # 後手の飛2枚・歩3枚
RBG3Prb4p  # 先手が飛・角・金・歩3枚、後手が飛・角・歩4枚
```

#### 4. 手数（Ply Count）

現在の手数を表します。
初期局面は `1` です。

## SFEN の一意性問題

### 問題点：持ち駒の順序

SFEN 文字列には、**一意性の問題**があります。
同じ局面でも、持ち駒の順序によって異なる SFEN 文字列になる可能性があります。

例：

```text
# これらは同じ局面だが、異なる SFEN 文字列
position sfen ... b RBG3P 1
position sfen ... b 3PRBG 1
position sfen ... b GBR3P 1
```

### 正規化ルール

この問題を解決するため、USI プロトコルの原案では以下の正規化ルールが定義されていました：

1. **駒種の順序**: `飛 角 金 銀 桂 香 歩`（R B G S N L P）
2. **先手・後手の順序**: すべての先手の駒を後手の駒より先に配置

正規化された表記例：

# 正規化された表記
RBG3Prb4p

```text
# 正規化されていない表記（非推奨）
3PRBGrb4p
pppRBGrbpppp
```

### 実装における推奨事項

SFEN を生成する際は、**常に正規化された順序**で出力すべきです：

```rust,ignore
const HAND_ORDER: [PieceType; 7] = [
    PieceType::ROOK,
    PieceType::BISHOP,
    PieceType::GOLD,
    PieceType::SILVER,
    PieceType::KNIGHT,
    PieceType::LANCE,
    PieceType::PAWN,
];

impl Hand {
    pub fn to_sfen(&self, color: Color) -> String {
        let mut result = String::new();

        for &piece_type in &HAND_ORDER {
            let count = self.count(piece_type);
            if count > 0 {
                // 2枚以上なら枚数を前置
                if count > 1 {
                    result.push_str(&count.to_string());
                }

                // 駒種を追加
                let ch = piece_type.to_usi_char();
                if color == Color::BLACK {
                    result.push(ch.to_ascii_uppercase());
                } else {
                    result.push(ch.to_ascii_lowercase());
                }
            }
        }

        result
    }
}
```

### パーサでの対応

SFEN をパースする際は、**どのような順序でも受け入れる**べきです：

```rust,ignore
impl Hand {
    pub fn from_sfen(s: &str) -> Result<(Hand, Hand), SfenError> {
        let mut black_hand = Hand::new();
        let mut white_hand = Hand::new();

        if s == "-" {
            return Ok((black_hand, white_hand));
        }

        let mut chars = s.chars().peekable();
        let mut count = 1;

        while let Some(ch) = chars.next() {
            if ch.is_ascii_digit() {
                // 枚数の読み取り
                let mut num_str = String::new();
                num_str.push(ch);

                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_digit() {
                        num_str.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                count = num_str.parse().unwrap_or(1);
            } else {
                // 駒種の読み取り
                let piece_type = PieceType::from_usi_char(ch.to_ascii_lowercase())?;
                let color = if ch.is_ascii_uppercase() {
                    Color::BLACK
                } else {
                    Color::WHITE
                };

                // 持ち駒に追加
                let hand = if color == Color::BLACK {
                    &mut black_hand
                } else {
                    &mut white_hand
                };

                for _ in 0..count {
                    hand.add(piece_type);
                }

                count = 1;  // リセット
            }
        }

        Ok((black_hand, white_hand))
    }
}
```

## SFEN パーサの実装

> **コード例について**: 以降の `Position::from_sfen` / `Hand::to_sfen` / `SfenError` などの実装は、
> パース・生成アルゴリズムを説明するための**参考実装**です。型名・メソッド名・エラー列挙子は説明用に簡略化しており、
> rsshogi の実際の実装とは異なります。実際の公開 API は次のとおりです。
>
> - `Position::from_sfen(sfen: &str) -> Result<Position, SfenError>`（局面を構築）
> - `Position::to_sfen(game_ply: Option<i32>) -> String`（SFEN を生成。`None` で現在手数、負数で手数を省略）
> - 構造化したい場合は `rsshogi::board::parse_sfen(sfen) -> Result<PositionState, SfenError>`
> - 持ち駒の解析・生成は内部関数（`board::parser` の `parse_hands` / `generate_hands_from_data`）が担い、`Hand` 自体に `to_sfen` / `from_sfen` はありません
> - `SfenError` の実バリアントは `MissingField` / `InvalidPiece` / `InvalidSquare` / `InvalidHandCount` / `InvalidTurn` / `InvalidPly` / `TrailingToken` / `InvalidMove` です

### 完全なパーサ実装例

```rust,ignore
impl Position {
    /// SFEN 文字列から局面を構築
    pub fn from_sfen(sfen: &str) -> Result<Self, SfenError> {
        let parts: Vec<&str> = sfen.split_whitespace().collect();

        if parts.len() < 3 {
            return Err(SfenError::InvalidFormat);
        }

        // 1. 盤面のパース
        let board = parse_board(parts[0])?;

        // 2. 手番のパース
        let side_to_move = match parts[1] {
            "b" => Color::BLACK,
            "w" => Color::WHITE,
            _ => return Err(SfenError::InvalidSideToMove),
        };

        // 3. 持ち駒のパース
        let (black_hand, white_hand) = Hand::from_sfen(parts[2])?;

        // 4. 手数のパース（省略可能）
        let ply = if parts.len() >= 4 {
            parts[3].parse().unwrap_or(1)
        } else {
            1
        };

        // Position を構築
        let mut pos = Position::empty();
        pos.set_board(board);
        pos.set_side_to_move(side_to_move);
        pos.set_hand(Color::BLACK, black_hand);
        pos.set_hand(Color::WHITE, white_hand);
        pos.set_ply(ply);

        // ビットボードとハッシュ値を初期化
        pos.rebuild_bitboards();
        pos.compute_hash();

        Ok(pos)
    }
}

fn parse_board(board_str: &str) -> Result<[Piece; 81], SfenError> {
    let mut board = [Piece::NO_PIECE; 81];
    let ranks: Vec<&str> = board_str.split('/').collect();

    if ranks.len() != 9 {
        return Err(SfenError::InvalidBoardFormat);
    }

    for (rank_idx, rank_str) in ranks.iter().enumerate() {
        let mut file_idx = 8;  // 9筋から1筋へ
        let mut chars = rank_str.chars();

        while let Some(ch) = chars.next() {
            if ch.is_ascii_digit() {
                // 空マスの数
                let empty_count = ch.to_digit(10).unwrap() as usize;
                file_idx = file_idx.saturating_sub(empty_count);
            } else if ch == '+' {
                // 成駒
                if let Some(next_ch) = chars.next() {
                    let piece_type = PieceType::from_usi_char(next_ch.to_ascii_lowercase())?;
                    let color = if next_ch.is_ascii_uppercase() {
                        Color::BLACK
                    } else {
                        Color::WHITE
                    };

                    let piece = Piece::from_parts(color, piece_type).promote();
                    let sq = Square::new(file_idx, rank_idx);
                    board[sq.index()] = piece;

                    file_idx = file_idx.saturating_sub(1);
                }
            } else {
                // 通常の駒
                let piece_type = PieceType::from_usi_char(ch.to_ascii_lowercase())?;
                let color = if ch.is_ascii_uppercase() {
                    Color::BLACK
                } else {
                    Color::WHITE
                };

                let piece = Piece::from_parts(color, piece_type);
                let sq = Square::new(file_idx, rank_idx);
                board[sq.index()] = piece;

                file_idx = file_idx.saturating_sub(1);
            }
        }

        if file_idx != usize::MAX {
            return Err(SfenError::InvalidRankLength);
        }
    }

    Ok(board)
}
```

### エラー型の定義

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SfenError {
    InvalidFormat,
    InvalidBoardFormat,
    InvalidRankLength,
    InvalidSideToMove,
    InvalidPieceChar,
    InvalidHandFormat,
}

impl std::fmt::Display for SfenError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            SfenError::InvalidFormat => write!(f, "Invalid SFEN format"),
            SfenError::InvalidBoardFormat => write!(f, "Invalid board format"),
            SfenError::InvalidRankLength => write!(f, "Invalid rank length"),
            SfenError::InvalidSideToMove => write!(f, "Invalid side to move"),
            SfenError::InvalidPieceChar => write!(f, "Invalid piece character"),
            SfenError::InvalidHandFormat => write!(f, "Invalid hand format"),
        }
    }
}

impl std::error::Error for SfenError {}
```

## SFEN 生成

### Position から SFEN を生成

```rust,ignore
impl Position {
    /// 局面を SFEN 文字列に変換
    pub fn to_sfen(&self) -> String {
        let mut sfen = String::new();

        // 1. 盤面
        for rank in 0..9 {
            let mut empty_count = 0;

            for file in (0..9).rev() {
                let sq = Square::new(file, rank);
                let piece = self.piece_at(sq);

                if piece == Piece::NO_PIECE {
                    empty_count += 1;
                } else {
                    // 空マスがあれば出力
                    if empty_count > 0 {
                        sfen.push_str(&empty_count.to_string());
                        empty_count = 0;
                    }

                    // 駒を出力
                    sfen.push_str(&piece.to_sfen());
                }
            }

            // 行末の空マス
            if empty_count > 0 {
                sfen.push_str(&empty_count.to_string());
            }

            // 段の区切り
            if rank < 8 {
                sfen.push('/');
            }
        }

        // 2. 手番
        sfen.push(' ');
        sfen.push(if self.side_to_move == Color::BLACK { 'b' } else { 'w' });

        // 3. 持ち駒
        sfen.push(' ');
        let black_hand = self.hand(Color::BLACK).to_sfen(Color::BLACK);
        let white_hand = self.hand(Color::WHITE).to_sfen(Color::WHITE);

        if black_hand.is_empty() && white_hand.is_empty() {
            sfen.push('-');
        } else {
            sfen.push_str(&black_hand);
            sfen.push_str(&white_hand);
        }

        // 4. 手数
        sfen.push(' ');
        sfen.push_str(&self.ply().to_string());

        sfen
    }
}

impl Piece {
    fn to_sfen(&self) -> String {
        let mut result = String::new();

        // 成駒なら + を前置
        if self.is_promoted() {
            result.push('+');
        }

        // 駒種
        let ch = self.piece_type().to_usi_char();

        // 先手なら大文字、後手なら小文字
        if self.color() == Color::BLACK {
            result.push(ch.to_ascii_uppercase());
        } else {
            result.push(ch.to_ascii_lowercase());
        }

        result
    }
}
```

## 特殊な局面

### startpos（平手初期局面）

USI プロトコルでは、平手初期局面を `startpos` で表現できます：

```text
position startpos
```

これは以下の SFEN と等価です：

```text
position sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

実装例：

```rust,ignore
impl Position {
    pub const STARTPOS_SFEN: &'static str =
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

    pub fn startpos() -> Self {
        Self::from_sfen(Self::STARTPOS_SFEN).unwrap()
    }
}
```

### 不正な局面の検出

SFEN パーサは、以下のような不正な局面を検出すべきです：

```rust,ignore
impl Position {
    /// 局面が合法かを検証
    pub fn is_valid(&self) -> bool {
        // 1. 玉が両者とも存在するか
        if self.pieces_cp(Color::BLACK, PieceType::KING).count() != 1 {
            return false;
        }
        if self.pieces_cp(Color::WHITE, PieceType::KING).count() != 1 {
            return false;
        }

        // 2. 駒の総数が適切か
        let total_pieces = self.count_all_pieces();
        if total_pieces > 40 {
            return false;
        }

        // 3. 歩の数が適切か（各筋1枚まで）
        if !self.validate_pawn_placement() {
            return false;
        }

        // 4. 相手玉が王手されていないか
        let opponent = self.side_to_move.opponent();
        if self.is_in_check(opponent) {
            return false;
        }

        // 5. 持ち駒に玉や成駒がないか
        if self.hand(Color::BLACK).has_king_or_promoted() {
            return false;
        }
        if self.hand(Color::WHITE).has_king_or_promoted() {
            return false;
        }

        true
    }

    fn validate_pawn_placement(&self) -> bool {
        for file in 0..9 {
            let file_bb = Bitboard::file_bb(file);

            // 先手の歩
            let black_pawns = self.pieces_cp(Color::BLACK, PieceType::PAWN) & file_bb;
            if black_pawns.count() > 1 {
                return false;
            }

            // 後手の歩
            let white_pawns = self.pieces_cp(Color::WHITE, PieceType::PAWN) & file_bb;
            if white_pawns.count() > 1 {
                return false;
            }
        }

        true
    }
}
```

## テストケース

### 正常系のテスト

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_startpos_roundtrip() {
        let pos = Position::startpos();
        let sfen = pos.to_sfen();
        let restored = Position::from_sfen(&sfen).unwrap();

        assert_eq!(pos, restored);
    }

    #[test]
    fn test_with_hand() {
        let sfen = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b RBG3P 10";
        let pos = Position::from_sfen(sfen).unwrap();

        assert_eq!(pos.side_to_move(), Color::BLACK);
        assert_eq!(pos.hand(Color::BLACK).count(PieceType::ROOK), 1);
        assert_eq!(pos.hand(Color::BLACK).count(PieceType::BISHOP), 1);
        assert_eq!(pos.hand(Color::BLACK).count(PieceType::GOLD), 1);
        assert_eq!(pos.hand(Color::BLACK).count(PieceType::PAWN), 3);
        assert_eq!(pos.ply(), 10);
    }

    #[test]
    fn test_promoted_pieces() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/4+P4/PPPP1PPPP/1B5R1/LNSGKGSNL w - 5";
        let pos = Position::from_sfen(sfen).unwrap();

        let promoted_sq = Square::new(4, 5);
        let piece = pos.piece_at(promoted_sq);

        assert!(piece.is_promoted());
        assert_eq!(piece.piece_type(), PieceType::PAWN);
    }
}
```

### 異常系のテスト

```rust
#[test]
fn test_invalid_sfen() {
    // 不正なフォーマット
    assert!(Position::from_sfen("invalid").is_err());

    // 段数が不正
    assert!(Position::from_sfen("lnsgkgsnl/1r5b1/ppppppppp b - 1").is_err());

    // 手番が不正
    assert!(Position::from_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL x - 1").is_err());
}

#[test]
fn test_hand_order_normalization() {
    // 異なる順序の持ち駒を受け入れる
    let sfen1 = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b RBG3P 1";
    let sfen2 = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b 3PRBG 1";
    let sfen3 = "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b GBR3P 1";

    let pos1 = Position::from_sfen(sfen1).unwrap();
    let pos2 = Position::from_sfen(sfen2).unwrap();
    let pos3 = Position::from_sfen(sfen3).unwrap();

    // すべて同じ局面
    assert_eq!(pos1, pos2);
    assert_eq!(pos2, pos3);

    // 出力は正規化される
    let output = pos1.to_sfen();
    assert!(output.contains("RBG3P"));
}
```

## 実装のベストプラクティス

### 1. 正規化された SFEN を出力

SFEN を生成する際は、常に正規化された形式（持ち駒を R B G S N L P の順序）で出力します。

### 2. 柔軟なパーサ

SFEN をパースする際は、どのような順序の持ち駒も受け入れます。

### 3. バリデーション

パース後、必ず局面の合法性を検証します。

### 4. エラーハンドリング

不正な SFEN に対して、適切なエラーメッセージを返します。

### 5. テスト

正常系・異常系の両方で十分なテストケースを用意します。

## 落とし穴

### SFEN の一意性問題

同じ局面でも、持ち駒の記述順序が異なると別の文字列になります。
文字列比較で局面の同一性を判定してはいけません。
比較にはパース後の `Position` 構造体か Zobrist ハッシュを使用してください。

### 手数フィールドの省略

USI の `position` コマンドでは手数フィールドが省略されることがあります。
パーサは手数がない場合にデフォルト値 `1` を使うべきです。

## まとめ

- **正規化**: 生成時は持ち駒の順序を R B G S N L P に統一
- **柔軟性**: パーサはどのような順序も受け入れる
- **バリデーション**: パース後に局面の合法性を検証
- **文字列比較に依存しない**: 局面の同一性判定には構造体かハッシュを使う

## 次に読む

→ **[棋譜・局面フォーマット](./formats.md)**: SFEN 以外のフォーマット（KIF, CSA, JKF）との比較と使い分けを解説します。

## 参考資料

- [SFEN文字列は一意に定まらない件 - やねうら王公式サイト](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%89%e3%81%aa%e3%81%84%e4%bb%b6/) - 持ち駒順序の問題
- [SFEN文字列は本来は一意に定まる件 - やねうら王公式サイト](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e6%9c%ac%e6%9d%a5%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%8b%e4%bb%b6/) - 正規化ルール
- [USI プロトコル仕様](https://shogidokoro2.stars.ne.jp/usi.html) - SFEN の定義
- [FEN - Chess Programming Wiki](https://www.chessprogramming.org/Forsyth-Edwards_Notation) - 元となったチェスのFEN
