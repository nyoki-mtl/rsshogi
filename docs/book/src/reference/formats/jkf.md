# JKF 形式

JKF（JSON Kifu Format）は、将棋の棋譜をJSON形式で表現するフォーマットです。プログラムでの扱いが容易で、Webアプリケーションとの親和性が高いのが特徴です。

## 公式仕様

- [Kifu for JS - JSON Kifu Format](https://github.com/na2hiro/Kifu-for-JS/tree/master/packages/json-kifu-format)

## 概要

JKF形式は以下の特徴を持ちます：

- **JSON形式**でプログラムでの扱いが容易
- **構造化されたデータ**で解析が簡単
- **分岐（変化手順）を完全にサポート**
- **メタデータを豊富に保持**
- **Webアプリケーションとの親和性が高い**
- **型安全**（TypeScript等で型定義が可能）

## 基本構造

JKF形式の棋譜は、以下の主要なフィールドで構成されます：

```json
{
  "header": { /* 対局情報 */ },
  "initial": { /* 開始局面 */ },
  "moves": [ /* 指し手のリスト */ ]
}
```

## ヘッダー情報（header）

対局に関するメタ情報を保持します：

```json
{
  "header": {
    "先手": "先手の対局者名",
    "後手": "後手の対局者名",
    "棋戦": "第68期順位戦A級",
    "戦型": "矢倉",
    "開始日時": "1999/07/15(木) 19:07:12",
    "終了日時": "1999/07/15(木) 19:07:17",
    "表題": "注目の一局",
    "持ち時間": "各6時間",
    "場所": "東京・将棋会館",
    "掲載": "将棋世界 2019年7月号",
    "備考": "備考欄"
  }
}
```

### 標準的なヘッダーフィールド

| フィールド名 | 説明 | 型 |
|--------------|------|-----|
| `先手` | 先手（下手）の対局者名 | string |
| `後手` | 後手（上手）の対局者名 | string |
| `棋戦` | 棋戦名 | string |
| `戦型` | 戦型 | string |
| `開始日時` | 対局開始日時 | string |
| `終了日時` | 対局終了日時 | string |
| `表題` | 棋譜の表題 | string |
| `持ち時間` | 持ち時間 | string |
| `場所` | 対局場所 | string |
| `掲載` | 掲載誌 | string |
| `備考` | 備考 | string |
| `手合割` | 手合割 | string |

ヘッダーには任意のフィールドを追加できます。

## 開始局面（initial）

対局の開始局面を定義します：

```json
{
  "initial": {
    "preset": "HIRATE"
  }
}
```

### 平手局面

```json
{
  "initial": {
    "preset": "HIRATE"
  }
}
```

### 駒落ち局面

```json
{
  "initial": {
    "preset": "2"  // 二枚落ち
  }
}
```

#### 駒落ちのプリセット

| preset値 | 手合割 |
|----------|--------|
| `"HIRATE"` | 平手 |
| `"KY"` | 香落ち |
| `"KY_R"` | 右香落ち |
| `"KA"` | 角落ち |
| `"HI"` | 飛車落ち |
| `"HIKY"` | 飛香落ち |
| `"2"` | 二枚落ち |
| `"3"` | 三枚落ち |
| `"4"` | 四枚落ち |
| `"5"` | 五枚落ち |
| `"5_L"` | 左五枚落ち |
| `"6"` | 六枚落ち |
| `"7_L"` | 左七枚落ち |
| `"7_R"` | 右七枚落ち |
| `"8"` | 八枚落ち |
| `"10"` | 十枚落ち |
| `"OTHER"` | その他 |

### 任意の局面

```json
{
  "initial": {
    "data": {
      "color": 0,  // 0: 先手番, 1: 後手番
      "board": [
        [{"color":1,"kind":"KY"}, {"color":1,"kind":"KE"}, ...],
        [...],
        ...
      ],
      "hands": [
        {"FU": 0, "KY": 0, ...},  // 先手の持駒
        {"FU": 3, "KA": 1, ...}   // 後手の持駒
      ]
    }
  }
}
```

#### 駒の表記

駒オブジェクトは `color`（手番）と `kind`（駒種）で表現します：

```json
{
  "color": 0,  // 0: 先手, 1: 後手
  "kind": "FU" // 駒種（後述）
}
```

#### 駒種（kind）

| 駒種 | 意味 | 成駒 | 意味 |
|------|------|------|------|
| `FU` | 歩 | `TO` | と |
| `KY` | 香 | `NY` | 成香 |
| `KE` | 桂 | `NK` | 成桂 |
| `GI` | 銀 | `NG` | 成銀 |
| `KI` | 金 | - | - |
| `KA` | 角 | `UM` | 馬 |
| `HI` | 飛 | `RY` | 龍 |
| `OU` | 玉 | - | - |

#### 盤面（board）

- 9x9の二次元配列
- `board[y][x]` で (x+1, y+1) の位置を表す
- 駒がない場合は `{}`（空オブジェクト）

#### 持駒（hands）

- 2要素の配列 `[先手の持駒, 後手の持駒]`
- 各駒種の枚数をオブジェクトで表現
- 持っていない駒種は省略可能（0として扱う）

## 指し手（moves）

指し手のリストを配列で表現します：

```json
{
  "moves": [
    {},  // 初手前の状態（コメント用）
    {
      "move": {
        "from": {"x": 7, "y": 7},
        "to": {"x": 7, "y": 6},
        "piece": "FU"
      },
      "time": {"now": {"m": 0, "s": 16}, "total": {"h": 0, "m": 0, "s": 16}},
      "comments": ["初手は７六歩"]
    },
    {
      "move": {
        "from": {"x": 3, "y": 3},
        "to": {"x": 3, "y": 4},
        "piece": "FU"
      },
      "comments": ["△３四歩"]
    },
    // ...
  ]
}
```

`rsshogi` の exporter では、この先頭要素の `comments` に
`Record.initial_comment` を出力します。`RecordMetadata.comment` は
header の `備考` に出力されます。

### 通常の移動

```json
{
  "move": {
    "from": {"x": 7, "y": 7},
    "to": {"x": 7, "y": 6},
    "piece": "FU"
  }
}
```

- `from`：移動元の座標 `{x: 1-9, y: 1-9}`
- `to`：移動先の座標 `{x: 1-9, y: 1-9}`
- `piece`：移動後の駒種（成る場合は成駒）

### 駒打ち

```json
{
  "move": {
    "to": {"x": 2, "y": 3},
    "piece": "FU"
  }
}
```

- `from` フィールドがない場合は駒打ち
- `piece`：打つ駒種

### 成り

```json
{
  "move": {
    "from": {"x": 2, "y": 2},
    "to": {"x": 2, "y": 2},
    "piece": "UM",
    "promote": true
  }
}
```

- `piece`：成駒の駒種
- `promote`：`true` の場合は成り

### 同の指し手

```json
{
  "move": {
    "from": {"x": 2, "y": 3},
    "to": {"x": 2, "y": 4},
    "piece": "FU",
    "same": true
  }
}
```

- `same`：`true` の場合、直前の手と同じ位置への移動

### 相対指定

```json
{
  "move": {
    "from": {"x": 5, "y": 8},
    "to": {"x": 5, "y": 8},
    "piece": "KI",
    "relative": "R"
  }
}
```

- `relative`：相対位置（`"L"`: 左, `"C"`: 直, `"R"`: 右, `"U"`: 上, `"M"`: 寄, `"D"`: 引, `"H"`: 行）

## 時間情報（time）

消費時間を記録します：

```json
{
  "time": {
    "now": {"m": 0, "s": 16},
    "total": {"h": 0, "m": 0, "s": 16}
  }
}
```

- `now`：その手の消費時間
- `total`：累積の消費時間

時間は `h`（時）、`m`（分）、`s`（秒）で表現します。

## コメント（comments）

各手にコメントを付加できます：

```json
{
  "move": { /* ... */ },
  "comments": [
    "この手が好手",
    "形勢は互角"
  ]
}
```

コメントは文字列の配列として表現します。

## 分岐（forks）

変化手順を `forks` フィールドで表現します：

```json
{
  "moves": [
    {},
    {"move": { /* 1手目 */ }},
    {"move": { /* 2手目 */ }},
    {
      "move": { /* 3手目 */ },
      "forks": [
        [
          {"move": { /* 変化1の3手目 */ }},
          {"move": { /* 変化1の4手目 */ }}
        ],
        [
          {"move": { /* 変化2の3手目 */ }},
          {"move": { /* 変化2の4手目 */ }}
        ]
      ]
    }
  ]
}
```

- `forks`：変化手順の配列
- 各要素は、その局面からの別の手順を表す配列

### 分岐の例

```json
{
  "moves": [
    {},
    {
      "move": {
        "from": {"x": 7, "y": 7},
        "to": {"x": 7, "y": 6},
        "piece": "FU"
      }
    },
    {
      "move": {
        "from": {"x": 3, "y": 3},
        "to": {"x": 3, "y": 4},
        "piece": "FU"
      }
    },
    {
      "move": {
        "from": {"x": 2, "y": 7},
        "to": {"x": 2, "y": 6},
        "piece": "FU"
      },
      "comments": ["本譜"],
      "forks": [
        [
          {
            "move": {
              "from": {"x": 7, "y": 8},
              "to": {"x": 6, "y": 8},
              "piece": "KI"
            },
            "comments": ["変化手順"]
          }
        ]
      ]
    }
  ]
}
```

## 特殊な指し手

### 投了・中断など

```json
{
  "special": "TORYO"  // 投了
}
```

特殊な手の種類：

| special値 | 意味 |
|-----------|------|
| `"TORYO"` | 投了 |
| `"CHUDAN"` | 中断 |
| `"SENNICHITE"` | 千日手 |
| `"TIME_UP"` | 時間切れ |
| `"ILLEGAL_MOVE"` | 反則負け |
| `"JISHOGI"` | 持将棋 |
| `"KACHI"` | 入玉宣言勝ち |
| `"HIKIWAKE"` | 引き分け宣言 |
| `"MATTA"` | 待った |
| `"TSUMI"` | 詰み |
| `"FUZUMI"` | 不詰 |
| `"ERROR"` | エラー |

## 完全な例

```json
{
  "header": {
    "先手": "先手の対局者名",
    "後手": "後手の対局者名",
    "棋戦": "第68期順位戦A級",
    "開始日時": "1999/07/15(木) 19:07:12",
    "終了日時": "1999/07/15(木) 19:07:17"
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
      },
      "time": {
        "now": {"m": 0, "s": 16},
        "total": {"h": 0, "m": 0, "s": 16}
      },
      "comments": ["初手は７六歩"]
    },
    {
      "move": {
        "from": {"x": 3, "y": 3},
        "to": {"x": 3, "y": 4},
        "piece": "FU"
      },
      "time": {
        "now": {"m": 0, "s": 0},
        "total": {"h": 0, "m": 0, "s": 0}
      },
      "comments": ["△３四歩"]
    },
    {
      "special": "CHUDAN"
    }
  ]
}
```

## rsshogi での対応状況

### サポート済み

- ✅ JKF形式への出力
- ✅ JKF形式からのパース
- ✅ 基本的な棋譜情報の出力
- ✅ 指し手と時間情報の出力
- ✅ コメントの出力
- ✅ 分岐（forks）の出力

### 制限事項

- ⚠️ 評価値の出力は未対応
- ⚠️ 一部の特殊な指し手は未対応

### 使用例（Python）

```python
import rsshogi as rs
import json

# KIF を読み込む
kif_text = """
開始日時：1999/07/15
手合割：平手
先手：先手の名前
後手：後手の名前
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 中断
"""

record = rs.record.Record.from_kif_str(kif_text)

# JKF に変換
jkf_text = record.to_jkf()

# JSON として解析
jkf_data = json.loads(jkf_text)

print(json.dumps(jkf_data, indent=2, ensure_ascii=False))

# JKF から Record へ戻す
roundtrip = rs.record.Record.from_jkf_str(jkf_text)
assert roundtrip.moves[0].move.to_usi() == "7g7f"
```

## 利点と制約

### 利点

- **プログラムでの扱いが容易**：JSON形式で解析が簡単
- **構造化されたデータ**：型安全な実装が可能
- **Webアプリケーションとの親和性**：JavaScriptとの相性が良い
- **分岐を完全にサポート**：変化手順を自然に表現
- **拡張性が高い**：任意のフィールドを追加可能

### 制約

- **ファイルサイズがやや大きい**：JSON形式のため
- **人間が直接読むには不向き**：構造が複雑
- **既存のソフトとの互換性**：KIF/CSA形式の方が広くサポートされている

## TypeScript 型定義（参考）

```typescript
interface JKF {
  header: { [key: string]: string };
  initial?: {
    preset?: string;
    data?: InitialData;
  };
  moves: Move[];
}

interface Move {
  move?: MoveData;
  time?: TimeData;
  comments?: string[];
  forks?: Move[][];
  special?: string;
}

interface MoveData {
  from?: { x: number; y: number };
  to: { x: number; y: number };
  piece: string;
  same?: boolean;
  promote?: boolean;
  relative?: string;
}
```

## 関連リンク

- [Kifu for JS - JSON Kifu Format（公式）](https://github.com/na2hiro/Kifu-for-JS/tree/master/packages/json-kifu-format)
- [Kifu for JS](https://github.com/na2hiro/Kifu-for-JS)

## 関連項目

- [KIF形式](kif.md)：人間が読みやすい棋譜形式
- [CSA形式](csa.md)：コンピュータ将棋協会の標準形式
- [KI2形式](ki2.md)：簡易棋譜形式
