# Test Data for P3 Board State Tests

このディレクトリにはP3フェーズ（盤面状態管理）のテストデータが含まれています。

## ファイル構成

### sfen_positions.txt
SFEN形式の局面データ。以下のカテゴリを含む：

- **基本局面**: 初期局面、手番違い、1手進んだ局面
- **benchmark 局面**: 序盤、中盤、終盤の代表的な局面
- **特殊局面**: 詰将棋、最多合法手局面、pin/checker検出テスト用
- **Zobristテスト用**: 微妙に異なる2局面（ハッシュ値の違いを検証）
- **不正局面**: 駒数超過など（ネガティブテスト用）

### perft_results.txt
参照実装で検証済みの perft 結果。以下を含む：

- **初期局面**: depth 1-6 の正解ノード数
- **複雑な中盤局面**: depth 3-4 の正解ノード数

### move_sequences.txt
手順データ。以下を含む：

- **千日手テスト**: 4回同一局面に到達する手順
- **二歩テスト**: 二歩の違法手を含む手順
- **打ち歩詰めテスト**: 打ち歩詰めの違法手を含む手順

## 使用方法

### SFEN round-trip テスト (Task 3.3)
```rust
use std::fs::read_to_string;

let data = read_to_string("crates/rsshogi/tests/test_data/sfen_positions.txt")?;
for line in data.lines() {
    if line.starts_with('#') || line.trim().is_empty() { continue; }
    let parts: Vec<&str> = line.split('|').collect();
    let name = parts[0].trim();
    let sfen = parts[1].trim();
    
    // Test: parse -> to_sfen -> parse again
    let pos1 = crate::board::position_from_sfen(sfen)?;
    let sfen2 = pos1.to_sfen(None);
    let pos2 = crate::board::position_from_sfen(&sfen2)?;
    assert_eq!(pos1, pos2, "Round-trip failed for {}", name);
}
```

### Perft テスト (Task 8.2)
```rust
use rsshogi::board::{perft, position::Position};

let pos = crate::board::hirate_position();
let perft_result = perft::perft(&pos, 4).expect("reference perft available");
assert_eq!(perft_result.nodes, 719_731);
```

### Property テスト (Task 8.1)
```rust
// 千日手テスト
let seq = get_sequence("repetition_4fold"); // move_sequences.txtから取得
let mut pos = crate::board::hirate_position();

for mv_str in seq.split_whitespace().skip(3) { // "position startpos moves"を飛ばす
    let mv = Move::from_usi(mv_str)?;
    pos.apply_move32(mv);
}

// 4回目の同一局面でrepetition検出（repetition_counter >= 3）
assert!(pos.is_repetition(3));
```

## データソース

- 参照実装: perft 正解値、benchmark 局面
- haitaka: 詰将棋、特殊ケーステスト局面  
- 手作り: 千日手、二歩、打ち歩詰めの手順

## 注意事項

- SFEN文字列内の数字は空マス数を示す（例: `02` = 2マス空き）
- `+`は成駒を示す（例: `+P` = と金、`+B` = 馬）
- 持ち駒は`b`/`w`の後に続く（例: `BGN` = 先手が角金桂を持っている）
- 手数は1から始まる（互換）
