# 形式と互換性

## 1.1.1 との互換性

1.2.0 は内部実装を置き換えていますが、次の永続表現は 1.1.1 と互換です。

| 形式 | 1.2.0 の契約 |
| --- | --- |
| HCP | 32 byte の局面表現を維持する。 |
| PackedSfen | 32 byte の局面表現と byte 順を維持する。 |
| HCPE | 38 byte entry と `GameResult` code を維持する。 |
| PACK | start tag、AperyMove、評価値、terminal marker、終了理由を維持する。 |
| YBB | header、index、move record、PackedSfen key を維持する。 |
| SBK | state identifier と graph reference の意味論を維持する。 |
| Zobrist | 64-bit key と `hash-128` の低位 64 bit を維持する。 |

1.1.1 で生成した有効なデータは 1.2.0 で読み込めます。
1.2.0 への更新だけを理由に再生成する必要はありません。
1.0.x から 1.1.x で変更された static book version と SAZ2 version は別の移行であり、1.2.0 でも旧 version を暗黙に読み替えません。

## 共通の decoder 方針

binary decoder は short buffer、cursor overflow、無効な code、盤外升、過剰または不足した駒在庫、非正規な手を error として返します。
PACK では各着手の合法性と、結果・終了理由の整合性も検査します。

`from_raw` は wire field を保持する低水準 constructor であり、その値が単独で合法であることを保証しません。
外部データには format decoder、`is_normal`、局面の `is_legal_move`、validation API を組み合わせます。

bit-packed 形式は、個別の frozen vector で別に定義されない限り、byte 内を least-significant-bit first で読み書きします。

## SFEN と USI

SFEN は盤面、手番、持ち駒、手数を表します。
DB2016 の key のように手数を identity に含めない API は、先頭 3 field を使い、手数を 1 へ正規化します。

USI position text は `position startpos moves ...` と `position sfen ... moves ...` を解析・正規化します。
通常の指し手に加えて、`Move::from_usi` / `Move32::from_usi` は `0000` を null move として受理します。
null move の正規文字列は `null` です。

## HCP、PackedSfen、HCPE

`HuffmanCodedPos` と `PackedSfen` はそれぞれ `[u8; 32]` を公開します。
どちらも盤面、持ち駒、手番を復元し、局面 API が別に受け取る手数と組み合わせます。
同じ長さでも code table が異なるため、相互に読み替えないでください。

HCPE は 32-byte HCP に評価値、best move、結果を加えた 38-byte entry です。
整数は little-endian です。
`GameResult` の raw code は persisted field なので、enum の宣言順や独自の連番へ変換せず、公開変換 API を使います。

## PACK

PACK は複数 game を区切りなしで連結できます。
1 game は次の順です。

1. start tag 1 byte。`1` は平手初期局面、`0` は後続する 32-byte HCP と little-endian `game_ply: u16` を使う。
2. 0 個以上の ply。各 ply は `move: u16`、`eval: i16` の little-endian 4 byte。
3. `result | (result << 7)` で表す terminal marker 2 byte。
4. end reason 1 byte。

ply の `move` は通常の `Move` ではなく `AperyMove` layout です。

- bit 0–6: 移動先の升 0–80。
- bit 7–13: 移動元の升 0–80。駒打ちは 81–87 が歩、香、桂、銀、角、飛、金に対応する。
- bit 14: 成り。
- bit 15: 0 固定。

駒打ちと成りは同時に指定できません。
`AperyMove::to_move` / `Move::to_apery` で変換し、raw 値を `Move::from_raw` へ渡さないでください。

`decode_game` と `record_from_game` は layout の正規性に加え、開始局面から各手が合法かを検査します。
`game_from_record` は各 main-line node に評価値と terminal を要求し、特殊評価値を PACK の通常評価範囲 `-32000..=32000` へ clamp します。
Rust の `decode_games` と Python の `decode_pack_file` は連結された全 game を読みます。

## 棋譜 text

