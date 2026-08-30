# membrane/ — 膜。app層で唯一networkに触るadapter 1 module(backend直上・不可視・0から)。→ req/01_MEMBRANE.md

構成: `route-table.json`(wire半分はcrateから機械抽出)・`coverage.json`(面の宣言)・`wire-fields.json`(NOT_DRAWNの母集団)・`src/`(実装本体・件数はディレクトリ一次)・`test/`(contract test・件数はディレクトリ一次)・`tools/`(抽出器・gate述語・実serve smoke)。この行の数値記載は鮮度gateの死角(req/104 §1①)——数を知りたければ手元で数え直せ。

```
node --test                                    # unit+contract 52本(2026-08-24実測)
bash tools/serve_bed.sh <gx> 8791 <log>        # WSLで実サーバを立てる
node tools/smoke_serve.mjs http://127.0.0.1:8791 <token>   # 実wireへの10 check
node tools/route_table_from_crate.mjs          # crateからroute抽出(件数はroute-table.jsonが正・drift検査はtest/route_table.test.mjs)
node tools/verdict_table_from_crate.mjs        # 2026-08-31 req/972 R-972-9追加: gx-core/src/verdict.rsのVerdictKindを抽出(drift検査はtest/verdict_table.test.mjs)。wire.mjs VERDICT_KINDSのみ対象・OUTCOMEはcrate由来でないため対象外(理由はtool内コメント)
node tools/bench.mjs                           # 2026-08-24 repair lane追加(app req/98 V-1): 1リクエストの膜内側(URL/header/idempotency/notice)msをmedian of 5で計測。実network msは別軸(smoke_serve.mjsが担当)
```

依存0(node組込のみ)・package.json無し(`.mjs`のみ)。

### 生成物の位置(2026-08-24 repair lane追加・app req/98 V-13)

- `route-table.json`: 生成物・source=crate。regen: `node tools/route_table_from_crate.mjs`(drift test: `test/route_table.test.mjs`)。
- `coverage.json`: **hand-maintained、非生成と宣言する**——面の宣言(どのrouteをどの面が使うか)は人が書く判断であり、crateから機械抽出できる性質のものではない。regenerateコマンドは無い(そもそもregenerate対象でない)。**ただし各faceの登録行自体は、そのfaceのcode一次(export)と`test/face_coverage.test.mjs`で機械照合される(req/967 §4-2・2026-08-30 terminalが最初のentry)**——手で書く判断は「どの面が使うか」であって「何を使っているか」の数え上げではない。
- `wire-fields.json`: **hand-maintained、非生成と宣言する**——NOT_DRAWNの母集団(意図的に描画しない配線)は人が書く判断。同じくregenerate対象でない。両fileとも変更時は`req/01_MEMBRANE.md`の該当宣言行と一致させる(一致検査は現状未実装=open)。
