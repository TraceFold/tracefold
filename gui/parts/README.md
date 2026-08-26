# parts/ — 小部品。C=描く(判断しない)/M=決める(DOMを触らない)の2種のみ・種を跨がない(全て0から)。→ req/04_PARTS_REBUILD.md

> **status**: 実装着地(2026-08-24)。`node --test "test/*.test.mjs"` = **148 pass / 0 fail / exit 0**(2026-08-24 repair lane訂正・app req/98 V-9: 旧「139」は自身の宣言式`for f in test/*.test.mjs; do grep -c '^test(' $f; done`を再実行した実測値と不一致だった=`tokens-generated.test.mjs`の6件が表から漏れ+`receipt-row.test.mjs`が15→18に増えていた事に未追従。数字は再生成、手打ちしない)。
> **由来**: 全file 0から(`§461`/`§24b`)。**COPY HARD BAN 準拠・外部OSS参照 0件**(§24d: 参考にした外部OSS pattern=無し。SVG sprite・`<details>`・WCAG相対輝度は W3C 仕様/標準要素であって OSS 実装ではない)。
> **旧BASE**: 観察のみ(`req/04a`)。code は1行も持ち込んでいない。

## 台帳(1部品=1行)

| # | 部品 | 種 | src | test | 単体test数 | 実render判定 |
|---|---|---|---|---|---|---|
| — | element(共通基盤=唯一のelement door) | C | `src/element.mjs` | `test/element.test.mjs` | 12 | 全fixture経由 |
| C1 | token(消費契約・新規作成しない) | C | `src/tokens.mjs` | `test/tokens.test.mjs` | 12 | `shots/tokens_narrow.png` |
| C2 | glyph-sheet | C | `src/glyph-sheet.mjs` | `test/glyph-sheet.test.mjs` | 14 | `shots/glyph-sheet_{narrow,dark}.png` |
| C3 | verdict-badge | C | `src/verdict-badge.mjs` | `test/verdict-badge.test.mjs` | 8 | `shots/verdict-badge_narrow.png` |
| C4 | receipt-row(+note) | C | `src/receipt-row.mjs` | `test/receipt-row.test.mjs` | 18 | `shots/receipt-row_{narrow,wide,dark}.png` |
| C5 | provenance-fold | C | `src/provenance-fold.mjs` | `test/provenance-fold.test.mjs` | 10 | `shots/provenance-fold_narrow.png` |
| C6 | serial | C | `src/serial.mjs` | `test/serial.test.mjs` | 10 | `shots/serial_narrow.png` |
| M1 | seal-claim | M | `src/seal-claim.mjs` | `test/seal-claim.test.mjs` | 10 | 該当なし(DOM接触0) |
| M2 | row-order | M | `src/row-order.mjs` | `test/row-order.test.mjs` | 15 | 該当なし(DOM接触0) |
| M3 | checkable | M | `src/checkable.mjs` | `test/checkable.test.mjs` | 13 | 該当なし(DOM接触0) |
| — | tokens.generated(自動生成・regen: `node tools/generate-tokens.mjs`) | 生成物 | `generated/tokens.generated.mjs` | `test/tokens-generated.test.mjs` | 6 | SHA-256 drift testで正当性を守る(shotなし・値testのみ) |
| — | AC-P0 境界gate | — | `tools/boundary.mjs` | `test/boundary.test.mjs` | 20 | — |

合計 **148**(機械count: `for f in test/*.test.mjs; do grep -c '^test(' $f; done` の総和 == `node --test` の tests 148。2026-08-24時点で再生成・以後この数字が動いたら本fileでなくこのコマンドを再実行して直す)。

`m4-state-inverse` は骨格へ昇格済(`req/02 §3`)につき本dirの対象外。

## 機械gate(全て負対照で1度以上赤を出してある)

`tools/boundary.mjs` / `test/boundary.test.mjs`。code面(comment除去後)とbyte面(全文)を分けて走る。

| gate | 内容 | 実測 |
|---|---|---|
| 母集団非空 | C≥1 かつ M≥1(規則が空集合に効くのを防ぐ) | C=7 / M=3 |
| M→DOM 0件 | 決める部品が document に届かない | 0(temp dirへ実file植えて赤を確認) |
| C→verdict分岐 0件 | 描く部品が verdict 値で経路を変えない | 0(`===`/`!==`/`case`/`switch` の4形で赤) |
| 色literal 0件 | `#hex`/`rgb()`/`hsl()` が出荷sourceに無い | 0 |
| RED 1点 | `CONSUMED.deny` を使うsiteの数 | **1**(`verdict-badge.mjs`のみ) |
| 非ASCII 0件 | 借り物記号(`●◆◇◈▾▴★■⏺`)+混入script | 0 |
| 手打ち分母 0件 | `sixty-four` 型のhardcode | 0(comment内の説明は除外・除外が盲にならない事も赤で確認) |
| `var(--ink-3)` 0件 | 実測4.51:1(light)で床すれすれ→text不使用 | 0 |
| element door 1つ | `element.mjs` 以外が element を組まない | 0 |

## 実描画で判明し、DOM計測では捕まらなかった欠陥(§24c の実証)

