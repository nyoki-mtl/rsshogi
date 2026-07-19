# 将棋の棋譜・局面フォーマット

> **前提知識**: [SFEN パーサ](./index.md)（SFEN フォーマットの詳細）

## このページの要点

- 将棋には **6 つの主要フォーマット**（SFEN, KIF, KI2, CSA, JKF, SBINPACK）があり、用途で使い分ける
- エンジン開発には **SFEN + CSA**、人間向けには **KIF**、Web には **JKF** が適する
- フォーマット間の変換では**情報の損失**が起きうる（特に KIF → SFEN でメタデータが失われる）
- KIF/KI2 の**エンコーディング問題**（Shift_JIS vs UTF-8）は実装上の大きな落とし穴

将棋の棋譜や局面を表現するための様々なフォーマットが存在します。
それぞれ異なる目的や特徴を持ち、用途に応じて使い分けられています。

## フォーマット一覧

### SFEN（Shogi Forsyth-Edwards Notation）

**用途**: 局面の簡潔な表現、USIプロトコル

**特徴**:
- 1行で局面を完全に表現
- チェスのFENを将棋向けに拡張
- USIプロトコルの標準フォーマット
- 人間が読めるが、主に機械向け

```text
**例**:
lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1
```

**長所**:
- 非常にコンパクト
- パース・生成が容易
- 国際的に通用（ASCII）

**短所**:
- 棋譜全体を表現できない（局面のみ）
- メタデータ（対局者名、日時など）を含められない

**詳細**: [SFEN パーサ](./index.md)

### KIF形式（.kif / .kifu）

**用途**: 人間が読む棋譜、棋譜管理ソフト

**特徴**:
- 柿木義一氏が開発
- 人間の可読性を最優先
- 全角文字を使用（日本語）
- 豊富なメタデータ
- 分岐や栞（ブックマーク）をサポート

```text
**例**:
# ---- Kifu for Windows V7 棋譜ファイル ----
開始日時：2024/01/15
表題：第73期ALSOK杯王将戦七番勝負 第1局
棋戦：王将戦
先手：藤井聡太王将
後手：菅井竜也八段
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)   ( 0:01/00:00:01)
   2 ３四歩(33)   ( 0:01/00:00:01)
   3 ２六歩(27)   ( 0:02/00:00:03)
   4 ４四歩(43)   ( 0:01/00:00:02)
   5 投了
まで4手で先手の勝ち
```

**長所**:
- 極めて人間に優しい
- 対局情報が豊富
- 棋譜ソフトで広く対応
- 分岐・コメント・栞機能

**短所**:
- ファイルサイズが大きい
- パース処理が複雑
- エンコーディングの問題（Shift_JIS / UTF-8）
- 厳密な仕様書が存在しない

**エンコーディング注意**:
- 古い棋譜: Shift_JIS
- 新しい棋譜: UTF-8
- 自動判定が必要な場合あり

### KI2形式（.ki2）

**用途**: より簡潔な人間向け棋譜

**特徴**:
- KIF形式の簡略版
- 指し手を自然言語風に表記
- 移動元の座標を省略

```text
**例**:
# ---- Kifu for Windows V7 棋譜ファイル ----
開始日時：2024/01/15
手合割：平手
▲７六歩 △３四歩 ▲２六歩 △４四歩
```

**長所**:
- KIF より読みやすい
- ファイルサイズが小さい

**短所**:
- 曖昧性がある（同じ駒種が複数ある場合）
- 正確な復元には推論が必要

### CSA形式（.csa）

**用途**: コンピュータ将棋、AI開発

**特徴**:
- コンピュータ将棋協会が策定
- ASCII互換の記号を使用
- 厳密な仕様（バージョン2.2 / 3.0）
- 機械処理を重視

```text
**例**:
V2.2
N+藤井聡太
N-菅井竜也
$EVENT:王将戦
$START_TIME:2024/01/15
$TIME_LIMIT:08:00+00
P1-KY-KE-GI-KI-OU-KI-GI-KE-KY
P2 * -HI *  *  *  *  * -KA * 
P3-FU-FU-FU-FU-FU-FU-FU-FU-FU
P4 *  *  *  *  *  *  *  *  * 
P5 *  *  *  *  *  *  *  *  * 
P6 *  *  *  *  *  *  *  *  * 
P7+FU+FU+FU+FU+FU+FU+FU+FU+FU
P8 * +KA *  *  *  *  * +HI * 
P9+KY+KE+GI+KI+OU+KI+GI+KE+KY
+
+7776FU
-3334FU
+2726FU
-4344FU
%TORYO
```

