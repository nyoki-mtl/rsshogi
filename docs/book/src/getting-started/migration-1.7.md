# 1.7.0 への移行

1.7.0 は Rust の Native Book 変換 API と、Rust / Python の `NativeBookBuilder` のエラー処理に破壊的変更を含む。
対象は `NativeBookBuilder` のエラー分類、`NativeBookImportOptions` と、`BookDatabase`、`YaneuraOuBook`、`SbkBook` の `write_native_book` の引数と変換挙動である。
YBB には同じ契約の `YbbBook::write_native_book` を追加する。
Native Book v4 の保存形式は変更しない。

1.6.0 で非推奨化した API は引き続き維持する。
その対象と 2.0.0 への移行は [1.6.0 の移行ガイド](migration-1.6.md) を参照。

## 設定の構築と引数

`NativeBookImportOptions { allow_lossy: true }` の構造体リテラルは、`NativeBookImportOptions::new().with_allow_lossy(true)` へ変更する。
設定は `Copy` を持たず、変換メソッドへ参照で渡す。
同じ設定を複数の変換へ再利用できる。

```rust,no_run
use rsshogi::book::{DEPTH_KIND_SEARCH, NativeBookImportOptions, YaneuraOuBook};

let source = YaneuraOuBook::open("source.db")?;
let options = NativeBookImportOptions::new()
    .with_depth_kind(DEPTH_KIND_SEARCH)
    .with_allow_lossy(true)
    .with_memory_budget(64 * 1024 * 1024)
    .with_block_positions(64);
let report = source.write_native_book("output.book", &options)?;
# Ok::<(), rsshogi::book::BookError>(())
```

`with_metadata` は題名、説明、深さ種別をまとめて置き換える。
その後の `with_depth_kind` は深さ種別だけを置き換える。
SBK では説明が `None` のときだけ元の description を引き継ぐ。
説明を削除する場合は `Some(String::new())` を指定する。

構築予算は既定 256 MiB、ブロック局面数は既定 64 とする。
予算が 1 MiB 未満、または局面数が `1..=16384` の範囲外なら、入力走査前に `BookError::InvalidData` を返す。
予算は builder の構築用データを対象とし、reader の保持領域や入力 1 行のメモリを含まない。

## NativeBookBuilder のエラー処理

`NativeBookBuilder` を直接利用する場合も、1.7.0 では次のエラー分類へ移行する。
シグネチャと `BookError` の型は変わらないため、コンパイルが通っても既存の `InvalidFormat` 分岐では捕捉できない場合がある。

| 失敗条件 | 1.6.0 | 1.7.0 |
|---|---|---|
| `new()` の保存先が既に存在する | `InvalidFormat` | `Io` の `ErrorKind::AlreadyExists` |
| 同一 run 内または run 間の重複局面 | `InvalidFormat` | `InvalidData` |
| メモリ予算やブロック局面数の範囲外、入力開始後の設定変更 | `InvalidFormat` | `InvalidData` |
| 単一レコードがメモリ予算を超える | `InvalidFormat` | `InvalidData` |

既存保存先は `BookError::Io(error)` の `error.kind() == std::io::ErrorKind::AlreadyExists` で判定する。
入力や設定の不正は `BookError::InvalidData(_)`、保存後に結果が確定しない場合は `BookError::SaveUncertain(_)` として処理する。
エラーメッセージによる機械的な判定は行わない。

```rust,no_run
use rsshogi::book::{BookError, NativeBookBuilder, NativeBookMetadata};
use std::io::ErrorKind;

match NativeBookBuilder::new("output.book", NativeBookMetadata::default()) {
    Ok(builder) => builder.finish()?,
    Err(BookError::Io(error)) if error.kind() == ErrorKind::AlreadyExists => {
        eprintln!("destination already exists");
    }
    Err(error) => return Err(error),
}
# Ok::<(), BookError>(())
```

公開時に競合した保存先も `Io(AlreadyExists)` として扱う。
ファイル構造の非対応やサイズ上限などは引き続き `InvalidFormat` になり得るため、この variant の処理自体を削除しない。

Python の `NativeBookBuilder` も、保存先が既に存在する場合の例外を `ValueError` から `FileExistsError`（`OSError` のサブクラス）へ変更する。
構築時と `finish()` 時の保存先競合は `except FileExistsError:` で捕捉する。
重複局面や不正なブロック局面数など、`InvalidData` に対応する例外は引き続き `ValueError` である。