KIF、KI2、CSA、JKF は共通の `Record` tree へ変換されます。
main line、変化、terminal、metadata を保持できる範囲は形式ごとに異なるため、形式間変換が byte-for-byte round trip になるとは限りません。

KIF の `**評価値=` は ShogiHome 互換の先手視点です。
内部の `EngineInfo.eval` は、その entry の直前の手番側から見た値なので、KIF 入出力時に符号を変換します。
KI2 の `同` は直後に全角スペースを一つ置く正規表記を使います。

CSA は V2.2 と V3.0 を扱います。
V3.0 出力は millisecond の消費時間と encoding 宣言を保持します。
複数局を `/` で区切る CSA には multi-game parser を使います。

## DB2016 text book

fixed header は `#YANEURAOU-DB2016 1.00` です。
header 自体は省略できますが、`#YANEURAOU-DB2016` で始まる別 version は unsupported です。
UTF-8 BOM は先頭行だけに許され、LF と CRLF は同等です。
空行、`#` comment、`//` comment を受理します。

position 行は `sfen <board> <side> <hands> [ply]` です。
identity は先頭 3 SFEN field で、内部では ply 1 へ正規化します。
ply を省略した position は metadata value 0 として保持し、数値でない ply は error です。

move 行は先頭の `move` を省略でき、次の field を持ちます。

```text
[move] <move|resign> [ponder|none] [value|none] [depth|none] [count]
```

`resign` は有効な move token です。
省略した optional field は `None` として保持します。
`YaneuraOuBook::iter_entries()` は file を逐次読み込み、不正な move 行を含む position group を `Err` として一度返した後、次の position group から読み続けます。
数 GB の book 全体を `Vec` へ展開しません。

既定の `YaneuraOuBook::open` は先頭 10,000 position の整列を確認します。
部分確認だけで binary lookup は行わないため、大規模 book は `validate_full()` を完了するか、`lookup_sfen_by_scan()` を選びます。
`YaneuraOuAccessMode` を使う Rust caller は、完全検証、caller 保証、scan-only を明示できます。

`YaneuraOuDb2016WriteOptions::new()` の writer は position group を正規化 SFEN の bytewise 昇順に並べます。
候補手は既定で score 降順、`with_preserved_move_order()` で入力順です。
`with_fixed_ply()`、`with_omitted_ply()`、`with_keep_last_on_duplicate()`、`with_noe()` で出力を調整できます。
元の DB2016 で省略されていた ply は再び省略し、`count: None` は token を出力しません。
ponder、score、depth の未知値は、それより後ろの field の位置を保つため `none` として出力します。

## YBB binary book

YBB は little-endian です。
32-byte header は 16-byte magic `YANE-BINBOOK-V1\0`、`record_count: u64`、`flags: u64` です。
定義済み flag は bit 0 のみで、move record が `depth` を持つことを表します。

後続する 44-byte index record は `packed_sfen[32]`、`moves_offset: u64`、`ply: u16`、`move_count: u16` です。
index は unsigned 32-byte PackedSfen の lexicographic 順で、`moves_offset` は move area の先頭からの相対位置です。
move record は `raw_move: u16`、`eval: i16`、optional `depth: u16` の 4 byte または 6 byte です。

既定 lookup は PackedSfen と ply の両方を照合します。
ignore-ply は同じ PackedSfen の最初の run を選びます。
flipped lookup は通常 lookup が miss した場合だけ行い、返す手を caller の座標へ戻します。

## SBK と SAZ2

SBK の graph reference は配列上の偶然の位置ではなく、公開された state identifier で解決します。
duplicate identifier、missing child、無効な手、short output は error または diagnostics になります。

SAZ2 は self-play game、各局面の played move、WDL、raw network outputs、visit snapshot、target weight、flags、optional mate、policy entry を保持します。
policy distribution は整数固定小数点で、合計値を独立に検証します。
unsupported version は field を推測して読み替えず、明示的に拒否します。
