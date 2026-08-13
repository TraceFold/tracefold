# Changelog

**Status**: 新設 v0.2.4・2026-08-13（`req/38` §68 #8 裁定＝`req/111` §要裁定 **B-7** 採）。

## 0. この file は何のために在るか — 2 つの条文がここを名指している

| 条文 | 逐語 | 本 file の役割 |
|---|---|---|
| 33 **NFR-024** | 「pre-1.0期間中のsemverポリシー: `0.y.z`間はminorバンプで破壊的変更を許容するが、**CHANGELOGへの明記を必須とする**。1.0以降は厳格semverへ移行する」 | 破壊的変更は禁じられていない。**黙って変える事**が禁じられている。その「明記」の置き場 |
| 47 **§4** | 「**journal schemaは`gx replay`による決定的リプレイが新旧バイナリ間で一致することをアップグレード前検証の条件とする**」 | 42 §3.13 の `EngineJournalRecord` の形が変われば既存 journal file が読めなくなる。**どの窓で形が変わったか**を読み手が知る場所 |

🔴 **この file が無かった間、47 §4 の条件は「宣言されているが enforce する物が無い」状態だった**（`req/110` §1 NFR-024・§2-④-g「CHANGELOG.md 不存在」）。本 file はその空白を埋めるが、**gate を作った訳ではない**——「release PR テンプレートに CHANGELOG 必須項目のチェックリストを設置し、レビューで強制」（NFR-024 の測定方法欄）はまだ存在しない。**器が在る事と、機械が見張っている事は別である。**

## 1. 🔴 まず正直に — ここに書いてあるのは release ではない

- **crates.io へ publish された物は 1 つも無い**。workspace の全 crate は `version = "0.1.0"` のままであり、`gx-substrate-conformance` と `probes/doubt` は `publish` 対象外を自分の manifest に書いている。
- **署名済み release artefact も無い**（47 §1(b)・33 NFR-013 の SLSA provenance は未着手）。
- ∴ **下の表の `req0.0x` は「公開版」ではなく、requirement lane（`req/38`）が検収 PASS を出した時点に打った内部 marker である**。値は `git tag -l` と各 tag の日付から機械で取った（`git tag -l --sort=creatordate --format='%(refname:short)|%(creatordate:short)'`）。要約文は各 tag の subject の転写であって、本 file が書き足した評価ではない。
- **semver としての意味を持つ最初の行は、まだ書かれていない。** 1.0 以降の厳格 semver への移行（NFR-024）も同様である。

## 2. Unreleased

- **v0.2.4 spec 批**（2026-08-13・`req/38` §68 割当 10 項）: 42 §3.13 の残 6 record 差を実体へ同期（doc のみ・**journal schema の形は 1 mm も動いていない**——動いたのは canon の側であり、`crates/gx-engine/src/store.rs` の `EngineJournalRecord` は 1 byte も変わっていない）／42 §0 の型帰属訂正／41 §2 crate tree 補完／33 NFR-018 の予定形解消／47 §2 の T1〜T4 定義／35 §E・§F の裁定転記／本 file の新設。
  - 🔴 **区別して読む**: 「42 §3.13 が変わった」は **canon が実装に追いついた**の意味であって、**journal file の互換性は 1 度も影響を受けていない**。47 §4 が条件とするのは実装の schema であり、canon の綴りではない。

## 3. Journal schema（47 §4 の対象）の変更履歴

**`gx replay` の決定的リプレイが新旧バイナリ間で一致する事**が upgrade の前提条件である。∴ `EngineJournalRecord` の形が変わった窓だけをここに列挙する。読み手が「どの形で書かれた directory か」を知る場所は `.gx/VERSION`（`req/56` §2・`crates/gx-cli/src/layout.rs` の `LAYOUT_VERSION`）である。

| 日付 | 窓 | 何が変わったか | 互換性 | 一次 |
|---|---|---|---|---|
| 2026-08-10 | **M6 手1**（tag `req0.08` に含まれる・commit `f763a5b`） | **E-M5-13**: `Planned` record に `locator: String` と `parents: Vec<TransformationId>` を追加 | 🔴 **破壊的**。この commit より前に書かれた journal file は読めない | `req/88` **M6-14** 採(a)・`req/38` §47・`crates/gx-engine/src/store.rs` の `Planned` variant |

🔴 **なぜ M6 手1 が窓だったのか（cost ではなく窓）**——`req/88` M6-14 逐語: 「**M6 は最初の配布物を作る手である**——配布前に破れば代償は 0、配布後に破れば全 user の journal が対象」。**配布物がまだ誰の手にも無いので、この破壊的変更の実被害は 0 である**。それは幸運であって規律ではないので、ここに記録する。

