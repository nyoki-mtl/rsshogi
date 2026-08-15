# 1.2.0 への移行

## 要点

1.1.1 から 1.2.0 への更新では、HCP、PackedSfen、PACK、HCPE、YBB、SBK、Zobrist key を再生成する必要はありません。
主な移行作業は、削除された Rust API の置換と、生成手の順序へ依存していないことの確認です。

Python の公開 module 構成と主要 API は維持されています。
通常版と AVX2 版の package version はどちらも 1.2.0 です。

## Rust dependency

```toml
[dependencies]
rsshogi = { version = "1.2.0", features = ["records", "book"] }
```

core の既定 feature は空です。
1.1.1 で利用していた optional feature は 1.2.0 でも明示してください。
MSRV は Rust 1.95 のままです。

## 削除された API

### `board::Bitboard256`

公開 `Bitboard256` は削除されました。
盤上の升集合には `types::Bitboard` または `board::Bitboard`、遠方駒の利きには `rook_attacks`、`bishop_attacks`、`lance_attacks` などの公開関数を使います。

`Bitboard256` の SIMD lane 配置や packed operation を直接使っていた code に drop-in replacement はありません。
必要な盤上演算を `Bitboard` の集合演算へ書き直してください。

### Qugiy 固有の公開定数

`QUGIY_STEP_ATTACKS`、`QUGIY_ROOK_MASK`、`QUGIY_BISHOP_MASK` は削除されました。
利きの取得には公開 attack 関数と `KING_ATTACKS`、`GOLD_ATTACKS`、`SILVER_ATTACKS`、`KNIGHT_ATTACKS`、`PAWN_ATTACKS` を使います。
内部 mask の index や table layout は互換契約ではありません。

### peta_shock-compatible solver

`solve_peta_shock_book`、`PetaShockOptions`、`YaneuraOuDb2016WriteOptions::peta_shock()` は削除されました。
DB2016 の lossless reader / writer、`BookDatabase`、YBB、SBK は残っています。

既存の score 付き DB2016 を読み書きするだけなら、`YaneuraOuBook` と `BookDatabase::write_yaneuraou_db2016` を使います。
solver 相当の graph backup が必要な application は、自身の探索・評価方針を持つ別 layer で実装してください。

## move generation

生成順は 1.2.0 から明示的に未規定です。
次のように順序を比較している code は、集合比較または明示 sort へ変更します。

```rust,ignore
use rsshogi::board::{MoveList, hirate_position};
use rsshogi::movegen::{LegalAll, generate_moves};

let position = hirate_position();
let mut moves = MoveList::new();
generate_moves::<LegalAll>(&position, &mut moves);

let mut raw_moves: Vec<_> = moves.iter().map(|mv| mv.raw()).collect();
raw_moves.sort_unstable();
```

GUI の入力照合、棋譜検証、perft には完全な `LegalAll` を使います。
`Legal` は探索向けに一部の任意不成を省略します。
`Captures`、`Quiets`、`Checks`、`QuietChecks`、`Evasions` などは mode 固有の sub-generator であり、`LegalAll` の代わりではありません。

`QuietChecks` は駒打ちの静かな王手を含みます。
`*All` mode は歩、香、大駒の任意不成を含みます。
王手中に非回避 mode を呼んでも自動的に回避手だけへ絞られないため、探索側の呼び出し条件を維持してください。

## raw move と PACK

`Move` と `AperyMove` の raw layout は異なります。
1.2.0 で layout 自体は変わっていませんが、型をまたぐ code は明示変換へ統一してください。

```rust,ignore
use rsshogi::types::Move;

let drop = Move::from_usi("B*4e").expect("valid drop");
let apery = drop.to_apery();
assert_eq!(apery.to_move(), drop);
```

PACK は `AperyMove` layout を使います。
1.1.1 の PACK はそのまま読め、1.2.0 が書いた駒打ち・成りを含む PACK も 1.1.1 と同じ byte contract です。

## 基本値の境界

1.2.0 の API を組み込む際は、次の境界を前提にしてください。

- `Move::from_usi("0000")` と `Move32::from_usi("0000")` は null move を返す。正規出力は `null`。
- `HandPiece::from_piece_type` は未成の持ち駒だけを受理し、成駒を demote しない。
- `Hand::add` / `Hand::sub` は overflow / underflow で panic する。外部入力には `checked_add` / `checked_sub` を使う。
- `Eval` の比較は `Cp` / `Special` の variant 順ではなく signed raw 値の数値順。
- `Bitboard::is_aligned` は玉から同じ向きの ray を判定し、玉を挟んだ二升には `false` を返す。

## DB2016

reader は path-backed で逐次処理します。
大規模 book を開いた後に binary lookup を使う場合は、完全な整列検証を実行します。

```rust,ignore
use rsshogi::book::YaneuraOuBook;

let mut book = YaneuraOuBook::open("book.db")?;
book.validate_full()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

整列を保証できない book は `lookup_sfen_by_scan` を使います。
`iter_entries()` では各 item の `Result` を処理してください。
不正な move 行を含む group が `Err` でも、iterator は次の position group へ進めます。

writer は、元の DB2016 で省略された ply と `count` を省略したまま出力します。
固定 ply や ply の完全省略が必要なら `with_fixed_ply` / `with_omitted_ply` を指定します。

## Python package の切り替え

portable build を更新する場合は次のようにします。

```console
python -m pip uninstall -y rsshogi-avx2
python -m pip install --upgrade "rsshogi==1.2.0"
```

AVX2 build へ切り替える場合は逆に通常版を削除してからインストールします。

```console
python -m pip uninstall -y rsshogi
python -m pip install "rsshogi-avx2==1.2.0"
```

同じ environment に二つの distribution を残さないでください。

## 更新後の確認

application 側で次を確認します。

1. Rust と Python の package version が 1.2.0 である。
2. 必要な Cargo feature が明示されている。
3. 削除された `Bitboard256`、Qugiy 定数、peta_shock API の参照がない。
4. 合法手を順序ではなく集合として扱っている。
5. `Legal` と `LegalAll`、pseudo-legal sub-generator の用途を区別している。
6. 既存の HCP、PackedSfen、PACK、HCPE、YBB、SBK を代表 sample で読み込める。
7. PACK の sample に駒打ち、成り、通常の centipawn 範囲外の `Eval::Special` が含まれる。
8. 大規模 DB2016 の lookup 前に整列検証または scan mode を選んでいる。
