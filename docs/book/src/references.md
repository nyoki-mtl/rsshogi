# 参考資料

`rsshogi` が互換性の基準として参照している外部実装、フォーマット仕様書、および将棋プログラミングの技術資料をまとめる。

## 参照実装と関連プロジェクト

- **YaneuraOu（やねうら王）**：<https://github.com/yaneurao/YaneuraOu>
  `rsshogi` が `Move`（16-bit）のビット配置、`PackedSfen`、`DB2016` / `YBB` 定跡、PACK 形式などの互換対象として参照する USI 将棋エンジン。
- **Stockfish**：<https://github.com/official-stockfish/Stockfish>
  `StateInfo` による差分状態管理やビットボード演算など、探索エンジン基盤の設計における参考実装。
- **Chess Programming Wiki**：<https://www.chessprogramming.org/>
  ビットボード、Zobrist ハッシング、段階的指し手生成などのアルゴリズム解説。

## やねうら王開発ブログ（技術記事）

- [将棋の完全ハッシュは何bitで表現できますか？](https://yaneuraou.yaneu.com/2015/12/29/%e5%b0%86%e6%a3%8b%e3%81%ae%e5%ae%8c%e5%85%a8%e3%83%8f%e3%83%83%e3%82%b7%e3%83%a5%e3%81%af%e4%bd%95bit%e3%81%a7%e8%a1%a8%e7%8f%be%e3%81%a7%e3%81%8d%e3%81%be%e3%81%99%e3%81%8b%ef%bc%9f/)：局面圧縮の理論的下限に関する考察。
- [将棋の局面を256bitに圧縮するには？](https://yaneuraou.yaneu.com/2016/07/02/%E5%B0%86%E6%A3%8B%E3%81%AE%E5%B1%80%E9%9D%A2%E3%82%92256bit%E3%81%AB%E5%9C%A7%E7%B8%AE%E3%81%99%E3%82%8B%E3%81%AB%E3%81%AF%EF%BC%9F/)：`PackedSfen` および `HuffmanCodedPos` の 256-bit 符号化方式。
- [SFEN文字列は一意に定まらない件](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%89%e3%81%aa%e3%81%84%e4%bb%b6/) / [SFEN文字列は本来は一意に定まる件](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e6%9c%ac%e6%9d%a5%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%8b%e4%bb%b6/)：SFEN における持ち駒の正規化順序。
- [256手ルールの実装を間違えていた話](https://yaneuraou.yaneu.com/2021/01/13/incorrectly-implemented-the-256-moves-rule/)：最大手数引き分け判定と `game_ply` の境界条件。

## 棋譜とプロトコルの仕様書

- **CSA 標準棋譜ファイル形式（V2.2）**：<https://www.computer-shogi.org/protocol/record_v22.html>
- **JSON Kifu Format（JKF）**：<https://github.com/na2hiro/Kifu-for-JS>
- **将棋の各種フォーマットについて（Qiita）**：<https://qiita.com/sunfish-shogi/items/964e139ef3bfd8f738d4>
- **コンピュータ将棋協会（CSA）**：<https://www.computer-shogi.org/>