**この表に無い変更は「無かった」の意味である**——`EngineJournalRecord` に触れた他の erratum（E-M5-1 の `ApplyStarted` 新設、M5-25 採(a) の `ProvenanceDerived` 新設、E-M5-9 の `Option` 化、M5H4-2 の `rollback`、M5H6-2 の `reason`/`actor`）は**すべて M5 の中**で、journal を持つ利用者が 1 人も存在しない時期に起きた。∴ 「破った」と書くべき対象が無い。**その事実自体を書いておかないと、次に読む者は「記録漏れ」と「起きていない」を区別できない。**

## 4. Milestone tag の履歴（`git tag -l` からの機械転写）

| tag | 日付 | 種別 | subject（転写） |
|---|---|---|---|
| `req0.00` | 2026-08-06 | annotated | baseline: spec 25 doc + probes 32 green + lake build RC=0 (2026-08-06) |
| `req0.01` | 2026-08-06 | annotated | scaffold: 36 probes green (typed parser), e2e floor, semantics map — hostile-audited (B1-B5 fixed, verify PASS) |
| `req0.02` | 2026-08-07 | lightweight | req(08): V§10 NEW-B1/M1検収PASS——req0.02成立裁定(spot e2e RC=0 GREEN 37/5・判別pass確認) |
| `req0.03` | 2026-08-07 | lightweight | req(48): req0.03 pipeline完遂——手6b両半分GREEN(153/33判別つき+lake)・手6c RUNTIME実測(encode522/decode394/cid601ns median N=1000)・tag発行裁定 |
| `req0.04` | 2026-08-08 | annotated | M2 complete: gx-log merkle ledger + gx-witness DSSE receipts. Floor 370/62, AC 12/12, mutation 90.9%, Kani 3/3, fuzz 3 targets crash 0, primaries byte-checked. T4/T5 not yet proven (M8). |
| `req0.05` | 2026-08-09 | lightweight | req(38): §27 M3 fix批検収PASS+J-1..7裁定+tag req0.05成立宣言(偽PASS 6本全捕捉・coverage 5/5・床511/83) |
| `req0.06` | 2026-08-09 | lightweight | req(38): §36 M4 fix批検収PASS+L-1〜L-7裁定+tag req0.06成立宣言(coverage 8/8 PASS・battery 11本独立再走・規律46/47・gotcha45) |
| `req0.07` | 2026-08-10 | annotated | M5 complete: gx-engine state machine (21/21 transitions), all 22 ACs, floor 968/170, E-M5-1..16, crash recovery proven, decay identified, mutants+coverage measured. req/38 §37-§46. |
| `req0.08` | 2026-08-11 | lightweight | req(38): §56 M6 fix批検収PASS(凍結計器独立再走: 床1211/221一致・battery13/13 RED・ci GREEN/clone e2e/semantics5/5)+M6FIX-1追認/2はM7 FR化/6 shard禁+規律54(UC三分法)制定+Λ7 routing(req/97§4正本+axiom independence註記)+M6FIX-7起票+tag req0.08 |
| `req0.09` | 2026-08-13 | lightweight | req(38): §65 fix批検収PASS+B-1〜B-11裁定+req/103引用訂正+M7完遂=tag req0.09 |

🔴 **`req0.00` の subject が「spec 25 doc」と綴っている事について**: 当時の実数であり、その後 canon は 28 doc になった（`req/semantics.json` の `spec.canon` title は v0.2.4 §68 #6 で実測へ訂正した）。**転写であって現在の主張ではない**ので、tag の文言は直さない——tag は打った時点の記録である。

🔴 **`req0.02`/`req0.03`/`req0.05`/`req0.06`/`req0.08`/`req0.09` は lightweight tag（commit への別名）であり、`req0.00`/`req0.01`/`req0.04`/`req0.07` は annotated tag（tag object を持つ）**。混在は意図された物ではなく経緯であり、**上の「日付」列は annotated なら tag object の日付、lightweight なら指す commit の日付**である（`%(creatordate:short)` が両者を同じ列で返す）。数を並べる以上、それが何の日付かを書く。

## 5. この file を次に書き足す手への規約

1. **journal schema を変えたら §3 に 1 行**。commit hash と、その形より前の journal が読めなくなる事を明記する（47 §4）。
2. **`0.y.z` の minor を上げる破壊的変更は §2 か新しい版の節に 1 行**（NFR-024）。「何を破ったか」を書く。「改善した」ではない。
3. **publish していない物を publish したように書かない**。§1 の但し書きは、その状態が変わるまで消さない。
4. **数は機械から取る**（`git tag -l`・`git log`・実 manifest）。要約文は転写であって評価ではない。
