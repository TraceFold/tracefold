// The pack AC-025 names by path. 34 AC-025 逐語: 「Given: Cedar PolicySet(`policies/fs/deny-etc.cedar`:
// `/etc/**`への書き込みを拒否)。When: `/etc/passwd`への書き込みTransformationを評価。Then: Cedar評価結果が
// Deny相当に写像される。`/tmp/x`へは Admit相当。」
//
// Where this file lives: root, not `req/`. req/38 §19's M3-16 ruling -- 「`policies/` は **root 直下**
// (AC-025 の逐語 path 優先・出荷物は req/ 下に置かない)」.
//
// What it is: **the shipped fs pack** (FR-028), as of hand 5. Hand 2 wrote this file as the smallest
// policy set AC-025 could be stated against, and its header said 「what it is not: the shipped pack …
// that is hand 5」. Hand 5 promoted it, and promoting it meant three things and no more: the file is
// embedded into the build by one road (`crates/gx-gate/src/packs.rs`), the conformance cases FR-028
// asks for exist (`crates/gx-gate/tests/ac_028.rs`: 「最低1 Admitケース・1 Denyケース」), and the
// forbid below was widened to cover the directory node itself (see the comment above it). The
// statements are still written to exercise both resource attributes the ASM-60-1 mapping supplies, so
// that dropping either one from the mapping turns AC-025 red rather than silently changing which
// policy answers.
//
// What it is not: a pack of ready-made invariants. FR-028 glosses a pack as 「既製invariant集」 and
// what ships here is policies -- because a rule an invariant could add at M3's visibility is a rule a
// policy already states, and the ones worth writing in Rust (「行数保存」-shaped) need the payload,
// which P-6 keeps opaque until M4's facts path. That is **D-9**'s decision; the three reasons are in
// `crates/gx-gate/src/packs.rs` and `req/65` §3, and `req/65` §4 raises the M4 re-reading so it is
// scheduled rather than remembered.
//
// What a pack can reason over at all -- the locator, the actor's key, the change context, the order,
// the inverse flag and the evidence summary, and **not** the change itself -- is M3-10's ruled range,
// written out in that module's doc. Read it before writing a policy here: 「行数を保存せよ」 and
// 「agent の変更には追加 evidence を要求せよ」 are both unwritable in v0.1, and learning that from a
// silent Allow is the failure the range exists to prevent.
//
// Two things about Cedar that decide the shape of this file, both from the primary
// (https://docs.cedarpolicy.com/auth/authorization.html, read 2026-08-08):
//
//   1. 「default deny: no request is authorized (decision Allow) unless there is a specific permit
//      policy that grants it; by default, the decision is Deny.」 So the second half of AC-025
//      (`/tmp/x` → Admit) is unreachable without a permit. The permit below is what makes an fs
//      transformation decidable at all; it is scoped to the fs substrate because this is the fs pack,
//      which means a git or MCP transformation evaluated against this set alone reaches Cedar's
//      third rule (nothing satisfied → Deny) rather than being quietly allowed by an fs policy.
//
//   2. 「forbid overrides permit」. The forbid below therefore needs no exception carved into the
//      permit: the two are independent statements and the deny wins wherever both apply.
//
// The `@id` annotations are not decoration. 「@id is not special in the Cedar language, it behaves
// like any other annotation. The Cedar CLI uses the @id annotation to set policy IDs」
// (https://docs.cedarpolicy.com/policies/syntax-policy.html) -- and `PolicySet::from_str` otherwise
// names policies `policy0`, `policy1`, ... by position. gx records the deciding policy's id in
// `PolicyDecisionRecord` and 42 §1.3 puts that record inside `AdmitProof`'s IdentityView, so a
// positional id would make a receipt's CID depend on the order the statements happen to be written
// in. `PolicyEngine::parse` refuses a policy that carries no `@id` for that reason.
//
// `like` is the only string matcher Cedar has, and it works on strings rather than on entity
// references (https://docs.cedarpolicy.com/policies/syntax-operators.html: 「Binary operator that
// evaluates to true if the string in the left operand matches the pattern string in the right
// operand … one or more asterisks (*) as wildcard characters」). That is why the locator arrives as
// an attribute of the resource entity and not only inside its id -- see `RequestView` in
// `crates/gx-gate/src/policy.rs` for the whole table.

@id("fs-permit-default")
permit (
    principal,
    action,
    resource
)
when { resource.substrate == "fs" };

// `/etc/**` as a Cedar pattern. `"/etc/passwd" like "/etc/*"` is true and `"/tmp/x" like "/etc/*"`
// is false, which is exactly the pair AC-025 asks about. `/etc` itself -- the directory node, with
// nothing after it -- does not match that pattern, and neither `/etc/**` nor AC-025 says it should:
// the glob names what is under the directory. Hand 2 left the question 「whether the shipped pack
// should also refuse a change to the node itself」 to hand 5, and hand 5 answers yes, which is the
// `==` disjunct below.
//
// The reason is what the two halves protect against together. A pack that forbade every file under
// /etc while permitting a change to /etc would refuse `/etc/passwd` and admit the transformation
// that replaces the directory holding it -- protection with a door beside it. AC-025 stays exactly
// as true as it was (both of its cases are unchanged), and `crates/gx-gate/tests/ac_028.rs` carries
// the node as its own conformance case so that the disjunct is a measured rule rather than a
// widening nobody checks. `tools/verify_m3h5.sh` §10 removes it and shows that case, and only that
// case, go red.
@id("fs-deny-etc")
forbid (
    principal,
    action,
    resource
)
when {
    resource.substrate == "fs"
    && (resource.locator == "/etc" || resource.locator like "/etc/*")
};