**駒の記号**:
- FU: 歩, KY: 香, KE: 桂, GI: 銀, KI: 金
- KA: 角, HI: 飛, OU: 玉
- TO: と, NY: 成香, NK: 成桂, NG: 成銀
- UM: 馬, RY: 龍

```text
**指し手の記号**:
+7776FU  # 先手が77の歩を76に移動
-3334FU  # 後手が33の歩を34に移動
+0055KA  # 先手が角を55に打つ（00は持ち駒）
```

**長所**:
- 厳密な仕様
- パースが容易
- ASCII互換
- 大会で広く使用

**短所**:
- 人間の可読性は低い
- 日本語名が記号化される

### JKF（JSON Kifu Format）

**用途**: Web アプリケーション、JSON ベースのシステム

**特徴**:
- JSON形式
- Kifu-for-JS プロジェクトで開発
- 構造化されたデータ
- JavaScript/Web 技術と親和性が高い

**例**:
```json
{
  "header": {
    "title": "第73期王将戦",
    "black": "藤井聡太",
    "white": "菅井竜也",
    "date": "2024/01/15"
  },
  "initial": {
    "preset": "HIRATE"
  },
  "moves": [
    {"move": {"from": {"x": 7, "y": 7}, "to": {"x": 7, "y": 6}, "piece": "FU"}},
    {"move": {"from": {"x": 3, "y": 3}, "to": {"x": 3, "y": 4}, "piece": "FU"}},
    {"special": "TORYO"}
  ]
}
```

**長所**:
- 構造化されている
- Web技術と親和性が高い
- 拡張が容易
- 型安全な処理が可能

**短所**:
- ファイルサイズが大きい
- まだ普及途上

### SBINPACK（Shogi Binary Pack）

**用途**: 機械学習訓練データ・大量棋譜の保存・転送（NNUE はその一用途）

**特徴**:
- バイナリ形式（テキストではない）
- PackedSfen（256bit 圧縮局面）を基盤
- 指し手を合法手インデックスで表現（ULEB128 + ZigZag エンコード）
- 評価値・勝敗結果も格納可能
- チャンクベース: マジック `SBN2` + チャンクサイズ（u32）。
- chain ごとに opaque metadata bytes 拡張枠を持ち、ユーザー定義の小さな sideband を格納できる

```text
**構造**（数値はすべてリトルエンディアン）:
チャンクヘッダ: "SBN2" (4 bytes) + chunk_size (u32, 4 bytes)
チェーン（先頭局面 = stem）:
  ├─ PackedSfen (32 bytes): 256bit 圧縮局面
  ├─ score (i16, 2 bytes): 先頭局面の評価値
  ├─ best_move (u16, 2 bytes): 先頭局面の最善手（Move の raw 値）
  ├─ ply_result (u16, 2 bytes): result(上位 6 bit) + ply(下位 10 bit)
  ├─ metadata_len (u8): metadata byte 数（0..=127）
  ├─ metadata: opaque user bytes
  └─ count (u16, 2 bytes): 後続手数 N
後続手（N 個ぶん繰り返し）:
  ├─ move (ULEB128): 合法手リスト中のインデックス
  └─ eval_delta (ZigZag + ULEB128): stem 側視点に正規化した評価値差分
```

> 評価値が ZigZag + ULEB128 になるのは後続手の差分（`eval_delta`）のみで、先頭局面の `score` は
> 固定幅 2 バイトの `i16` です。

**長所**:
- 極めてコンパクト（1 局面あたり ~36 バイト）
- ランダムアクセス可能（固定サイズのチャンク）
- 大量データ処理に適している

**短所**:
- 人間が読めない
- 復号に合法手生成が必要（指し手がインデックスのため）
- 専用ツールが必要

**詳細**: [sbinpack 仕様](../../reference/sbinpack.md)

