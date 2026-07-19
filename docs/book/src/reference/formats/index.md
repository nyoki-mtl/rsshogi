# 棋譜フォーマット一覧

rsshogiは、将棋の棋譜を表現する主要なフォーマットに対応しています。各フォーマットには特徴と用途があり、状況に応じて使い分けることができます。

## サポートしているフォーマット

| フォーマット | 説明 | パース | 出力 | 主な用途 |
|--------------|------|--------|------|----------|
| [KIF](kif.md) | 人間が読みやすい標準形式 | ✅ | ✅ | 棋譜の保存・閲覧 |
| [KI2](ki2.md) | KIFの簡略版 | ✅ | ✅ | 棋譜入力・閲覧 |
| [CSA](csa.md) | コンピュータ将棋協会の標準形式 | ✅ | ✅ | データ交換・大会 |
| [JKF](jkf.md) | JSON形式の棋譜 | ✅ | ✅ | Webアプリ・プログラム処理 |

## 学習バイナリ形式

| フォーマット | 説明 | 主な用途 |
|--------------|------|----------|
| [HCP / HCPE / PackedSfen / pack](training-binaries.md) | 固定長/可変長の局面・学習データ | NN 学習データ、主要な参照実装との互換 |
| [sbinpack](../sbinpack.md) | バイナリ棋譜（Stockfish binpack の将棋版） | NN 学習・バイナリ棋譜アーカイブ |
| [sazpack](../sazpack.md) | AlphaZero 系 sparse policy 教師データ | AZ/self-play shard・policy/value 学習 |

## 定跡フォーマット

| フォーマット | 説明 | 主な用途 |
|--------------|------|----------|
| [定跡アーキテクチャ（3 層モデル）](book-architecture.md) | ルックアップ層 / 外部リーダ層 / 編集 IR 層の設計 | 定跡 API の全体像把握 |
| [Book](book.md) | rsshogi 独自の定跡バイナリ | 大規模定跡の高速参照 |
| [外部定跡（DB2016 / YBB / SBK）](external-books.md) | DB2016・YBB・SBK 形式の外部定跡 | 既存ツールと互換性のある定跡の参照 |
| [DB2016 形式](yaneuraou.md) | やねうら王のテキスト定跡フォーマット詳細 | DB2016 の構造・書式の理解 |
| [SBK 形式](sbk.md) | ShogiGUI 由来の protobuf 定跡フォーマット詳細 | SBK の構造・スキーマの理解 |

## フォーマット比較

### 可読性

**最も読みやすい：** KI2 > KIF > CSA > JKF

- **KI2**：自然な日本語表記で最も読みやすい
- **KIF**：日本語表記で読みやすく、詳細な情報も含む
- **CSA**：機械処理優先だが、慣れれば読める
- **JKF**：JSON形式で人間が直接読むには不向き

### ファイルサイズ

**最も小さい：** CSA ≈ KI2 < KIF < JKF

- **CSA**：簡潔な表記で最小
- **KI2**：移動元座標を省略するため小さい
- **KIF**：詳細な情報を含むためやや大きい
- **JKF**：JSON形式のオーバーヘッドで最大

### プログラムでの扱いやすさ

**最も扱いやすい：** JKF > CSA > KIF > KI2

- **JKF**：構造化されたJSON形式で解析が容易
- **CSA**：シンプルな文法で実装しやすい
- **KIF**：日本語表記だが、構造は明確
- **KI2**：局面を進めながら解析する必要があり複雑

### 情報の保持

**最も豊富：** JKF ≈ KIF ≈ CSA V3.0 > CSA V2.x > KI2

- **JKF**：分岐、コメント、時間など全てサポート
- **KIF**：分岐、コメント、しおりなど豊富
- **CSA V3.0**：評価値・読み筋など新機能を追加
- **CSA V2.x**：基本的な情報を保持
- **KI2**：基本的な指し手情報のみ

## フォーマット選択のガイド

### 用途別の推奨フォーマット

#### 棋譜を人間が読む・編集する

→ **KI2** または **KIF** を推奨

