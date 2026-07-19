# 参考資料

## コードベース・実装参考

- **YaneuraOu プロジェクト**: <https://github.com/yaneurao/YaneuraOu> - rsshogi が互換性や検証対象として参照する将棋エンジン
- **Stockfish**: <https://github.com/official-stockfish/Stockfish> - 探索エンジン実装やビットボード技術の一般的な参考

## 技術文献・Wiki

- **Chess Programming Wiki**: <https://www.chessprogramming.org/> - 将棋にも応用可能なビットボード・合法手生成技術

## やねうら王公式サイト（技術記事）

### 基礎技術
- [256手ルールの実装を間違えていた話](https://yaneuraou.yaneu.com/2021/01/13/incorrectly-implemented-the-256-moves-rule/) - 引き分け判定の落とし穴
- [将棋の完全ハッシュは何bitで表現できますか？](https://yaneuraou.yaneu.com/2015/12/29/%e5%b0%86%e6%a3%8b%e3%81%ae%e5%ae%8c%e5%85%a8%e3%83%8f%e3%83%83%e3%82%b7%e3%83%a5%e3%81%af%e4%bd%95bit%e3%81%a7%e8%a1%a8%e7%8f%be%e3%81%a7%e3%81%8d%e3%81%be%e3%81%99%e3%81%8b%ef%bc%9f/) - 局面圧縮の理論的限界
- [将棋の局面を256bitに圧縮するには？](https://yaneuraou.yaneu.com/2016/07/02/%E5%B0%86%E6%A3%8B%E3%81%AE%E5%B1%80%E9%9D%A2%E3%82%92256bit%E3%81%AB%E5%9C%A7%E7%B8%AE%E3%81%99%E3%82%8B%E3%81%AB%E3%81%AF%EF%BC%9F/) - 実用的な256bit圧縮
- [SFEN文字列は一意に定まらない件](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%89%e3%81%aa%e3%81%84%e4%bb%b6/) - SFEN持ち駒順序の問題
- [SFEN文字列は本来は一意に定まる件](https://yaneuraou.yaneu.com/2016/07/15/sfen%e6%96%87%e5%ad%97%e5%88%97%e3%81%af%e6%9c%ac%e6%9d%a5%e3%81%af%e4%b8%80%e6%84%8f%e3%81%ab%e5%ae%9a%e3%81%be%e3%82%8b%e4%bb%b6/) - SFEN正規化ルール

## フォーマット仕様

### 棋譜フォーマット
- [将棋の各種フォーマットについて](https://qiita.com/sunfish-shogi/items/964e139ef3bfd8f738d4) - KIF/CSA/SFEN/JKF等の包括的解説
- [CSA標準棋譜ファイル形式](http://www.computer-shogi.org/protocol/record_v22.html) - CSA 2.2仕様書
- [Kifu-for-JS](https://github.com/na2hiro/Kifu-for-JS) - JSON Kifu Format (JKF)

## 大会・コミュニティ

- [コンピュータ将棋協会](https://www.computer-shogi.org/) - WCSC等の大会ルール
- [floodgate](http://wdoor.c.u-tokyo.ac.jp/shogi/floodgate.html) - 24時間対局サーバー

## 本ドキュメント内の関連ページ

- [特殊ルール](internals/movegen/special-rules.md) - 千日手、打ち歩詰め、入玉宣言
- [局面の圧縮](internals/serialization/compression.md) - 256bit 圧縮技術
- [SFEN](internals/serialization/index.md) - SFEN 形式の詳細と実装
- [棋譜フォーマット](internals/serialization/formats.md) - KIF/CSA/KI2 の内部実装
