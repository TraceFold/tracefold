# shell/ — 器(W-layer)

faceを1つも名指さず、宣言からdockとspaceを建て、逆を持つactだけでlayoutを変える。要件正本=`../req/02_SHELL_WLAYER.md`、裁定=`glovrex/req/38 §470`(器act/面actの二本立て)。

依存0(node組込+素のDOM)。frameworkもbundlerも使わない。

## Docker-IA chrome lane, 2026-08-24 (req/38 SS551/SS553/SS558, app req/97 Pass 2)

**What changed**: `.rail`は**space nounのみ**(verify/inspect、宣言(`SPACES`定数)由来、最大7=`Frame.RAIL_SPACE_CAPACITY`)に絞り、旧railが混ぜていたface quick-launchとtheme切替は新設の`.launcher`列(rail右隣、`kernel/manifest.mjs`の`rail:true`宣言を読む唯一の消費者=W14の「宣言に消費者が居る」を保つ)へ移した。全object view(docked face・staged tab)に**breadcrumb**(`space / dock-or-stage / face title`、素のtext、借り物symbol無し)と**copyable command block**(`kernel/command.mjs` `commandFor()`が組む`gx dock:go --index … --side … --at … --id …` / `gx tab:go …`——表示されている物を再現する実verb、36x36pxのcopyボタン付き)を追加。**bottom status bar**(`.strip`)にLIVE計測数値3枠を追加: suite(`.run/report.json`)・bench(`.bench/report.json`)・serve state——**fetch/XHR/WebSocketは使っていない**(W11 gateが禁止・一度実際にfetchで書いて赤を出し発見・修理した)。`tools/serve.mjs`が起動の度にディスクを読み`/.measures.gen.mjs`という一つのESモジュールとして返し、`demo/boot.mjs`は普通の`import`でそれを読む(ブラウザが全moduleを読むのと同じ経路・membraneの外に出ない)。値が届かない枠は`kernel/measures.mjs`の`NOT_WIRED`定数どおり正直に「not wired」と表示し、捏造した数字は出さない。

**5原則自己申告**: ①雛形=Docker Desktop型IA(rail=top-levelナビ・launcher/breadcrumb/commandは同型を全objectへ再利用)②軽量高速=`node tools/bench.mjs`実測済(shell mount median 0.2138ms、strip自身に表示)③英語+comment=全新規codeが英語④常時CRUD=このlaneはread/navigateのみ追加(既存act群は不変)⑤DB原則=strip数値はディスク上のJSONを都度読み直す(store=source、cacheしない)。

**検証**: `node --test`(shell)55→**67**(+12、`test/command.test.mjs`・`test/measures.test.mjs`新設)・全14 gate green(`node tools/gates.mjs`)・実Chrome window(`tools/rig/renderer.mjs`+`real_window.ps1`)で両テーマ撮影、4act(space:go/theme:set/command-copy/pane:divide)のinteraction pass実施——`.command-copy`は実clipboard permission dialogを本物のOS窓で発生させることを確認(headless CDPでは見えなかった実挙動)。**実窓検証で見つけた欠陥**: `demo/checks.mjs`の自動チェックが起動の度に`.command-copy`を自クリックしていた=実ユーザーの初回起動でOS clipboard許可dialogが毎回出る事故——checkから削除し、存在/サイズ検証のみ残した(押下確認は本README record配下の`shell_act_command-copy.png`が担う)。critic re-score=`req/97` Pass 2、shell/record/ verdict 1→**3**。

**明示的にやらなかった事**: breadcrumbは非活性(押せない)——critic Pass 2の worst defectとして記録済(`req/97`)。fetch経由での`.run/report.json`到達は`serve.mjs`の`ROOT`境界(shellツリーの外)を越える設計だったため撤回し、server-side注入に切替えた——この設計変更自体が本laneの中で見つかった1つの誤り。

## 走らせる

```
node tools/gen_manifest.mjs      # facesフォルダ → manifest.gen.mjs / modules.gen.mjs
node tools/gen_routes.mjs        # 膜のroute table → routes.gen.mjs
node --test "test/*.test.mjs"    # unit test
node tools/gates.mjs             # source側のgate(exit 1で落ちる)
node tools/negative.mjs          # 各gateに故意の欠陥を注入して赤を出させる
node tools/inverse_census.mjs    # 逆の分類と、historyが実際に保持している量
node tools/serve.mjs             # http://127.0.0.1:8788/ に実windowデモ
powershell -File tools/real_window.ps1 -Out record   # 実窓で開いて題名を読み写真を撮る
node tools/bench.mjs             # 2026-08-24 repair lane追加(app req/98 V-1): Mounted.raise/lowerのkernel dispatch msをmedian of 5で計測。実DOM mount msは別軸で未計測(file内で明記)
node --test test/command.test.mjs test/measures.test.mjs   # 2026-08-24 Docker-IA chrome lane追加: gxコマンド整形とstrip live-numbers整形の純関数test
```

## file

| file | 役 |
|---|---|
| `kernel/digest.mjs` | BLAKE3(公開vectorで検証)。自作checksumを置かない為だけに在る |
| `kernel/tree.mjs` | 分割木。破壊的変異をしない=逆が構造的に作れる |
| `kernel/layout.mjs` | 状態⇄1行。両方向が恒等。線に乗らない物は状態でない |
| `kernel/acts.mjs` | act registry(器track/面trackの二本立て・面trackは`invert: null`) |
| `kernel/state.mjs` | 唯一の道。`#current`はprivate fieldで、代入箇所は1つ |
| `kernel/manifest.mjs` | faceの宣言schema・dockの掟・rail gate |
| `kernel/slots.mjs` `kernel/dismiss.mjs` `kernel/keys.mjs` | 器が握る表(4表のうち3つ・4つ目=act registry) |
| `kernel/mount.mjs` | `mount(host, port, notices) → unmount`と、その回数 |
| `kernel/viewpoint.mjs` | 焦点/scroll/hover。状態でない(型で分離) |
| `kernel/marks.mjs` | 構造の記号。size必須・借り物0 |
| `kernel/render.mjs` | 差分描画。全面再構築をしない。2026-08-24: rail=space専用/launcher新設/breadcrumb+command block/strip live-numbers枠を追加 |
| `kernel/command.mjs` | 2026-08-24新設。dock/tabの現在表示を再現する`gx`コマンド文字列を組む純関数(`commandFor`)。DOM無し・test済 |
| `kernel/measures.mjs` | 2026-08-24新設。suite/bench/serve生データを strip表示文字列へ整形する純関数群。fetchしない・test済 |
| `kernel/shell.mjs` | 組み立て。face idのliteralが0件なのはここも同じ |
| `kernel/shell.css` | 色literal 0件。tokenは`/s-common/tokens.css`(正本を指すだけ・copyしない)。2026-08-24: `.launcher`/`.object-meta`/`.breadcrumb`/`.command`新設、body-text floorをvar(--t-record)=14pxへ統一 |
| `demo/` | 最小の実windowデモ。faceはplaceholder(本物は`req/03`)。`boot.mjs`が`/.measures.gen.mjs`をimportしstripへ渡す(2026-08-24) |
| `tools/serve.mjs` | 2026-08-24: `/.measures.gen.mjs`route追加——`.run/report.json`(app root)と`.bench/report.json`(このpackage)を都度読みESモジュール文字列として返す。fetchを使わせない設計 |
| `record/` | 実窓で載った証拠(題名・写真・face毎の記録)。2026-08-24: 両テーマ+4act interaction pass追加 |

1 fileずつimportできてtestできる。連結された断片ではない。