- **KI2**：より簡潔で読みやすい
- **KIF**：詳細な情報も確認したい場合

#### プログラムで棋譜を処理する

→ **JKF** または **CSA** を推奨

- **JKF**：Webアプリケーション、JavaScript/TypeScript
- **CSA**：コンピュータ将棋、データ交換

#### 将棋ソフト間でデータ交換

→ **CSA** を推奨

- コンピュータ将棋協会の標準形式
- 多くのソフトがサポート

#### 棋譜データベース・アーカイブ

→ **KIF** または **CSA** を推奨

- **KIF**：情報が豊富で可読性も高い
- **CSA**：ファイルサイズが小さく、処理も高速

#### Webアプリケーション

→ **JKF** を推奨

- JSON形式でJavaScriptとの親和性が高い
- 構造化されたデータで扱いやすい

## rsshogi の互換方針

rsshogi は仕様書どおりの文法だけでなく、実際に流通している棋譜との実務互換も重視します。

- parser は ShogiHome/tsshogi 系が読み書きする入力を広く受ける方向を優先します。
- exporter は `shogi-validator` に近い標準的な見た目を意識しつつ、
  `Record` が保持している終局情報やコメントをできるだけ落としません。
- KIF/KI2 では、変化手順と `まで...` 要約が同居する場合、変化ブロックを先に、
  要約行を最後に出力します。
- CSA では、CSA 3.0 のプログラムコメントである `'*...` をコメントとして受理し、出力も `'*...` に寄せます。plain な `'...` は通常コメントとして読み飛ばします。

## フォーマット詳細

### [KIF形式](kif.md)

柿木将棋や「Kifu for Windows」で採用されている、人間が読みやすい棋譜形式です。

**特徴：**
- 日本語表記で読みやすい
- コメント機能が充実
- しおり機能で特定局面を記録
- 詰将棋情報にも対応

```text
**例：**
開始日時：1999/07/15
手合割：平手
先手：先手の名前
後手：後手の名前
手数----指手---------消費時間--
   1 ７六歩(77)   ( 0:16/00:00:16)
   2 ３四歩(33)   ( 0:00/00:00:00)
   3 中断         ( 0:03/ 0:00:19)
```

### [KI2形式](ki2.md)

KIF形式を簡略化したもので、移動元座標を省略し、1行に複数の指し手を記録できます。

**特徴：**
- 移動元座標を省略
- 1行に複数の指し手を記録可能
- より簡潔で読みやすい
- ファイルサイズが小さい

**例：**
開始日時：1999/07/15
手合割：平手
先手：先手の名前
後手：後手の名前

```text
▲７六歩 △３四歩 ▲２六歩 △８四歩 ▲２五歩 △８五歩
▲７八金 △３二金 ▲２四歩 △同　歩 ▲同　飛 △２三歩打
```

### [CSA形式](csa.md)

コンピュータ将棋協会が定めた標準的な棋譜交換フォーマットです。

**特徴：**
- プログラムによる解析を重視
- 標準化された駒と位置の表記
- バージョン管理により拡張可能
- V3.0で評価値・読み筋に対応

```text
**例：**
'CSA encoding=UTF-8
V3.0
N+先手
N-後手
$EVENT:第34回世界コンピュータ将棋選手権
$START_TIME:2024/05/05 15:05:40
PI
+
+2726FU
T0
-3334FU
T6
%CHUDAN
```

### [JKF形式](jkf.md)

将棋の棋譜をJSON形式で表現するフォーマットです。

**特徴：**
- JSON形式でプログラムでの扱いが容易
- 構造化されたデータ
- 分岐を完全にサポート
- Webアプリケーションとの親和性が高い

**例：**
```json
{
  "header": {
    "先手": "先手の名前",
    "後手": "後手の名前",
    "開始日時": "1999/07/15"
  },
  "initial": {
    "preset": "HIRATE"
  },
  "moves": [
    {},
    {
      "move": {
        "from": {"x": 7, "y": 7},
        "to": {"x": 7, "y": 6},
        "piece": "FU"
      }
    }
  ]
}
```