## 深さ種別を明示する

全形式で `depth_kind` の既定値を `DEPTH_KIND_UNSPECIFIED`（0）とする。
DB2016 の変換も探索深さ（1）を自動設定しない。
探索由来の深さなら `with_depth_kind(DEPTH_KIND_SEARCH)`、定跡内の後退解析距離なら `DEPTH_KIND_RETROGRADE`（2）を指定する。
由来が分からない場合は未指定のまま扱う。
未知の `u32`、深さの `None` と `Some(0)` は保持する。
深さ 0 や 9999 から生成方法を推測しない。

## 欠落許可と局面スキップを分ける

| 入力の状態 | 1.7.0 の扱い |
|---|---|
| 手数、回数、ponder など保存できないフィールド | `with_allow_lossy(true)` の場合だけ捨てて報告する。 |
| SFEN 解析と候補手検査に成功するが PackedSfen に符号化できない局面 | `with_skip_unencodable_positions(true)` の場合だけ除外する。 |
| 構文不正 SFEN、復号不能または非正準のキー、非合法手、重複候補手、同一保存局面への複数行 | オプションによらず拒否する。 |
| SBK reader の重複局面診断 | 変換開始時に拒否する。 |
| SBK reader の未解決状態 | `with_allow_lossy(true)` の場合だけ欠落状態数として報告する。 |

`allow_lossy` だけで局面の除外を許していた呼び出しは、必要な場合にだけ専用スキップ設定を追加する。
候補手の検査はスキップ判定より先に行う。
`allow_lossy = false` では、対象行の SFEN と候補手を検査してから最初の未許可の欠落で中断する。
エラーには入力位置、取得可能な SFEN、欠落フィールドを含め、後続入力は走査しない。
失敗時の診断は全件の欠落集計や入力検査の完了を意味しない。
スキップ行は `skipped_positions` と `dropped_fields["position"]` に各 1 件を加算し、その行の手数や回数を重ねて数えない。
この除外自体は `allow_lossy = false` でも許可されるが、保存する他行のフィールド欠落は引き続き `allow_lossy` に従う。

SBK の候補手検査は reader が返す解決済み行を対象とする。
未解決状態や reader が省略した要素まで元ファイル全体の検査済みとは扱わない。

## 欠落集計と保存結果を確認する

DB2016 の明示手数は `position.original_ply` を 1 件とし、手数省略時は数えない。
所有 IR の `yaneuraou.min_ply` は元手数（省略時は 0）と異なる場合だけ追加計上する。
YBB の手数は各行の `ybb.ply` として数える。
診断の先頭 32 件は `issues`、それ以降の件数は `omitted_issues` に保持する。
未知の `dropped_fields` キーも表示または保持し、説明文字列 `reason` やエラーメッセージを機械的な判定に使わない。

入力データと設定の不正は `BookError::InvalidData` とする。
重複局面の診断には保存キーと復元可能な SFEN を含める。
外部ソートで検出する重複には元の入力位置を保持しないため、検出時に処理していた行の位置を付けない。
既存保存先と開始後に作成された競合ファイルは上書きせず、いずれも `BookError::Io` の `ErrorKind::AlreadyExists` を返す。
公開前の失敗では自分の出力を公開せず、一時ファイルの削除を試みる。
公開後に同期などが失敗した場合は `BookError::SaveUncertain` を返し、完成した出力が存在し得る。
無条件再実行や自動削除を避け、再オープンと `validate_full()` などで内容を確認する。
内容の検証成功と、永続化成功の保証は区別する。

## Rust と Python の追加機能

新しい Native 変換設定、YBB の書き出し、DB2016 の逐次 writer、`Position::to_flipped()`、`NativeBook::get_with_flipped()`、`Position::clone_compact()` は Rust 向けである。
`clone_compact()` は全有効履歴と設定を保持し、余剰予約容量を引き継がない。
探索全体の高速化を保証する操作ではない。

Python には今回の Native 変換 API を追加しない。
既存の `NativeBookBuilder` には、上記の `FileExistsError` への例外型変更が適用される。
棋譜出力では、表現できない勝者や終局理由の組み合わせを `ValueError` で拒否する修正を含む。
各形式の制約は [KIF](../reference/formats/kif.md)、[KI2](../reference/formats/ki2.md)、[CSA](../reference/formats/csa.md)、[JKF](../reference/formats/jkf.md) を参照。
