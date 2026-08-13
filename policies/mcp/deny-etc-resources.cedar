// The shipped **MCP** pack (FR-028, AC-074), beside `policies/fs/deny-etc.cedar` and
// `policies/git/deny-nonbranch-refs.cedar`.
//
// 32 FR-028 逐語: 「gx-gateは出荷policy pack（既製invariant集）を`policies/`配下に同梱しなければなら
// ない（MUST）」, and 34 AC-074 is the git/MCP half: 「Given: git/MCP substrate向け出荷policy pack
// （`policies/{git,mcp}/`）。When: 各packのconformanceテストを実行する。Then: 各packが最低1 Admit
// ケース・1 Denyケースを持ち両方pass。」 The conformance cases are `crates/gx-gate/tests/ac_074.rs`;
// the road into the build is `crates/gx-gate/src/packs.rs`.
//
// Where this file lives: root, not `req/` -- req/38 §19's M3-16, the ruling that put the other two
// there.
//
// # 🔴 What a rule here may say is bounded twice, and the second bound is the sharp one
//
// **First**, by the locator `gx-adapter-mcp` writes: `<server endpoint>#<resource URI>`
// (`crates/gx-adapter-mcp/src/locator.rs`, `Position::locator`). The server carries a scheme
// (ASM-69-3's argument: a bare name is 「whichever server that name pointed at when this ran」), the
// resource carries no `#` of its own, and both parts arrive **normalised** by RFC 3986 §6.2.2 --
// which the next section is about.
//
// **Second, and this is the one that decides what ships**: 🔴 **the tool's name is not visible to a
// policy.** 42 §3.4 makes a delta's payload 「opaque な変更記述。core/gate/witness は byte 列としての
// み扱う(P-6)」, and this adapter's payload is where `tool` lives. So the rule a reader expects from
// an MCP pack -- 「forbid the `shell.exec` tool」 -- **cannot be written in v0.1**, not 「is not written
// yet」. Shipping a statement that looked like it constrained which tool runs would be M3-10's
// overclaim with a proxy's name on it, and req/99 §3's **D-4** by the other road: 「到達不能な定数を
// 宣言する事は…読み手が到達可能だと信じる定数になる」.
//
// What is left, and it is enough for one honest rule, is **where the call lands**: the resource URI.
//
// # What ships: two statements
//
//   1. `mcp-permit-default` -- Cedar is default-deny (「no request is authorized (decision Allow)
//      unless there is a specific permit policy that grants it」,
//      https://docs.cedarpolicy.com/auth/authorization.html, read 2026-08-08 and recorded in M3), so
//      without a permit an MCP deployment admits nothing at all. Scoped to the mcp substrate, which
//      is what keeps an fs or git change evaluated against this set alone falling to Cedar's third
//      rule rather than being quietly admitted here.
//
//   2. `mcp-deny-etc-resources` -- a forbid over `file:` resources at or under `/etc`.
//
// # 🔴 Why `/etc`, on **this** substrate
//
// It is the fs pack's rule, written on the substrate that would otherwise be the road around it.
// `policies/fs/deny-etc.cedar` refuses a change to `/etc/passwd` on the fs substrate; the same file,
// reached by asking an MCP server to write it, is a **different `SubstrateKind`, a different locator
// and a different policy subject**, and nothing in the fs pack is in force on it. AC-051's whole
// claim about this adapter is 「バイパス手段が技術的に存在しない」 at the *call* boundary; that claim is
// about roads to the substrate and says nothing about a shipped **policy** having a hole shaped like
// a second substrate. This statement is the policy-layer half of the same sentence.
//
// It is reachable: `file:` is MCP's own commonest resource scheme (the protocol's own examples read
// `file:///…`), the adapter's `parse` accepts it, and `crates/gx-gate/tests/ac_074.rs::
// every_case_names_a_locator_the_mcp_adapter_could_produce` keeps that boundary measured rather than
// remembered.
//
// 🔴 **What makes the rule reach a spelling nobody wrote it for, and what still evades it.**
// `resource.locator` is compared as a string and Cedar's `like` cannot fold a `..`, so
// `file:///srv/../etc/passwd` matches this forbid **only because `gx-adapter-mcp` has already folded
// it** (RFC 3986 §6.2.2: dot segments removed, unreserved percent-triplets decoded) before the
// locator reaches a gate. That is H-2's requirement of an adapter (「locator は正規化済みで gate に
// 渡す」, req/38 §25) and `crates/gx-adapter-mcp/tests/h2_normalised_before_the_gate.rs` measures it
// end to end, including the half that shows **the gate normalises nothing** -- the same fact
// `crates/gx-gate/tests/false_admit.rs::the_pack_judges_the_locator_as_given_not_as_resolved` pins
// on the fs substrate, which is a statement about the gate rather than about either pack (the fs
// adapter folds `..` too; E-M4-12 makes it a contract for every adapter).
//
// What is **not** folded is what this pack cannot reach: RFC 3986 §6.2.3 and §6.2.4 are deliberately
// not performed (`crates/gx-adapter-mcp/src/lib.rs` argues why -- a normaliser that merged two
// endpoints a deployment distinguishes is worse than one that splits them), so a server reachable
// under two spellings is two policy subjects here. And nothing lexical can close a **server-side**
// alias: if a server maps `file:///srv/notes.md` onto `/etc/passwd`, this rule never sees it. That is
// the MCP analogue of 45 TH-2's symlink residue, and it is one of the reasons the crate root says
// what the membrane offers against effects outside the object is serialisation rather than detection.
//
// # Why the node itself, as well as everything under it
//
// `like "*#file:///etc/*"` does not match the directory node, and hand 5 of M3 settled the same
// question for the fs pack (req/65 §3): a pack that forbade every file under /etc while permitting a
// change to /etc would refuse `/etc/passwd` and admit the change that replaces the directory holding
// it -- protection with a door beside it. So the disjunct is here for the same reason it is there.
//
// # What is **not** here
//
// A rule about the **server**. Refusing a scheme -- `stdio://`, say -- would refuse the ordinary MCP
// deployment (most servers are local processes) and therefore the wedge case itself, which is the
// mistake the git pack refused to make about `refs/heads/main`. A deployment that trusts one server
// and not another writes one statement of its own: a shipped pack is a starting point, not a policy
// (35 RSK-5 「Policy authorship cold start」, and 45 §87's 「緩和するが解消しない」).
//
// A rule about **evidence or order**. Both are in M3-10's range and both are deployment questions,
// not defaults: `context.order == 2` is one clause anybody can write, and D-9's reasoning (a shipped
// Rust body would be a second road to a rule a policy already states) is unchanged here.
//
// `@id` on every statement is a format requirement rather than decoration (**ASM-62-1** / C-4): a set
// without it is refused at load, because `PolicySet::from_str` names policies by position, that id
// lands in `PolicyDecisionRecord`, and 42 §1.3 puts the record inside `AdmitProof`'s IdentityView --
// so swapping two statements in this file would change what a receipt says without changing what was
// decided.

@id("mcp-permit-default")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "mcp" };

// `like` is the only string matcher Cedar has and it works on strings rather than on entity
// references (https://docs.cedarpolicy.com/policies/syntax-operators.html: 「Binary operator that
// evaluates to true if the string in the left operand matches the pattern string in the right
// operand … one or more asterisks (*) as wildcard characters」). The locator arrives as an attribute
// of the resource entity for exactly that reason -- see `RequestView` in `crates/gx-gate/src/policy.rs`
// for the whole mapping.
//
// The patterns anchor on `#`, the separator between the server endpoint and the resource, and here
// the anchor is stronger than the git pack's: `parse` refuses a resource URI carrying a `#` of its
// own, so a locator this adapter produces holds **exactly one** `#` and `*#file:///etc/` can only
// mean 「the resource begins `file:///etc/`」. Without the anchor, a server endpoint that happened to
// spell `file:///etc` in a path or a query would drag every call on that server into the forbid;
// `crates/gx-gate/tests/ac_074.rs` carries that case rather than leaving the anchoring to be
// believed.
@id("mcp-deny-etc-resources")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && (resource.locator like "*#file:///etc/*" || resource.locator like "*#file:///etc")
};