## フォーマット間の変換

rsshogiを使用すると、フォーマット間の変換が簡単に行えます：

### Python での例

```python
import rsshogi as rs

# KIF を読み込み
kif_text = open("game.kif", "r", encoding="utf-8").read()
record = rs.record.Record.from_kif_str(kif_text)

# 各フォーマットに変換
csa_text = record.to_csa()
ki2_text = record.to_ki2()
jkf_text = record.to_jkf()

# ファイルに保存
with open("game.csa", "w", encoding="utf-8") as f:
    f.write(csa_text)

with open("game.ki2", "w", encoding="utf-8") as f:
    f.write(ki2_text)

with open("game.jkf", "w", encoding="utf-8") as f:
    f.write(jkf_text)
```

### Rust での例

```rust,ignore
use rsshogi::records::formats::{kif, csa, ki2, jkf};
use std::fs;

// KIF を読み込み
let kif_text = fs::read_to_string("game.kif")?;
let record = kif::parse_kif_str(&kif_text)?;

// 各フォーマットに変換
let csa_text = csa::export_csa(&record)?;
let ki2_text = ki2::export_ki2(&record)?;
let jkf_text = jkf::export_jkf(&record)?;

// ファイルに保存
fs::write("game.csa", csa_text)?;
fs::write("game.ki2", ki2_text)?;
fs::write("game.jkf", jkf_text)?;
```

## rsshogi の対応状況

### 完全対応

- ✅ KIF のパース・出力
- ✅ CSA のパース・出力
- ✅ KI2 のパース・出力
- ✅ JKF のパース・出力

### 部分対応

- ⚠️ CSA V3.0 の新機能（評価値・読み筋）は部分対応

### 未対応

- ❌ 評価値の出力（全フォーマット）

## 実装上の注意点

### 分岐の扱い

各フォーマットでの分岐の対応状況：

| フォーマット | パース | 出力 | 備考 |
|--------------|--------|------|------|
| KIF | ✅ | ✅ | `変化：N手` 形式 |
| KI2 | ✅ | ✅ | `変化：N手` 形式 |
| CSA | ⚠️ | ❌ | 仕様上の制限 |
| JKF | ✅ | ✅ | `forks` フィールド |

### 文字コード

| フォーマット | 推奨文字コード | 備考 |
|--------------|----------------|------|
| KIF | UTF-8 (`.kifu`) / Shift-JIS (`.kif`) | 拡張子で区別 |
| KI2 | UTF-8 / Shift-JIS | 自動判定 |
| CSA | UTF-8 (V3.0) / Shift-JIS (V2.x) | ファイル内で宣言 |
| JKF | UTF-8 | JSON標準 |

## 参考リンク

### 公式仕様

- [棋譜ファイル KIF 形式（柿木義一氏）](https://kakinoki.o.oo7.jp/kif_format.html)
- [Kifu for Windows の紹介](https://kakinoki.o.oo7.jp/KifuwInt.htm)
- [棋譜の表記方法（日本将棋連盟）](https://www.shogi.or.jp/faq/kihuhyouki.html)：公式な棋譜表記ルール
- [CSA標準棋譜ファイル形式 V3.0](http://www2.computer-shogi.org/protocol/record_v3.html)
- [CSA標準棋譜ファイル形式 V2.2](http://www2.computer-shogi.org/protocol/record_v22.html)
- [CSA標準棋譜ファイル形式 V2.1](http://www2.computer-shogi.org/protocol/record_v21.html)
- [JSON Kifu Format（Kifu for JS）](https://github.com/na2hiro/Kifu-for-JS/tree/master/packages/json-kifu-format)

### 関連サイト

- [コンピュータ将棋協会](http://www2.computer-shogi.org/)
- [柿木将棋](https://kakinoki.o.oo7.jp/)
- [Kifu for JS](https://github.com/na2hiro/Kifu-for-JS)

## 関連ドキュメント

- [Record API（Python）](../../python/record.md)
- [Records API（Rust）](https://docs.rs/rsshogi)：docs.rs（外部サイト）