🔴 **初回shootで全glyphが黒塗り**になっていた。`<use>` の shadow content は **sprite側でなく instance 側の継承鎖**を読むため、`fill="none"`/`stroke` を sprite root にだけ書いていたのが原因(ring→黒円、破線枠→黒塊)。**overlap=0 / oversize=0 / 正しい寸法 / 正しい個数** という計測はすべて緑のままだった。修正=`STROKE` を instance にも書く(`glyph()`)。再発防止=①`test/glyph-sheet.test.mjs` の instance stroke assertion ②`tools/shoot.mjs` の `filledGlyphs`(実render後の computed `fill`/`stroke` を読む)。
**教訓の追認**: 「DOM rect正常≠実描画正常」は寸法だけの話ではない——**寸法も位置も重なりも全部正しいまま、描画そのものが別物**になり得る。

## 目視判定(自分でscreenshotを見た結果・9枚撮影/8枚目視)

- **AC-F1(N-1)= 閉じた**: note は行の**下にflowで**入り、下の行を押し下げる。720/1280 の両幅・light/dark の両themeで**text衝突0**を目視。`position:absolute` を部品tree全体で0件にし、gateが植えた absolute で赤を出す。
- **AC-F2(N-2 重なり)= 該当構造が無い**: 二重描画経路(2つ目のelement builder)を gate で0件化。sprite は1 page 1枚(`sprites=1` 実測)。
- **AC-F3(全glyph明示サイズ)= 閉じた**: size 省略で **throw**(7形で赤)。size は属性と inline style の二重書き。実render で `oversize=0`。
- **T-5(RED 1点)= 目視でも1点**: 全fixtureを通して赤いのは `Deny` のみ。
- **C6**: 同じcodeが16桁digestに「out of 16」、64桁に「out of 64」と書くのを1枚で並べて確認。

## `[]` 残件(緑にしていない)

- `[ ]` **light既定と「themeをhtmlにnailしない」(T-6)は現状の正本と両立しない**。正本 `tokens.css` は bare `:root` が dark、light は `[data-theme="light"]` か OS 選好でのみ来る。∴ 何も nail しなければ既定は**OS任せ**であり light ではない(初回shootが全部darkで出たのが実証)。shootは `Emulation.setEmulatedMedia` で light/dark を明示して撮っている=「既定がlight」を証明した訳ではない。**器(`req/02`)側の裁定が要る**。
- `[ ]` **視覚的近接2組**(機械gateでは捕まらない): ①14px の `Admit`/`Deny`(22pxでは明確に分離・実rowでは常に凍結語が併走するので実害は低い) ②`structure/hole` と undefined mark(共に破線囲み)。意味idは別なので `meaningCollisions` は0のまま——**形の近さは機械が見ていない**。
- `[ ]` `<details>` の開閉三角は**UA提供のmarker**であり自作sheet由来ではない(G-1の対象外と判断したが、画面上に非bespokeな印が1つ出ているのは事実として記録)。
- `[◐]` **bench追加(2026-08-24 repair lane, app req/98 V-1)**。`node tools/bench.mjs` = `row()`(`src/receipt-row.mjs`)の1,000行tree構築msをmedian of 5で計測・`.bench/report.json`へ永続化・budget超過はhard-red exit。**paint(実DOM描画)は依然未計測**——tree構築msと実描画msは別軸(このfile内で明記)。
- `[ ]` `receipt-row` の note が行に対して**視覚的にindentされていない**(衝突0は達成、帰属の見せ方は未詰め)。
- `[ ]` `installSheet()` の「1回だけ」は fixture の `sprites=1` 実測で支えており、**2回呼んだ時の挙動は実windowで未発火**。
- `[ ]` **red-first の順序は厳密には守れていない**。gateの負対照は全て実際に赤を出してあるが、happy path の assertion は実装後に書いた。
- `[ ]` 独立再走 `[●]` = **0件**(全て本laneの自己申告=`[◐]`)。

## 生成物の再生成コマンド(2026-08-24 repair lane追加・app req/98 V-13)

- `generated/tokens.generated.mjs`: `node tools/generate-tokens.mjs`(SHA-256 drift testが不整合を赤にする)。
- `fixtures/*.html`: `node tools/fixtures.mjs`(部品を実HTML fixtureへ書き出す・source=`src/*.mjs`)。
- `fixtures/shots/*.png`: `node tools/shoot.mjs`(headless Chromeで`fixtures/*.html`を撮影・source=fixtureそのもの)。両者ともstore=sourceでなくderived(捨てて作り直せる)。

## 見ていない範囲(分母)

- 旧BASE の部品source: 本laneは**1行も読んでいない**(観察は `req/04a` が済ませており、本laneの入力はその観察表とAC)。
- `glovrex_web/req/component/*.req.md` 10本 = 0行。
- Z-3(参照真値のtest PASS数と同数以上)は**測れていない**——`req/05` に部品ごとのPASS数が無く、分母が存在しない。本laneの139は**自分の分母**であって同値性の証明ではない。
- 実tauri窓での確認 = 0件(headless Chrome のみ)。
- `faces/`・`shell/` への接続 = 未(部品は誰にも消費されていない)。