## フォーマット比較表

| フォーマット | 可読性 | サイズ | パース難度 | 主な用途 | メタデータ |
|-------------|--------|--------|-----------|----------|-----------|
| SFEN | 低 | 極小 | 易 | USI, 局面共有 | なし |
| KIF | 高 | 大 | 中 | 棋譜管理, 鑑賞 | 豊富 |
| KI2 | 高 | 中 | 難 | 簡易棋譜 | 中程度 |
| CSA | 低 | 小 | 易 | AI開発, 大会 | 中程度 |
| JKF | 中 | 大 | 易 | Web, API | 豊富 |
| SBINPACK | なし | 極小 | 中 | 機械学習訓練データ/大量棋譜 | 最小限 |

## 用途別の推奨フォーマット

### エンジン開発

**内部表現**: Position 構造体
**USI通信**: SFEN
**テストデータ**: SFEN（コンパクト）または CSA（厳密性）

```rust
// テストケースでの使用例
#[test]
fn test_position_from_sfen() {
    let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
    let pos = Position::from_sfen(sfen).unwrap();
    assert!(pos.is_valid());
}
```

### 棋譜データベース

**保存形式**: CSA（厳密性・省スペース）または JKF（構造化）
**エクスポート**: KIF（人間向け）

### Web アプリケーション

**API通信**: JKF または SFEN
**表示**: KIF 風の整形出力

### 棋譜管理ソフト

**主形式**: KIF
**互換性**: KIF ↔ CSA ↔ KI2 の相互変換

## フォーマット変換の注意点

### 情報の損失

フォーマット間の変換では、情報が失われることがあります：

| 変換 | 失われる情報 |
|------|-------------|
| KIF → SFEN | 棋譜全体、メタデータ、分岐 |
| KI2 → CSA | 曖昧性の解決で推論が必要 |
| CSA → KIF | 日本語表記の復元 |
| SFEN → KIF | 棋譜履歴、メタデータ |

### エンコーディングの問題

**KIF/KI2 形式**でのエンコーディング検出：

```rust,ignore
pub fn detect_encoding(bytes: &[u8]) -> Encoding {
    // BOMチェック
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Encoding::Utf8;
    }

    // UTF-8バリデーション
    if std::str::from_utf8(bytes).is_valid() {
        return Encoding::Utf8;
    }

    // Shift_JISと仮定
    Encoding::ShiftJis
}
```

### 曖昧性の解決

**KI2 形式**では、指し手が曖昧な場合があります：

```text
▲２四歩  # どの歩か？
```

解決方法：
1. 盤面を保持しながらパース
2. 合法手を生成
3. 表記と一致する手を選択

```rust,ignore
pub fn parse_ki2_move(pos: &Position, move_str: &str) -> Result<Move32, ParseError> {
    // 合法手を生成
    let legal_moves = generate_legal_moves(pos);

    // 表記に一致する手を探す
    for mv in legal_moves {
        if move_matches_ki2_notation(mv, move_str, pos) {
            return Ok(mv);
        }
    }

    Err(ParseError::AmbiguousMove)
}
```

## 実装例：複数フォーマット対応

### フォーマット自動判定

```rust
pub enum KifuFormat {
    Sfen,
    Kif,
    Ki2,
    Csa,
    Jkf,
}

impl KifuFormat {
    pub fn detect(content: &str) -> Option<Self> {
        let content = content.trim();

        // SFEN: 1行で完結、特徴的なパターン
        if content.lines().count() == 1 && content.contains('/') {
            return Some(KifuFormat::Sfen);
        }

        // JSON: 最初の文字が '{'
        if content.starts_with('{') {
            return Some(KifuFormat::Jkf);
        }

        // CSA: 'V2.2' で始まる
        if content.starts_with("V2.") {
            return Some(KifuFormat::Csa);
        }

        // KIF vs KI2: より複雑な判定
        if content.contains("手数----指手") {
            return Some(KifuFormat::Kif);
        }

        // KI2: ▲や△の後に座標がない
        if content.contains('▲') || content.contains('△') {
            return Some(KifuFormat::Ki2);
        }

        None
    }
}
```

### 統一インターフェース

```rust,ignore
pub trait KifuParser {
    fn parse(&self, content: &str) -> Result<Game, ParseError>;
}

pub struct SfenParser;
pub struct KifParser;
pub struct CsaParser;

impl KifuParser for SfenParser {
    fn parse(&self, content: &str) -> Result<Game, ParseError> {
        // SFEN パース処理
        let parts: Vec<&str> = content.split_whitespace().collect();

        // position を構築
        let position = if parts.len() > 1 && parts[0] == "sfen" {
            Position::from_sfen(&parts[1..5].join(" "))?
        } else {
            Position::from_sfen(content)?
        };

        // moves を適用（あれば）
        let mut game = Game::new(position);

        if let Some(moves_idx) = parts.iter().position(|&s| s == "moves") {
            for move_str in &parts[moves_idx + 1..] {
                let mv = Move32::from_usi(move_str)?;
                game.do_move(mv)?;
            }
        }

        Ok(game)
    }
}
```

## パフォーマンスの考慮

### ファイルサイズ

1000手の棋譜の典型的なサイズ：

| フォーマット | サイズ（概算） |
|-------------|--------------|
| SFEN (moves付き) | 5-10 KB |
| CSA | 30-50 KB |
| KIF | 50-100 KB |
| JKF | 80-150 KB |

### パース速度

速い順：

1. **SFEN**: 単純な文字列パース
2. **CSA**: 固定フォーマット
3. **JKF**: JSON パーサに依存
4. **KIF**: 複雑なパース処理
5. **KI2**: 曖昧性解決が必要

## フォーマット選択の決定木

```text
局面だけを表現したい？
  → Yes → SFEN
  → No → 棋譜全体が必要
    人間が読む？
      → Yes → KIF（メタデータ豊富）/ KI2（簡潔）
      → No → 機械処理向け
        Web/API で使う？
          → Yes → JKF（JSON 構造化）
          → No → CSA（厳密・省サイズ）
```

## 落とし穴

### KIF/KI2 の文字エンコーディング

古い棋譜ファイル（2010 年代以前）は Shift_JIS エンコーディングが一般的ですが、
新しいファイルは UTF-8 が主流です。バイト列の先頭で BOM を確認し、
なければ UTF-8 として試行、失敗時に Shift_JIS にフォールバックする戦略が実用的です。

### KI2 の曖昧性

KI2 形式では移動元の座標が省略されるため、同じ駒種が複数ある場合に曖昧性が生じます。
パース時には局面を保持しながら合法手を生成し、表記と一致する手を選択する必要があります。
これは指し手生成器と局面管理が連携する複雑な処理になります。

### フォーマット変換の情報損失

`KIF → SFEN` では対局者名・日時・コメント・分岐がすべて失われます。
逆方向 `SFEN → KIF` でも棋譜履歴が欠落します。
変換前にメタデータを別途保存することを検討してください。

## まとめ

- **SFEN**: USI 通信の標準、局面記述に最適、メタデータなし
- **KIF**: 人間向け最優、メタデータ豊富、エンコーディング問題あり
- **CSA**: 機械処理向け、厳密な仕様、大会で広く使用
- **JKF**: Web 親和性が高い JSON 形式、普及途上
- **SBINPACK**: 機械学習訓練データ・大量棋譜向けバイナリ形式、極めてコンパクト（NNUE はその一用途）
- フォーマット選択は**用途**で決定し、変換時の情報損失に注意

## 次に読む

→ **[局面の圧縮](./compression.md)**: 256 ビット圧縮など、局面を効率的に保存する技術を解説します。

## 参考資料

- [将棋の各種フォーマットについて - Qiita](https://qiita.com/sunfish-shogi/items/964e139ef3bfd8f738d4) - 包括的なフォーマット解説
- [SFEN パーサ](./index.md) - SFEN の詳細
- [USI（Universal Shogi Interface）プロトコル（外部資料）](https://shogidokoro2.stars.ne.jp/usi.html) - USI プロトコル
- [CSA標準棋譜ファイル形式](http://www.computer-shogi.org/protocol/record_v22.html) - CSA 仕様書
- [Kifu-for-JS](https://github.com/na2hiro/Kifu-for-JS) - JKF 仕様
