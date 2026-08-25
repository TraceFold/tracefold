// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! Which tools can be undone, and by what.
//!
//! 41 §4 requires `invert` to answer "the delta that undoes this one" or `Ok(None)`, and DR-1(a) is why (sem: SEM-gx-adapter-mcp-113)
//! the whole wedge exists. For a filesystem the inverse is derivable -- write the old bytes back. For a
//! git repository it is derivable -- put the reference where it was. **For a tool call it is not
//! derivable at all**: what a tool does is the server's, and nothing in the MCP protocol says which
//! other tool undoes it, or whether one exists.
//!
//! So the fact has to be **declared**, and by the only party that knows it: whoever runs the server.
//! This type is that declaration, and it is the deployment's input the way a policy pack is.
//!
//! # 🔴 What this is not: a second gate
//!
//! A catalogue does **not** decide which tools may be called. That is the gate's judgement and 52
//! contract 2 forbids inventing a second: a tool this catalogue has never heard of is planned, carried and (sem: SEM-gx-adapter-mcp-114)
//! (if a policy admits it) called, exactly like one it knows. What the catalogue changes is one
//! question -- "can this be undone?" -- and the answer flows to the gate through (sem: SEM-gx-adapter-mcp-115)
//! `GateInput.invert_available` (**E-M4-5**), which is where 41 §5 already puts it.
//!
//! An unknown tool is therefore a tool whose calls are **irreversible as far as gx knows**, and
//! **E-M3-4** escalates a change with no inverse to a person. That is the conservative direction and it
//! is the one an empty catalogue takes.
//!
//! # 🔴 Two restore forms, because real servers are not shaped like the demo (req/38 §92, ruling 1) (sem: SEM-gx-adapter-mcp-116)
//!
//! v0.1's one form hands every restore tool canonical DAG-CBOR of `{contents, uri}` -- MCP's own
//! `resources/read` shape, right for a tool whose whole job is "write these bytes back at this uri" (sem: SEM-gx-adapter-mcp-117)
//! (the fs/notes demo, req/136's mock saas). A1-1 measured against the first **real** target
//! (github/github-mcp-server v1.9.0) that no real API-wrapper tool reads that pair: each wants its own
//! named, structured parameters (`owner`, `repo`, `path`, `branch`, `message`, `sha`, ...), and the
//! mock's undo "success" was the mock not validating its arguments (req/152 §5, the A2 filing finding). (sem: SEM-gx-adapter-mcp-118)
//!
//! So a declaration now carries an optional **[`RestoreTemplate`]**: "build the restore tool's (sem: SEM-gx-adapter-mcp-119)
//! arguments *this way*, from the forward call and from the escrowed prior contents". A declaration (sem: SEM-gx-adapter-mcp-120)
//! without one keeps the v0.1 `{contents, uri}` convention **unchanged** -- the fs/notes/mock
//! deployments and every existing test read exactly as before (no-delete; the old form is narrowed,
//! not removed: it remains the default and the right form for `resources/read`-shaped restore tools).
//!
//! What a template may draw from is fixed by *when* the inverse is built: **E-M4-30** constructs the
//! escrow before `apply` (43 T-10b), so the material is the forward call's own arguments and the
//! resource's prior contents -- and nothing the server only says *after* the call runs (a created
//! issue's number, for one, which is why github's issue pair is declarable but not template-able;
//! `tests/mcp_restore_template.rs` and req/153 carry that denominator).
//!
//! 🔴 **Narrowed, not removed, by v0.3-a A-2′** (`req/38` §98, ruling 1 + §99, ruling 1, two-phase (sem: SEM-gx-adapter-mcp-121)
//! escrow): the sentence above stays true *of an escrow that must be complete at T-10b*. A
//! declaration may now name do-time members ([`ArgSource::DoResult`] /
//! [`ArgSource::DoResultNumberFrom`]) — the escrow is then **partial** (`Pending`), every
//! pre-state member still resolves before `apply` exactly as above (zero regression on that
//! road), and the do-time member is completed after `apply` from the journalled observation,
//! inside the same Committing critical section. The issue pair is therefore template-able since
//! this window, with the accepted residue §98/§99 spell out (opt-in per declaration; a completion
//! failure is `Unavailable` on the receipt, never a silent success).
//!
//! 🔴 **Widened, not replaced, by `req/38` §196 (DR-46-9 A-3 / DR-46-10 / DR-46-12)**: a prior a
//! server will not hand over through `resources/read`
//!
//! Everything above assumes the resource's prior contents are **readable**, because
//! `ToolTransport::read` sends MCP's `resources/read` and the fs/notes demo and github's contents
//! face both answer it. `req/265` §1 measured the first target where that assumption is simply
//! false: `github/github-mcp-server` v1.9.0 registers **five** resource templates, all of them
//! `repo://…/contents…`, so an issue, a pull request and a gist have **no read face at all**. The
//! same hole closed Notion out one target earlier (`req/38` §123, DR-V4B-1). For those tools the
//! inverse is not un-constructible — every coordinate it needs is already in the forward call —
//! it is **un-escrowable**, which is a different fact and deserves a different answer.
//!
//! So a declaration may now carry a [`PriorRead`]: "the prior this restore draws from is what
//! **this read tool** answers, called with **these arguments**". Three bounds come with it, and
//! none of them is optional:
//!
//! 1. **The tool and the arguments are the deployment's, never the agent's.** The tool name comes
//!    out of the catalogue file and every argument is built by a [`RestoreTemplate`] from the
//!    forward call's own members and from constants — `crate::invert` is the only site that
//!    reaches [`crate::ToolTransport::read_prior_by_tool`], and `tests/ac_051.rs` D-6 derives that
//!    from this crate's `src/` rather than being told it.
//! 2. **A read declaration may not draw from a prior.** [`ArgSource::PriorContentsUtf8`],
//!    [`ArgSource::PriorJson`] and the two do-result words are refused inside a [`PriorRead`]
//!    ([`PriorRead::arguments_from_forward`]): a read is what *produces* the prior, and an empty
//!    prior resolving to `""` would be a declaration that quietly reads nothing.
//! 3. **A read that fails refuses the effect** ([`OnReadFailure::Refuse`], the default and the
//!    behaviour every catalogue shipped before this window already had). A deployment that would
//!    rather take the effect with its reversibility recorded as
//!    [`Reversibility::Unknown`] declares [`ON_READ_FAILURE_KEY`] and gets that instead —
//!    opt-in, never silent (DR-46-12).
//! 4. 🔴 **A read must say which object it answered about, and gx checks it** — the fourth bound,
//!    added by **DR-46-15** (`req/38` §199 ruling 2) after the eighteenth adversarial audit
//!    measured what the first three left open. [`PriorRead`] carries a required
//!    [`ObjectIdentity`]: the deployment spells how the read's own answer names the object, as
//!    this adapter's resource URI, and `crate::invert` requires that spelling to be the locator
//!    the compare-and-set attests. The whole argument, and the measurement that forced it, is at
//!    [`ObjectIdentity`].
//!
//! 5. 🔴 **The compare-and-set half has the same second road, declared per locator** — the fifth
//!    bound, added by **DR-46-16** (`req/38` §218 ruling 1). [`CAS_READ_KEY`] is where a
//!    deployment says which tool reads the objects under a resource-URI prefix, and
//!    [`crate::McpAdapter::snapshot`], `precondition` and the post-apply observation take that
//!    road instead of `resources/read` when one matches. A tools-only server that declares it can
//!    now be planned on.
//!
//! 🔴 **Superseded, and the previous window's sentence is kept beside the correction rather than
//! deleted**: until DR-46-16, this paragraph read *"`snapshot` and `precondition` still go through
//! `resources/read` … this window moves the escrow half alone. The CAS half is `req/38` §123
//! ruling 1 (b)'s open ground and stays open."* That open ground is now closed **for declared
//! locators only**. What remains open, said here rather than found later:
//!
//! * a locator matching no `$cas_read` pattern on a server with no resource face is still one
//!   `gx wrap` refuses to plan on — declaration unlocks it, and nothing else does
//!   (`tests/notion_page_catalogue.rs` carries both measurements now: the refusal without a
//!   declaration and the round trip with one);
//! * the CAS road does **not** yet bind its read's answer to the object the way [`ObjectIdentity`]
//!   binds the escrow road's. That is **DR-46-21** (`req/38` §218 ruling 2, `req/305`), and
//!   `docs/LIMITS.md` carries it on the page a buyer reads.
//!
//! 🔴 **Narrowed, not removed, by `req/38` §102, ruling 2** (`req/164` §2 F2): the fail-safe sentence (sem: SEM-gx-adapter-mcp-122)
//! above is a claim about derivation **failure** — a declaration whose `do_result_number_from`
//! pointer names a **different** resource's digit-terminated URL (`.../issues/1/comments/456`)
//! derives with confidence and can mint a wrong number, so declaration soundness (that the pointer
//! names the created resource's own URL) is the deployment's responsibility, the same way the
//! catalogue's "what undoes what" already is. (sem: SEM-gx-adapter-mcp-123)

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where one argument of a restore call comes from (the template vocabulary).
///
/// Externally tagged on purpose: the JSON spelling of a catalogue file is `{"forward": "owner"}`,
/// `{"const": "..."}`, `"prior_contents_utf8"`, `{"git_blob_sha1_of_forward": "content"}` -- one
/// tag, one meaning, no untagged guessing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgSource {
    /// Copy a member of the **forward call's** JSON arguments, verbatim (any JSON value).
    ///
    /// The forward call is the one thing an inverse is *about*, so its arguments are the natural
    /// coordinates (`owner`, `repo`, `path`, `branch`, ...) of where the compensating call lands.
    Forward(String),
    /// A fixed string the deployment declares (`message: "gx undo: ..."`, `state: "closed"`).
    Const(String),
    /// 🔴 A fixed **JSON value** of any type the deployment declares (`in_trash: false`, `n: 3`,
    /// `{"k": "v"}`) — **DR-V4B-2** (`req/38` §123 ruling 2, `req/189`).
    ///
    /// `Const` carries a string and nothing else, so a restore tool whose argument is a boolean
    /// (`notion:patch-page {in_trash: false}` — the second half of the trash round trip `req/183`
    /// measured raw) could not be declared at all: the lane worked around it with the DELETE tool
    /// and left the reverse undeclarable. This is the typed form. `Const` is unchanged (backward
    /// compatible: every shipped catalogue reads as before); `const_json` is a second spelling for
    /// a value that is not a string, resolved verbatim (`serde_json::Value`, canonical DAG-CBOR
    /// once it is inside the escrowed inverse like every other member).
    ConstJson(serde_json::Value),
    /// The resource's **prior contents**, as UTF-8 text -- the body 42 §5 requires an escrow to
    /// carry ("since a digest-only form makes an actual undo physically impossible"), read at escrow time while the (sem: SEM-gx-adapter-mcp-124)
    /// pre-state is still live (43 T-10b). Non-UTF-8 contents make the template unresolvable
    /// (a binary member is v0.2's `blob` residue, the same one `gx-mcp-wire`'s read path declares).
    PriorContentsUtf8,
    /// 🔴 A member of the **escrowed prior**, addressed by JSON pointer (RFC 6901), verbatim.
    ///
    /// The seventh word (**DR-46-10**, `req/38` §196), and the mirror image of
    /// [`ArgSource::DoResult`]: that one points into what the server said *after* the call, this
    /// one points into what the server said *before* it. Both are pointers into one JSON document
    /// and neither invents a coordinate.
    ///
    /// Why it had to exist for the read-by-tool road: [`ArgSource::PriorContentsUtf8`] hands the
    /// **whole** prior over as one string, which is right when the prior *is* the value (a file's
    /// bytes, `create_or_update_file`'s `content`). A read tool answers a **document** — a gist's
    /// JSON, an issue's JSON — and the restore call wants one member of it (`/files/notes.md/content`,
    /// `/body`, `/title`). Without this word the only declarable form was "send the entire read
    /// answer as the new content", which is not the inverse of anything.
    ///
    /// Resolution is exact and fails safe: the prior must parse as JSON and the pointer must
    /// resolve, or the template is unresolvable and `crate::invert` answers `Ok(None)` — the third
    /// `Ok(None)` this module's header already describes. A pointer that resolves to a JSON `null`
    /// resolves to `null`, which is a value the deployment declared and not an absence.
    ///
    /// 🔴 Not shipped alone: `req/38` §196 rules DR-46-10 adopted "in the same lane as DR-46-9 A-3,
    /// never on its own", because a pointer into a prior nobody can read has nothing to point at.
    ///
    /// 🔴 **Narrowed, not replaced, by DR-46-14** (`req/38` §198 ruling (c), §199 ruling 2): the
    /// payload is a [`PriorPointer`], whose first spelling **is** the literal string this word
    /// shipped with, and whose second is a pointer **bound to the forward call**
    /// (`["/files/", {"forward": "filename"}, "/content"]`). The reason is measured rather than
    /// theoretical and is written out at [`PriorPointer`].
    PriorJson(PriorPointer),
    /// The lower-hex **git blob SHA-1** of a forward text argument: `sha1("blob <len>\0" ++ bytes)`.
    ///
    /// 🔴 Why a generic MCP catalogue carries a git-flavoured derivation: the first real target's
    /// update tool (`create_or_update_file`, github-mcp-server v1.9.0) *requires* the blob sha of
    /// the file being replaced and validates it against the live file. At undo time the live file
    /// holds what the **forward call wrote**, and a git blob id is content-addressed -- so the value
    /// the server will demand *after* the forward call is computable *before* it, from the forward
    /// arguments alone. That is the one shape of "a do-time return's SHA" an escrow built before (sem: SEM-gx-adapter-mcp-125)
    /// `apply` (**E-M4-30**) can carry: derivable-from-forward, never reported-by-server. SHA-1 here
    /// is git's content address, not a security boundary (the receipts' own hash road stays
    /// gx-canon's, 41 §6).
    GitBlobSha1OfForward(String),
    /// 🔴 A member of the **forward call's observed result** (JSON pointer, RFC 6901), verbatim.
    ///
    /// The fifth word (`req/38` §98, ruling 1, two-phase escrow): a value that exists only in the (sem: SEM-gx-adapter-mcp-126)
    /// server's do-time answer — which E-M4-30's physics puts strictly after the escrow is built
    /// (43 T-10b). A declaration carrying this member therefore makes the escrow **partial**: the
    /// inverse is escrowed with every other member resolved and this one pending, the applied
    /// call's answer is journalled as an observation, and the engine completes the inverse inside
    /// the same Committing critical section (`gx_substrate::InverseCompletion`). Every completion
    /// failure folds to `InverseStatus::Unavailable` with the commit continuing (§99, ruling 2-④) -- (sem: SEM-gx-adapter-mcp-127)
    /// fail-safe, never silent.
    ///
    /// 🔴 Kept although github-mcp-server v1.9.0's write results carry no such member (`req/161`
    /// §1 measured `{"id","url"}` only): the vocabulary is per-server declaration material, and a
    /// server whose write result does carry the value (`/number` on some future or third-party
    /// server) declares this form. For v1.9.0's actual shape, see
    /// [`ArgSource::DoResultNumberFrom`].
    DoResult(String),
    /// 🔴 The trailing decimal path segment of a **forward call's observed result** text member
    /// (JSON pointer to the text; the derivation is `/(\d+)$` cast to a JSON **number**).
    ///
    /// The sixth word (`req/38` §99, ruling 1). Measured need: github-mcp-server v1.9.0's write (sem: SEM-gx-adapter-mcp-128)
    /// results are `MinimalResponse { id, url }` with no `number` member — and upstream's own doc
    /// comment declares the derivation ("all other information can be derived from the URL", (sem: SEM-gx-adapter-mcp-129)
    /// `minimal_types.go`, `req/161` §1, verbatim). So the issue pair declares (sem: SEM-gx-adapter-mcp-130)
    /// `"issue_number": {"do_result_number_from": "/url"}` and the number is minted from
    /// `.../issues/1` at completion time. A pointer that resolves to no string, or to a string
    /// whose last `/`-segment is not all decimal digits, derives nothing — fail-safe:
    /// `InverseCompleted { None }` → `Unavailable` → a later undo is refused by name, and the
    /// receipt shows `Admit` beside `inverse_delta: None`. The URL's shape is upstream's published
    /// design intent rather than an API contract (§99, ruling 1's accepted residue): if it ever (sem: SEM-gx-adapter-mcp-131)
    /// changes, the failure mode is that refusal, never a wrong call.
    DoResultNumberFrom(String),
}

/// 🔴 One piece of a [`PriorPointer`] that is bound to the forward call (**DR-46-14**).
///
/// Two spellings and no third: a literal run of pointer text (`"/files/"`), or a member of the
/// **forward call** substituted into it (`{"forward": "filename"}`). Nothing here may draw from
/// the prior or from a do-time result, because the pointer is what says *where in the prior to
/// look* and a pointer that read the prior to build itself would be circular.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PointerSegment {
    /// Literal pointer text, spelled into the pointer verbatim.
    Literal(String),
    /// A member of the forward call's arguments, RFC 6901-escaped
    /// ([`escape_pointer_token`]) and substituted.
    Forward {
        /// The forward member's name.
        forward: String,
    },
}

/// 🔴 **DR-46-14** (`req/38` §198 ruling (c) / §199 ruling 2) — where in the escrowed prior a
/// [`ArgSource::PriorJson`] member looks, either literally or **bound to the forward call**.
///
/// # What was measured, and why a literal pointer was not enough
///
/// RFC 6901 has no variables. `req/268` P-9 measured the consequence on the first real target:
/// a declaration reading `{"prior_json": "/files/notes.md/content"}` beside
/// `{"filename": {"forward": "filename"}}` resolves **anyway** for a forward call against
/// `other.md` — `notes.md` is still in the document — so gx answered `true` and built a restore
/// carrying `other.md`'s coordinate with `notes.md`'s text. That is not an inverse, and nothing on
/// the road said so.
///
/// The fix is the one `req/38` §198 (c) named first: let the pointer's segments come from the
/// **forward call**, so the member the declaration guards is the member the call touched. A
/// declaration that spells
///
/// ```text
/// "content": { "prior_json": ["/files/", { "forward": "filename" }, "/content"] }
/// ```
///
/// resolves to `/files/other.md/content` for a call against `other.md` — which either names that
/// file's own prior text, or resolves to nothing and the third `Ok(None)` answers, so the wrong
/// member can no longer travel with a `true` beside it.
///
/// # Backward compatible by construction
///
/// [`PriorPointer::Literal`] is the v0.5-d spelling, unchanged: a JSON **string** parses to it and
/// resolves exactly as it did, so every catalogue written in the previous window reads to the same
/// value. The bound form is a JSON **array**, which the previous window's parser refused outright.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PriorPointer {
    /// One literal RFC 6901 pointer, as v0.5-d shipped it.
    Literal(String),
    /// Segments that concatenate into a pointer, with forward members substituted.
    Bound(Vec<PointerSegment>),
}

/// RFC 6901 §3's escaping for one reference token: `~` becomes `~0` and `/` becomes `~1`.
///
/// Public because it is the half of [`PriorPointer::Bound`] a reader most wants to check by hand:
/// a filename carrying a `/` must not be able to widen the pointer it is substituted into, and the
/// order of the two replacements is the one the RFC fixes (`~` first, or `/` would be re-escaped).
#[must_use]
pub fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

impl PriorPointer {
    /// Build the pointer this call's prior is read at.
    ///
    /// # Errors
    /// A human sentence when a segment names a forward member the call does not carry, or carries
    /// as something other than text. The caller answers `Ok(None)` over it, which is the third
    /// `Ok(None)` of the module header: the declaration is sound and *this* call is not one it was
    /// written for.
    pub fn resolve(
        &self,
        forward: &serde_json::Map<String, serde_json::Value>,
    ) -> core::result::Result<String, String> {
        match self {
            PriorPointer::Literal(pointer) => Ok(pointer.clone()),
            PriorPointer::Bound(segments) => {
                let mut pointer = String::new();
                for segment in segments {
                    match segment {
                        PointerSegment::Literal(text) => pointer.push_str(text),
                        PointerSegment::Forward { forward: member } => {
                            let text = forward
                                .get(member)
                                .and_then(serde_json::Value::as_str)
                                .ok_or_else(|| {
                                    format!(
                                        "the forward call carries no text argument {member:?} to \
                                         bind the prior pointer's segment to"
                                    )
                                })?;
                            pointer.push_str(&escape_pointer_token(text));
                        }
                    }
                }
                Ok(pointer)
            }
        }
    }

    /// Whether the pointer draws any segment from the forward call.
    #[must_use]
    pub fn is_bound(&self) -> bool {
        matches!(self, PriorPointer::Bound(_))
    }
}

impl ArgSource {
    /// Whether this member can only be resolved **after** `apply`, from the observed result.
    #[must_use]
    pub fn is_do_result(&self) -> bool {
        matches!(
            self,
            ArgSource::DoResult(_) | ArgSource::DoResultNumberFrom(_)
        )
    }

    /// 🔴 Whether this member draws from the **prior** — the material a read is what produces.
    ///
    /// Read by [`PriorRead::arguments_from_forward`] so that a read declaration naming one is
    /// refused at resolution rather than resolving against an empty prior: `PriorContentsUtf8`
    /// over zero bytes is `Ok("")`, and a read whose argument silently became the empty string
    /// would be a declaration that reads the wrong thing while answering `Ok`.
    #[must_use]
    pub fn is_prior(&self) -> bool {
        matches!(self, ArgSource::PriorContentsUtf8 | ArgSource::PriorJson(_))
    }

    /// 🔴 **`req/303` H-01 / DR-46-19** — whether this member draws on something the **forward
    /// call does not carry**.
    ///
    /// # Why this exists rather than `!matches!(source, ArgSource::Forward(_))`
    ///
    /// R20 spelled [`RestoreSpec::soundness`]'s widened gate as a **negative** set: every variant
    /// that is not `Forward` counts as drawing from elsewhere. The twenty-first audit measured what
    /// that costs. [`ArgSource::GitBlobSha1OfForward`] is not `Forward`, so it passed — and that
    /// variant's own doc says, three paragraphs up, that the value it produces is *"computable
    /// before [the forward call], from the forward arguments alone … derivable-from-forward, never
    /// reported-by-server"*. A template of `{"uri": {"forward": "uri"}, "sha":
    /// {"git_blob_sha1_of_forward": "contents"}}` is therefore a function of the forward call and
    /// nothing else, which is the exact sentence [`TEMPLATE_NAMES_NO_PRIOR`] prints when it
    /// refuses — and it passed the gate that prints it. Measured end to end: `verdict=Admit`, a
    /// signed commit, and the undo gx printed emptied the object with `rc=0`.
    ///
    /// The negative spelling said "not the one variant I thought of". The positive one says which
    /// variants draw from outside, one arm each, and is the thing a reader can check against the
    /// doc comment of every variant above.
    ///
    /// # Fail-closed for a variant nobody classified
    ///
    /// The `_` arm answers **`false`**. A word added to this vocabulary later and not classified
    /// here is treated as *carried by the forward call*, so a template built only out of it is
    /// **refused** rather than admitted. That is the direction of the defect this method exists to
    /// close: the audit's hole was a variant that silently counted as safe.
    #[must_use]
    pub fn draws_from_outside_the_forward_call(&self) -> bool {
        match self {
            // The prior: the object as it was, which is the material an inverse is made of.
            ArgSource::PriorContentsUtf8 | ArgSource::PriorJson(_) => true,
            // The applied call's own answer: the id of a thing that did not exist before the call,
            // which is what an inverse-by-deletion is keyed on (`fixtures/notion-page-catalogue.json`).
            ArgSource::DoResult(_) | ArgSource::DoResultNumberFrom(_) => true,
            // A value the declaration itself supplies: the other side of a flipped field
            // (**DR-V4B-2**, `req/38` §123 ruling 2 — `patch-page {in_trash: false}`).
            ArgSource::Const(_) | ArgSource::ConstJson(_) => true,
            // A member of the forward call, verbatim.
            ArgSource::Forward(_) => false,
            // 🔴 A **function** of a member of the forward call. Not `Forward`, and still nothing
            // the forward call does not carry — this is the variant `req/303` H-01 measured.
            ArgSource::GitBlobSha1OfForward(_) => false,
            // 🔴 Fail-closed. See this method's second section: an unclassified word is treated as
            // carried by the forward call, so it cannot on its own satisfy the gate.
            #[allow(unreachable_patterns)]
            _ => false,
        }
    }

    /// Resolve a do-result member against the observed result (UTF-8 JSON bytes).
    ///
    /// # Errors
    /// A human sentence naming the member's pointer and the reason: the observation is not JSON,
    /// the pointer resolves to nothing, or (for the derived form) the pointed text carries no
    /// trailing `/<digits>` segment. The caller answers `None` (→ `Unavailable`) over it.
    pub fn resolve_from_observation(
        &self,
        observation: &[u8],
    ) -> core::result::Result<serde_json::Value, String> {
        let observed: serde_json::Value = serde_json::from_slice(observation)
            .map_err(|e| format!("the observed result is not JSON: {e}"))?;
        match self {
            ArgSource::DoResult(pointer) => observed.pointer(pointer).cloned().ok_or_else(|| {
                format!("the observed result carries nothing at the pointer {pointer:?}")
            }),
            ArgSource::DoResultNumberFrom(pointer) => {
                let text = observed
                    .pointer(pointer)
                    .and_then(serde_json::Value::as_str)
                    .ok_or_else(|| {
                        format!("the observed result carries no text at the pointer {pointer:?}")
                    })?;
                let number = trailing_number(text).ok_or_else(|| {
                    format!(
                        "the text at {pointer:?} ends in no `/<digits>` segment to derive a \
                         number from (got {text:?})"
                    )
                })?;
                Ok(serde_json::Value::Number(number))
            }
            other => Err(format!(
                "{other:?} is not a do-result member and resolves before apply"
            )),
        }
    }
}

/// The derivation of [`ArgSource::DoResultNumberFrom`]: the trailing `/`-separated path segment,
/// accepted only when it is one or more decimal digits and nothing else, minted as a JSON number.
///
/// Public so a test can hold the edge cases (`.../issues/1`, `.../pull/2`, a non-numeric tail, an
/// empty tail) against it without going through a whole template. `u64` because GitHub's per-repo
/// numbers are positive and the JSON number the servers' update tools read is integral;
/// out-of-range digits refuse rather than round.
#[must_use]
pub fn trailing_number(text: &str) -> Option<serde_json::Number> {
    // `/(\d+)$`, literally: a '/' must exist, and everything after the last one must be digits.
    let (_, segment) = text.rsplit_once('/')?;
    if segment.is_empty() || !segment.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    segment.parse::<u64>().ok().map(serde_json::Number::from)
}

/// How a restore tool's arguments are built, member by member.
///
/// A `BTreeMap` so that resolution is deterministic in a way a reader can predict: the resolved
/// JSON object carries its members in key order, the same order two runs of one declaration
/// produce.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RestoreTemplate {
    arguments: BTreeMap<String, ArgSource>,
}

impl RestoreTemplate {
    /// An empty template: a restore call with no arguments at all.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare one argument.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, source: ArgSource) -> Self {
        self.arguments.insert(name.into(), source);
        self
    }

    /// The declared members, for a report line.
    #[must_use]
    pub fn arguments(&self) -> &BTreeMap<String, ArgSource> {
        &self.arguments
    }

    /// Build the restore call's arguments: UTF-8 JSON, the same encoding a forward call's
    /// arguments already travel in (`gx wrap` stores the agent's JSON verbatim, and
    /// `gx-mcp-wire`'s `arguments_of` sends a JSON object through unchanged).
    ///
    /// # Errors
    /// A human sentence naming the member and the reason, when this call does not carry the
    /// material the declaration names: forward arguments that are not a JSON object, a forward
    /// member that is absent (or non-text where text is derived from), prior contents that are not
    /// UTF-8. The caller (`crate::invert`) answers `Ok(None)` over it -- "gx cannot construct the (sem: SEM-gx-adapter-mcp-132)
    /// undo of this call" is a fact for **E-M3-4**'s escalation, not a crash. (sem: SEM-gx-adapter-mcp-133)
    pub fn resolve(
        &self,
        forward_arguments: &[u8],
        prior_contents: &[u8],
    ) -> core::result::Result<Vec<u8>, String> {
        let (arguments, pending) = self.resolve_split(forward_arguments, prior_contents)?;
        if let Some((name, _)) = pending.iter().next() {
            return Err(format!(
                "the member {name:?} is a do-result member and resolves only after apply; \
                 `resolve` answers escrow-time material alone (use `resolve_split`)"
            ));
        }
        Ok(arguments)
    }

    /// The two-phase form (`req/38` §98, ruling 1): resolve every escrow-time member now, and hand (sem: SEM-gx-adapter-mcp-134)
    /// back the do-result members still **pending** the applied call's observation.
    ///
    /// An empty pending map is the whole of the old behaviour — a declaration with no do-result
    /// member resolves exactly as it always did, and `crate::invert` escrows the result as a
    /// complete inverse. A non-empty one makes the escrow partial (`InverseStatus::Pending`), and
    /// the pending members travel **inside the partial delta's payload** so that the completion
    /// step reads its instructions from the escrowed value itself rather than from a catalogue a
    /// recovery might no longer hold.
    ///
    /// # Errors
    /// [`RestoreTemplate::resolve`]'s, for the escrow-time members.
    pub fn resolve_split(
        &self,
        forward_arguments: &[u8],
        prior_contents: &[u8],
    ) -> core::result::Result<(Vec<u8>, BTreeMap<String, ArgSource>), String> {
        let forward: serde_json::Value = serde_json::from_slice(forward_arguments)
            .map_err(|e| format!("the forward call's arguments are not JSON: {e}"))?;
        let forward = forward.as_object().ok_or_else(|| {
            "the forward call's arguments are not a JSON object, and a template has nothing to \
             draw `forward` members from"
                .to_string()
        })?;
        let mut resolved = serde_json::Map::new();
        let mut pending = BTreeMap::new();
        for (name, source) in &self.arguments {
            let value = match source {
                ArgSource::Forward(member) => forward
                    .get(member)
                    .cloned()
                    .ok_or_else(|| format!("the forward call carries no argument {member:?}"))?,
                ArgSource::Const(text) => serde_json::Value::String(text.clone()),
                ArgSource::ConstJson(value) => value.clone(),
                ArgSource::PriorContentsUtf8 => serde_json::Value::String(
                    String::from_utf8(prior_contents.to_vec()).map_err(|e| {
                        format!(
                            "the prior contents are not UTF-8 ({e}); a binary restore member is \
                             v0.2's `blob` residue"
                        )
                    })?,
                ),
                // DR-46-10: a pointer into the prior. Both failures are "this call is not one the
                // declaration was written for" -- the caller answers `Ok(None)` over them.
                //
                // 🔴 DR-46-14: the pointer is built from the forward call first, so a bound
                // segment names the member *this* call touched rather than the member the
                // declaration happened to be written against.
                ArgSource::PriorJson(spec) => {
                    let pointer = spec.resolve(forward)?;
                    let prior: serde_json::Value =
                        serde_json::from_slice(prior_contents).map_err(|e| {
                            format!(
                                "the prior contents are not JSON, so the pointer {pointer:?} has \
                                 no document to resolve against ({e})"
                            )
                        })?;
                    prior.pointer(&pointer).cloned().ok_or_else(|| {
                        format!("the prior contents carry nothing at the pointer {pointer:?}")
                    })?
                }
                ArgSource::GitBlobSha1OfForward(member) => {
                    let text = forward
                        .get(member)
                        .and_then(serde_json::Value::as_str)
                        .ok_or_else(|| {
                            format!(
                                "the forward call carries no text argument {member:?} to derive a \
                                 git blob sha from"
                            )
                        })?;
                    serde_json::Value::String(git_blob_sha1_hex(text.as_bytes()))
                }
                // The do-time members: nothing to draw from yet (E-M4-30's physics), so the
                // declaration itself is carried forward for the completion step.
                ArgSource::DoResult(_) | ArgSource::DoResultNumberFrom(_) => {
                    pending.insert(name.clone(), source.clone());
                    continue;
                }
            };
            resolved.insert(name.clone(), value);
        }
        let arguments = serde_json::to_vec(&serde_json::Value::Object(resolved))
            .map_err(|e| format!("the resolved arguments have no JSON form: {e}"))?;
        Ok((arguments, pending))
    }
}

/// The lower-hex SHA-1 of git's blob object form: `"blob <len>\0" ++ contents`.
///
/// The same value `git hash-object` prints and the GitHub contents API demands as its
/// optimistic-concurrency token. Public so that a test can hold it against `git hash-object`'s own
/// vectors without going through a whole template.
#[must_use]
pub fn git_blob_sha1_hex(contents: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", contents.len()).as_bytes());
    hasher.update(contents);
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(40);
    for byte in digest {
        use core::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// 🔴 **DR-46-9 A-3** (`req/38` §196): where a tools-only server's **prior** comes from.
///
/// "The prior this restore draws from is what `by_tool` answers, called with these `arguments`."
/// The arguments are an ordinary [`RestoreTemplate`], which is the point — no second vocabulary
/// was invented for a read, so the words a deployment already knows (`forward`, `const`,
/// `const_json`, `git_blob_sha1_of_forward`) are the words it writes here.
///
/// What a read declaration may **not** name is a prior member or a do-result member, and
/// [`PriorRead::arguments_from_forward`] refuses both by name rather than resolving them: a read
/// runs before there is a prior and long before there is a result.
///
/// `deny_unknown_fields` for [`RestoreSpec`]'s reason: a misspelt `by_tool` that parsed would be a
/// read the operator did not declare, found at the first failed escrow.
///
/// 🔴 **`identity` is required, and that is the whole of DR-46-15** (`req/38` §199 ruling 2). See
/// [`ObjectIdentity`] for what it says and why a read that cannot say it is one gx refuses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorRead {
    by_tool: String,
    #[serde(default)]
    arguments: RestoreTemplate,
    identity: ObjectIdentity,
}

impl PriorRead {
    /// "call `by_tool`, with the arguments this template builds, and the answer is about the
    /// object `identity` spells".
    #[must_use]
    pub fn new(
        by_tool: impl Into<String>,
        arguments: RestoreTemplate,
        identity: ObjectIdentity,
    ) -> Self {
        Self {
            by_tool: by_tool.into(),
            arguments,
            identity,
        }
    }

    /// 🔴 **DR-46-15** — how this read's answer says which object it is about.
    #[must_use]
    pub fn identity(&self) -> &ObjectIdentity {
        &self.identity
    }

    /// 🔴 **`req/269` M-05 / DR-46-15** — everything about this declaration that can be judged
    /// **without a call**, judged before `gx wrap` starts.
    ///
    /// The audit measured the cost of leaving these to resolution time: a catalogue naming a prior
    /// member inside a read declaration started a session, ran, and then refused the first effect
    /// with a sentence that read like the *server* had failed — while the server had never been
    /// asked (zero arrivals). Two of the three checks below are exactly that mistake, and the
    /// third is the read that is bound to no object at all.
    ///
    /// 🔴 **`req/279` L-03** adds the fourth: a read face with **no name**. The audit measured
    /// `arrivals=["read_by_tool  {}"]` — an empty tool name went to the wire and the server
    /// answered `-32602`. The harm is small; the place is the point. R17 put "what can be known
    /// before a call is known before the call" here, and this is one line in that same place.
    ///
    /// # Errors
    /// A human sentence naming the member and the mechanism. [`Catalogue::from_json`] turns it
    /// into a parse error, so a file carrying one never reaches a running session.
    pub fn soundness(&self) -> core::result::Result<(), String> {
        if names_nothing(&self.by_tool) {
            return Err(format!(
                "the read declaration's `by_tool` is {:?}, which names no tool: a read face has a \
                 name before it has arguments",
                self.by_tool
            ));
        }
        for (name, source) in self.arguments.arguments() {
            if source.is_prior() {
                return Err(format!(
                    "the read declaration's member {name:?} draws from the prior contents, which \
                     is what this read exists to produce"
                ));
            }
            if source.is_do_result() {
                return Err(format!(
                    "the read declaration's member {name:?} draws from the applied call's result, \
                     which does not exist until long after this read"
                ));
            }
        }
        self.identity.soundness()
    }

    /// The read tool's name, as the deployment declared it.
    #[must_use]
    pub fn by_tool(&self) -> &str {
        &self.by_tool
    }

    /// The declared arguments, for a report line and for the AC-051 derivation.
    #[must_use]
    pub fn arguments(&self) -> &RestoreTemplate {
        &self.arguments
    }

    /// Build the read call's arguments from the forward call alone.
    ///
    /// # Errors
    /// A human sentence when the declaration names material a read cannot have (a prior member or
    /// a do-result member), and [`RestoreTemplate::resolve`]'s errors otherwise. Every one of them
    /// is a read this deployment declared and this call cannot make, which `crate::invert` treats
    /// as a failed read — so [`OnReadFailure`] decides what happens next, exactly as it does for a
    /// read the server refused.
    pub fn arguments_from_forward(
        &self,
        forward_arguments: &[u8],
    ) -> core::result::Result<Vec<u8>, String> {
        // 🔴 `req/269` M-05: the same three questions [`PriorRead::soundness`] answers at parse
        // time are asked again here, because a catalogue built in code (`Catalogue::with_prior_read`)
        // never went through a parser. Both roads therefore refuse, and neither refuses in the
        // words of a server that was never called -- `crate::invert` carries the sentence.
        self.soundness()?;
        // Nothing above survives to reach the prior, so the empty slice is unreachable material
        // rather than a value the resolution may quietly use.
        let (arguments, _pending) = self.arguments.resolve_split(forward_arguments, &[])?;
        Ok(arguments)
    }
}

/// 🔴 **DR-46-15** (`req/38` §199 ruling 2) — how a read's answer says **which object it is
/// about**, in the one spelling gx can check it against: the locator's resource URI.
///
/// # The hole this closes, measured
///
/// `req/269` H-01 measured the shape the read-by-tool road had without this. gx quantifies over
/// **two** objects at once and never asked whether they were the same one:
///
/// * `snapshot`, `precondition` and the post-apply observation all read
///   `position.resource()` — the locator. That is the object the compare-and-set attests, and the
///   object an undo is refused over when the world moved (DR-43-1).
/// * `invert`'s read-by-tool call carries **no locator at all**: the tool name is the catalogue's
///   and the arguments are built from the forward call. The bytes it escrows come from whatever
///   object that tool answered about.
///
/// On the only server this road exists for, those two cannot be made the same by accident: a gist
/// has no resource face, so a locator that names it is one `snapshot` refuses to plan on, and the
/// only deployment that runs is one whose locator names a **different** object. The measured
/// consequence was the accident this product exists to prevent — a third party's write to the gist
/// was silently overwritten by an undo, because the compare-and-set was watching a file nobody had
/// touched.
///
/// `req/269` M-03 measured the second half from the other side: with no predicate over the read's
/// answer, a read tool answering with **another object's document** produced `true` and a restore
/// call carrying a stranger's text.
///
/// # What the declaration says, and why it is spelled as a resource URI
///
/// One predicate closes both. The deployment declares how the object the read answered about is
/// **spelled as this adapter's resource URI**, out of parts:
///
/// ```text
/// "identity": [ "gist:", { "answer": "/id" } ]
/// ```
///
/// and `crate::invert` requires the result to equal `position.resource()`, normalised the way
/// every other locator on this road is (`crate::locator`). A read that answered about
/// `SOMEONE-ELSES-GIST` spells a different resource; a read whose object is simply not the
/// locator's spells a different resource. Both refuse, and they refuse through
/// [`OnReadFailure`] — the posture DR-46-12 already fixed — because from the escrow's side the
/// fact is the same one: **the prior this transformation needed did not arrive**.
///
/// At least one [`IdentityPart::Answer`] is required ([`ObjectIdentity::soundness`]): an identity
/// built only from the forward call and constants would bind the locator to the agent's
/// coordinates and leave the *answer* unchecked, which is M-03 with extra steps.
///
/// # 🔴 What this still does not close, said here rather than found later
///
/// It binds the **escrowed bytes** to the attested object. It does not bind the **restore call's
/// own target**: what a tool does with the arguments it is handed is the server's, and this
/// adapter has said so since its crate root ([`crate::ToolCall::resource`] — "not necessarily the
/// resource the tool will touch"). A declaration whose restore template names a different object
/// than its read is still a declaration soundness burden, the same one `req/38` §102 ruling 2 put
/// on the deployment, and `docs/LIMITS.md` says so on the page a buyer reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectIdentity {
    parts: Vec<IdentityPart>,
}

/// One piece of an [`ObjectIdentity`].
///
/// Three spellings: literal text, a pointer into the **read tool's answer**, or a member of the
/// **forward call**. A value that is a JSON string is taken verbatim and a value that is a JSON
/// number is rendered as JSON renders it; anything else refuses, because an identity assembled out
/// of an object or an array would be a spelling nobody can compare to a URI.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IdentityPart {
    /// Literal text of the resource URI (`"gist:"`, `"repo://"`).
    Literal(String),
    /// A member of the **read tool's answer**, by RFC 6901 pointer. At least one of these is
    /// required: it is what binds the answer to the object.
    Answer {
        /// The pointer into the read tool's answer.
        answer: String,
    },
    /// A member of the **forward call's** arguments, for the parts of a URI the answer does not
    /// carry (an owner, a repository).
    Forward {
        /// The forward member's name.
        forward: String,
    },
}

impl ObjectIdentity {
    /// The parts, in the order they concatenate.
    #[must_use]
    pub fn new(parts: Vec<IdentityPart>) -> Self {
        Self { parts }
    }

    /// The declared parts, for a report line.
    #[must_use]
    pub fn parts(&self) -> &[IdentityPart] {
        &self.parts
    }

    /// Whether the declaration draws anything from the read tool's answer.
    #[must_use]
    pub fn names_the_answer(&self) -> bool {
        self.parts
            .iter()
            .any(|part| matches!(part, IdentityPart::Answer { .. }))
    }

    /// What can be judged about this declaration **without a call**.
    ///
    /// # Errors
    /// A human sentence when the declaration is empty, or when it names no member of the read
    /// tool's answer — the two shapes that would let an unbound read through wearing a bound one's
    /// face.
    pub fn soundness(&self) -> core::result::Result<(), String> {
        if self.parts.is_empty() {
            return Err(
                "the read declaration's `identity` is empty, so nothing says which object the \
                 read answered about"
                    .to_string(),
            );
        }
        if !self.names_the_answer() {
            return Err(
                "the read declaration's `identity` names no `answer` member, so it spells the \
                 locator out of the forward call alone and leaves the read's own answer unchecked \
                 (DR-46-15: the answer is what has to be bound)"
                    .to_string(),
            );
        }
        Ok(())
    }

    /// Spell the resource URI this read's answer is about.
    ///
    /// 🔴 **`req/279` M-03** — the answer has to put something **into** the spelling. See
    /// [`IdentityFault::AnswerNotRead`] for what the audit measured when it did not.
    ///
    /// # Errors
    /// [`IdentityFault::Unread`] when the answer is not JSON, when a declared pointer or forward
    /// member is absent, or when either resolves to a JSON value that is not a string or a number;
    /// [`IdentityFault::AnswerNotRead`] when every `answer` part spelled the empty string.
    pub fn resource_from(
        &self,
        answer: &[u8],
        forward_arguments: &[u8],
    ) -> core::result::Result<String, IdentityFault> {
        let answered: serde_json::Value = serde_json::from_slice(answer).map_err(|e| {
            IdentityFault::Unread(format!(
                "the read tool's answer is not JSON, so `identity` has nothing to read: {e}"
            ))
        })?;
        let forward: serde_json::Value =
            serde_json::from_slice(forward_arguments).map_err(|e| {
                IdentityFault::Unread(format!("the forward call's arguments are not JSON: {e}"))
            })?;
        let mut spelled = String::new();
        // 🔴 `req/279` M-03: "would the spelling change if every `answer` part were the empty
        // string" is exactly one bit, and it is this one. Tracked in the single pass the spelling
        // already costs rather than by rendering twice, because the answer can be a megabyte.
        let mut the_answer_spelled_something = false;
        for part in &self.parts {
            match part {
                IdentityPart::Literal(text) => spelled.push_str(text),
                IdentityPart::Answer { answer: pointer } => {
                    let value = answered.pointer(pointer).ok_or_else(|| {
                        IdentityFault::Unread(format!(
                            "the read tool's answer carries nothing at the `identity` pointer \
                             {pointer:?}, so it never said which object it was about"
                        ))
                    })?;
                    let text = identity_text(value).ok_or_else(|| {
                        IdentityFault::Unread(format!(
                            "the read tool's answer carries {} at the `identity` pointer \
                             {pointer:?}, which is neither text nor a number and cannot spell a \
                             resource",
                            bounded(&value.to_string())
                        ))
                    })?;
                    the_answer_spelled_something |= !text.is_empty();
                    spelled.push_str(&text);
                }
                IdentityPart::Forward { forward: member } => {
                    let value = forward.get(member).ok_or_else(|| {
                        IdentityFault::Unread(format!(
                            "the forward call carries no argument {member:?} for `identity`"
                        ))
                    })?;
                    spelled.push_str(&identity_text(value).ok_or_else(|| {
                        IdentityFault::Unread(format!(
                            "the forward call carries {} at {member:?}, which is neither text nor \
                             a number and cannot spell a resource",
                            bounded(&value.to_string())
                        ))
                    })?);
                }
            }
        }
        if !the_answer_spelled_something {
            return Err(IdentityFault::AnswerNotRead(format!(
                "every `answer` part of the read declaration's `identity` spelled the empty \
                 string, so {} was built out of the forward call alone and the read's own answer \
                 was never checked",
                bounded(&spelled)
            )));
        }
        Ok(spelled)
    }
}

/// 🔴 **`req/279` M-01 and M-03** — why an `identity` did not produce the object a read answered
/// about, in the two shapes a reader debugs differently.
///
/// Before this window both travelled as one `String` and `crate::invert` appended DR-46-15's
/// sentence to either, so a read that could not be read at all was told it *had named a different
/// object* — and handed the remedy "point the change at the object the read answers for", which
/// nobody can execute when nothing was read. That is `req/269` M-05's species (a fault wearing
/// another's face) one gate further along, and the fix is the same one R17 applied there: give the
/// second cause its own sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IdentityFault {
    /// 🔴 **`req/279` M-01** — the answer could not be read: not JSON, not UTF-8, empty, nested
    /// past the parser's limit, a pointer that resolves to nothing, or a value that is neither
    /// text nor a number. Nothing here establishes *which* object the read was about, and in
    /// particular nothing establishes that it was a different one.
    Unread(String),
    /// 🔴 **`req/279` M-03** — every [`IdentityPart::Answer`] spelled the empty string, so the
    /// resource was built out of the forward call alone.
    ///
    /// [`ObjectIdentity::soundness`] asks whether an `answer` part is *present*; the audit added
    /// one that is present and contributes nothing (`{"answer": "/pad"}` against a server whose
    /// `/pad` is always `""`). The declaration passed parse, the spelling equalled the locator
    /// because it came from the agent's own call, and gx answered `true` while escrowing a
    /// stranger's text. Presence is syntax; this is the predicate.
    AnswerNotRead(String),
}

impl IdentityFault {
    /// The sentence, without the constant `crate::invert` pairs it with.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            IdentityFault::Unread(detail) | IdentityFault::AnswerNotRead(detail) => detail,
        }
    }
}

impl core::fmt::Display for IdentityFault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.detail())
    }
}

/// 🔴 **`req/279` L-04** — how much of a server's text may enter a refusal.
///
/// The audit set a read tool's `id` to 1 MiB and measured a **1,049,118-byte** refusal, which
/// travels verbatim to the agent's tool result and to the operator's terminal. A refusal is a
/// sentence a person acts on; past a few lines the extra bytes are the server's payload wearing a
/// message's clothes. The constants are untouched — this bounds only what is interpolated into
/// them — and the elision says how much was dropped, so nothing is silently lost.
///
/// The cut is at a `char` boundary, which is why this is a function and not a slice: cutting a
/// UTF-8 sequence in half would panic inside the very path that reports a failure.
#[must_use]
pub(crate) fn bounded(text: &str) -> String {
    /// Enough for a URI a person recognises and for the pointer-shaped values a declaration spells.
    const MAX: usize = 256;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut cut = MAX;
    while cut > 0 && !text.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}...({} bytes)", &text[..cut], text.len())
}

/// 🔴 **R36 / `req/476` L-02** — the first top-level key this document declares twice, if any.
///
/// # Why this is a second parse and not a byte scan
///
/// A scan for `"$determinism_boundary"` in the bytes would count the same string inside a value, a
/// nested object or a comment-shaped string, and would then refuse a valid file — a checker whose
/// false positives are refusals is worse than the hole it closes. This walks the document with
/// serde's own `MapAccess`, so what it sees are exactly the top-level **keys** JSON has, in the
/// order they were written, before any map collapses them.
///
/// The value of each pair is read as [`serde::de::IgnoredAny`]: the point is the key list, and
/// parsing the values twice would double the work on every catalogue for nothing.
///
/// Scope, stated rather than implied: **top level only**. Every reserved slot and every tool name
/// is a top-level key, so that is the surface `req/476` L-02 is about. A duplicate inside one
/// entry's `arguments` is still last-one-wins and is recorded as a residue.
fn first_duplicate_key(bytes: &[u8]) -> Option<String> {
    struct TopLevelKeys(Vec<String>);

    impl<'de> serde::Deserialize<'de> for TopLevelKeys {
        fn deserialize<D: serde::Deserializer<'de>>(
            deserializer: D,
        ) -> core::result::Result<Self, D::Error> {
            struct Keys;

            impl<'de> serde::de::Visitor<'de> for Keys {
                type Value = Vec<String>;

                fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str("a JSON object")
                }

                fn visit_map<A: serde::de::MapAccess<'de>>(
                    self,
                    mut map: A,
                ) -> core::result::Result<Self::Value, A::Error> {
                    let mut keys = Vec::new();
                    while let Some(key) = map.next_key::<String>()? {
                        let _: serde::de::IgnoredAny = map.next_value()?;
                        keys.push(key);
                    }
                    Ok(keys)
                }
            }

            deserializer.deserialize_map(Keys).map(TopLevelKeys)
        }
    }

    let keys: TopLevelKeys = serde_json::from_slice(bytes).ok()?;
    let mut seen = std::collections::BTreeSet::new();
    keys.0.into_iter().find(|key| !seen.insert(key.clone()))
}

/// The one rendering an [`IdentityPart`] may take: a JSON string verbatim, a JSON number as JSON
/// writes it, and nothing else.
fn identity_text(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// 🔴 **`req/291` M-04** — the sentence a declaration whose `restored_by` names no tool carries.
///
/// The symmetric half of `req/279` L-03, which made a read face with no name a parse error. The
/// twentieth audit measured the field beside it: `{"doc.write": {"restored_by": ""}}` parsed,
/// `reversibility` answered `true`, and the escrow it built came back out of gx's **own** decoder
/// as `<undecodable: ... an mcp operation names the tool it calls, and this one names none>`. A
/// declaration that parses here and cannot be read back there is a declaration one half of this
/// crate accepts and the other half refuses, and which of the two is right is not in question.
///
/// A constant for [`READ_FAILURE_REFUSAL`](crate::READ_FAILURE_REFUSAL)'s reason: a probe can hold
/// a refusal to its wording only if there is one wording to hold.
pub const RESTORE_TOOL_UNNAMED: &str = "which names no tool: an effect that needs undoing names \
     the tool that undoes it before it says how that tool is called, and an escrow built from a \
     nameless tool is one this adapter's own decoder refuses to read back. What to fix: name the \
     restore tool, or drop the entry (`req/291` M-04: the symmetric half of `req/279` L-03 -- a \
     read face has a name before it has arguments, and so does a restore face)";

/// 🔴 **`req/291` H-01 / DR-46-19** — the sentence a template that never names the prior carries.
///
/// R18 closed "a `read_by` and **no** `arguments` template". The twentieth audit fired the shape
/// one step along: a template that exists and is built out of the **forward call alone**. It
/// parsed, `reversibility` answered `true`, the gate admitted, the commit was signed, and running
/// the undo gx itself printed left the object **empty** — `rc=0`, with a signed commit receipt
/// beside it. The do/undo round trip passed every fingerprint check gx makes, because those ask
/// "did the object move the way the applied delta said" and never "did it come back to where the
/// forward call found it".
///
/// R18's own sentence is the argument for closing this one: *a read face says where a prior comes
/// from; a template says how a restore call is built out of it, and one without the other is not a
/// declaration*. A template that names no prior is the same claim in the other spelling, and it is
/// the same **file-local** fact — judged here, before a session, with a predicate
/// ([`ArgSource::is_prior`]) this module already had.
///
/// # 🔴 The members that are not the prior and still make an inverse
///
/// `req/298` §1 item 1 asked for `is_prior()` alone. Run against this repository, that spelling
/// refuses **three** declarations that are sound and shipped, so it is not the gate — the
/// widenings are listed here and declared in `req/299` rather than made silently:
///
/// * **The inverse is a deletion**, keyed on what the forward call created:
///   `fixtures/notion-page-catalogue.json` undoes `API-post-page` with `API-delete-a-block` on
///   `{"do_result": "/id"}`, and `fixtures/github-issue-catalogue.json` closes the issue its
///   forward call opened. Neither needs a prior; the id did not exist before the call.
/// * **The inverse is a constant**, because the forward call flipped a field whose other value is
///   known without reading anything: `notion:patch-page {in_trash: true}` is undone by
///   `notion:patch-page {in_trash: false}` — the whole reason [`ArgSource::ConstJson`] exists
///   (**DR-V4B-2**, `req/38` §123 ruling 2). `is_prior() || is_do_result()` refuses it, which the
///   twentieth audit did not measure because `const_json` is in `req/291` §3's *not tried* list.
///
/// So the gate is the sentence this constant already says: a template must **not be a function of
/// the forward call alone**. At least one member is drawn from somewhere else — the prior, the
/// applied call's own result, or a value the declaration itself supplies. An empty template falls
/// to the same test. Where a **read face** is declared the gate stays the narrow one (`is_prior`),
/// because a read face exists to produce the prior and a template beside one that draws no prior
/// is a read gx performs and throws away.
///
/// # 🔴 What this does not close, stated rather than implied
///
/// A constant is a value the operator supplied, and gx does not read it. So
/// `{"uri": {"forward": "uri"}, "note": {"const": "gx undo"}}` passes this gate while carrying
/// nothing of the prior, and if the restore tool needs contents it is not given, the audit's
/// destruction is still reachable — by a declaration one member wider than the one that was
/// measured.
///
/// 🔴 **`req/324` H-01 (`req/38` §231 ruling 1) — and it is a *family*, not that one spelling.**
/// [`ArgSource::draws_from_outside_the_forward_call`] writes `Const` and `ConstJson` on **one
/// arm**, and the twenty-fifth audit drove the second spelling to the same terminal state as the
/// first: accepted, `Admit`, the effect landed, and the printed `gx undo` emptied the object with
/// `rc=0`. The sentence above named one of the two, `docs/LIMITS.md` named the same one, and the
/// correction §230 wrote *in order to record that the residual is spelling-dependent* named it a
/// third time. `do_result` satisfies the same gate and was measured **not** to destroy (undo
/// `rc=1`, object unmoved), so the residual is neither one word nor all six: it is the two that
/// resolve at plan time. `docs/LIMITS.md` v0.5-m carries the width, and
/// `tests/r26_limits_family_sync.rs` is red until the page names every word this classifier
/// classifies. Closing that needs to know **which** member of a restore call is the object's body,
/// which is not a file-local fact and is not in this lane. `docs/LIMITS.md` says so on the page a
/// buyer reads, and `req/299` puts the width of DR-46-19 to Fable.
///
/// # 🔴 What R22 corrected (`req/303` H-01, `req/38` §221 ruling 1)
///
/// Everything above is unchanged and still true. What was wrong was not this sentence but the
/// **predicate** the sentence was implemented as. R20 spelled it `!matches!(source,
/// ArgSource::Forward(_))` — a *negative* set — and [`ArgSource::GitBlobSha1OfForward`] is not
/// `Forward`, so `{"uri": {"forward": "uri"}, "sha": {"git_blob_sha1_of_forward": "contents"}}`
/// passed a gate whose own words are *"a function of the forward call alone"* while being exactly
/// that: that variant's doc comment says the value is *"computable before [the forward call], from
/// the forward arguments alone"*. The twenty-first audit ran it end to end — `Admit`, a signed
/// commit, and the undo gx printed emptied the object with `rc=0`. The predicate is now the
/// **positive** set [`ArgSource::draws_from_outside_the_forward_call`], one arm per variant, and a
/// variant nobody classified answers `false`.
pub const TEMPLATE_NAMES_NO_PRIOR: &str = "so the restore call it builds is a function of the \
     forward call alone: it carries nothing of what the object held before, and applying it hands \
     the restore tool whatever an absent prior renders as rather than the prior. gx answers `true` \
     for such a declaration and the undo it prints can empty the object it claims to restore. What \
     to fix: draw one member from the prior (`\"prior_contents_utf8\"`, or `{\"prior_json\": ...}`) \
     -- or, for a forward call whose inverse is a deletion, from the applied call's own result \
     (`{\"do_result\": ...}`) (`req/291` H-01 / DR-46-19: a template that names no prior is not a \
     declaration of an inverse, by the same argument `req/279` H-01 used for a template that is \
     not there at all)";

/// 🔴 **`req/303` M-03** (`req/38` §221 ruling 5) — which **half of a declaration** a fault is
/// about.
///
/// # The defect this closes, measured
///
/// Every entry fault used to be wrapped in one sentence: *"the **read declaration** of entry X is
/// not sound"*, and on the `invert` road [`crate::DECLARATION_UNSOUND_REFUSAL`] was appended to it,
/// which adds *"The declared read face was never called"*. R18 wrote both when the only entry fault
/// there was **was** about a read face. R20 then added two faults that are not — a `restored_by`
/// that names no tool ([`RESTORE_TOOL_UNNAMED`]) and a template that draws nothing the forward call
/// does not carry ([`TEMPLATE_NAMES_NO_PRIOR`]) — and the twenty-first audit measured the result:
/// a catalogue with **no `read_by` anywhere in it** was refused with a sentence about its read
/// declaration, followed by a claim that a read face that does not exist was never called, followed
/// by a remedy (*correct the read declaration*) the reader cannot execute.
///
/// That is the species `req/269` M-05 and `req/279` M-01 were both ruled defects for: a fault
/// wearing another fault's face. The machine was right in every case — the object did not move and
/// nothing reached the server — and the **record** was wrong, which is the half this product sells.
///
/// # Why an enum rather than a wider sentence
///
/// A single sentence that covered all three would have to say "something about this entry", and
/// then the remedy could not be specific either. Three faces, each with its own subject and its own
/// closing sentence, keep the rule R17 established: the cause first, and only remedies that are
/// executable for *this* cause.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationFace {
    /// The **name** of the restore tool: `restored_by` ([`RESTORE_TOOL_UNNAMED`]).
    RestoreFace,
    /// The `arguments` **template**: what the restore call is built out of
    /// ([`TEMPLATE_NAMES_NO_PRIOR`]).
    Template,
    /// The `read_by` **read face**: where the prior comes from. R18's original subject, and the
    /// only one [`crate::DECLARATION_UNSOUND_REFUSAL`]'s wording is true about.
    ReadFace,
}

impl DeclarationFace {
    /// The subject a wrapping sentence names, without the entry: *"the read declaration"*.
    #[must_use]
    pub fn subject(self) -> &'static str {
        match self {
            DeclarationFace::RestoreFace => "the restore declaration",
            DeclarationFace::Template => "the `arguments` template",
            DeclarationFace::ReadFace => "the read declaration",
        }
    }

    /// Whether a sentence about this fault may speak about a **declared read face**.
    ///
    /// `crate::invert` reads this to choose which closing sentence to append: only
    /// [`DeclarationFace::ReadFace`] has one to speak about.
    #[must_use]
    pub fn is_about_a_read_face(self) -> bool {
        matches!(self, DeclarationFace::ReadFace)
    }
}

/// 🔴 **`req/303` M-03** — one entry fault, with the face it is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclarationFault {
    face: DeclarationFace,
    why: String,
}

impl DeclarationFault {
    /// Build one.
    #[must_use]
    pub fn new(face: DeclarationFace, why: impl Into<String>) -> Self {
        Self {
            face,
            why: why.into(),
        }
    }

    /// Which half of the declaration this is about.
    #[must_use]
    pub fn face(&self) -> DeclarationFace {
        self.face
    }

    /// The cause, in the deployment's own coordinates. This is the string the pre-R22 API returned.
    #[must_use]
    pub fn why(&self) -> &str {
        &self.why
    }
}

impl std::fmt::Display for DeclarationFault {
    /// The cause alone, so that every caller that already had a `String` reads exactly what it read
    /// before this type existed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.why)
    }
}

/// 🔴 **`req/303` L-05 / `req/38` §221 ruling 5-③** — the sentence a tool name spelled with a
/// **combining mark** carries.
///
/// # What was measured
///
/// The twenty-first audit wrote `écrire` (U+00E9, one scalar) and `e` + U+0301 (two scalars) as two
/// keys of one catalogue file and got `declared=2`. The two render identically in every editor and
/// every terminal a reader will open the file in, so the file says one thing to a machine and
/// another to the person who has to approve it. An operator cannot have meant that: "what undoes
/// what" is a decision a human makes by reading, and a declaration a human cannot distinguish from
/// its neighbour is not a decision.
///
/// # Why the refusal is the decomposed spelling rather than the collision
///
/// Catching the *collision* exactly means normalising, and normalising means a Unicode table this
/// crate does not carry (its manifest states why it takes no dependency it does not need, and
/// `Cargo.lock` is outside the lane that wrote this). Refusing the decomposed spelling needs no
/// table: a key that carries a canonical combining mark **has** a composed twin that renders the
/// same, whether or not that twin is also in the file, so refusing it closes the collision from the
/// other side and is decidable from the key's own scalars.
///
/// The trade is stated rather than implied: this refuses a tool name whose *server* spells it
/// decomposed, even when no twin is declared. It is a parse error with a remedy in it, so the cost
/// is one edit at start-up rather than a wrong undo later.
///
/// # 🔴 What this does not close
///
/// The ranges below are the five **combining-diacritical** blocks. Canonical equivalence is wider:
/// Hangul syllable decomposition, Hebrew points, Arabic harakat, Indic matras and *singleton*
/// equivalences (U+212B ANGSTROM SIGN against U+00C5) are **not** covered, and neither is
/// compatibility equivalence (NFKC) or the confusable-script family. `req/303` L-05 also measured a
/// right-to-left override (U+202E) and a zero-width space (U+200B) travelling in a tool name; those
/// remain accepted. `docs/LIMITS.md` says so on the page a buyer reads. Closing the rest needs
/// `unicode-normalization` in this crate's manifest, which is a dependency decision and a
/// `Cargo.lock` edit — named in `req/310` rather than taken silently.
pub const TOOL_NAME_IS_DECOMPOSED: &str = "carries a combining mark, so it is a decomposed \
     spelling of a name that has a composed spelling rendering exactly the same. Two keys that \
     look identical in the file are two declarations an operator cannot tell apart while approving \
     them, and \"what undoes what\" is a decision made by reading. What to fix: spell the tool \
     name in its composed (NFC) form (`req/303` L-05: `\"\\u{00e9}\"` and `\"e\\u{0301}\"` were \
     measured as two declarations of one visible name)";

/// The five Unicode blocks of canonical **combining diacritical marks**, as inclusive scalar ranges.
///
/// Fixed by Unicode's block allocation rather than by a version-dependent property table, which is
/// why they can be written down here. See [`TOOL_NAME_IS_DECOMPOSED`] for what they do and do not
/// cover.
const COMBINING_MARK_BLOCKS: [(u32, u32); 5] = [
    (0x0300, 0x036F), // Combining Diacritical Marks
    (0x1AB0, 0x1AFF), // Combining Diacritical Marks Extended
    (0x1DC0, 0x1DFF), // Combining Diacritical Marks Supplement
    (0x20D0, 0x20FF), // Combining Diacritical Marks for Symbols
    (0xFE20, 0xFE2F), // Combining Half Marks
];

/// 🔴 **`req/312` L-02 (R23)** — whether a declared `$cas_read` prefix **governs** a resource URI.
///
/// Longest-prefix matching was implemented as `resource.starts_with(pattern)`, which is a match on
/// bytes and not on the thing a URI is made of. The audit measured the consequence: a deployment
/// that wrote `doc://page` also governed `doc://pageant/secret` — a **different name space** — and
/// since a CAS read's answer is not yet bound to the object it is about (DR-46-21), the object of
/// the neighbour is read through the declared tool and becomes this object's digest.
///
/// The rule is that the pattern must end where a segment ends. Three ways that is true, and the
/// argument for each:
///
/// * the whole resource is the pattern (an exact declaration);
/// * the pattern already ends on a delimiter — `/` (a path segment, `doc://page/`) or `:` (a
///   scheme, `doc:`) — so what follows it is by construction inside it;
/// * what follows the pattern in the resource starts a new path segment (`doc://page` against
///   `doc://page/1`), which is the case a deployment means when it writes a prefix without a
///   trailing slash.
///
/// `doc://` ends on `/` and therefore still governs every `doc://…`, which is the shipped
/// declaration shape and the negative control for this repair.
#[must_use]
fn prefix_governs(pattern: &str, resource: &str) -> bool {
    let Some(rest) = resource.strip_prefix(pattern) else {
        return false;
    };
    rest.is_empty() || pattern.ends_with('/') || pattern.ends_with(':') || rest.starts_with('/')
}

/// 🔴 **`req/320` L-01 (`req/38` §229 ruling 2)** — the scalars that are invisible at an edge and
/// that `char::is_whitespace` answers `false` for.
///
/// The axis this gate declares is *an edge a reader cannot see*, and R24 implemented it as
/// `char::is_whitespace` — which is a **category**, not that axis. The twenty-fourth audit walked
/// the difference: U+200B and U+FEFF were accepted in all five positions a name is spelled, so
/// `notes.restore` and `notes.restore\u{200b}` stayed two names to the byte comparison and one name
/// to the person approving the file, which is the whole of what `req/316` L-02 was.
///
/// **The width is exactly these five, and they are enumerated rather than described**: the format
/// effectors that render as nothing (U+200B ZERO WIDTH SPACE, U+2060 WORD JOINER, U+FEFF ZERO WIDTH
/// NO-BREAK SPACE) and the two joiners (U+200C, U+200D) which carry meaning **inside** a word in
/// several scripts and none at all at an edge. Everything else stays where `docs/LIMITS.md` puts
/// it: a right-to-left override, and canonical equivalence, are still open and still declared.
pub const INVISIBLE_EDGE_SCALARS: [char; 5] =
    ['\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}'];

/// 🔴 **`req/324` M-03 (`req/38` §231 ruling 2)** — the class the five scalars above are five
/// members of.
///
/// # Why an enumeration was the wrong shape, twice
///
/// R24 implemented *an edge a reader cannot see* as `char::is_whitespace`, which is a **category**
/// and not that axis. R25 widened it to that category plus five named scalars and this file wrote
/// *"the width is exactly these five, and they are enumerated rather than described"*. The
/// twenty-fifth audit then walked twelve more scalars — U+00AD, U+061C, U+115F, U+1160, U+17B4,
/// U+180E, U+200E, U+2061, U+2066, U+3164, U+FFF9, U+E0001 — through the five positions a
/// declaration spells a name in, and the edge gate stopped **none of the sixty cells**. An
/// enumeration is a list of the scalars somebody thought of, and this gate is about the ones
/// nobody did.
///
/// # The class, and why this one
///
/// **Default_Ignorable_Code_Point ∪ General_Category=Cf** (Unicode `DerivedCoreProperties.txt` and
/// `UnicodeData.txt`), beside `char::is_whitespace`. The union rather than either half, because
/// each half misses part of the axis: the Hangul fillers (U+115F, U+1160, U+3164) are `Lo` —
/// *letters* — and are blank on the page, so `Cf` alone does not reach them; the interlinear
/// annotation anchor (U+FFF9) is explicitly **subtracted** from Default_Ignorable, so that property
/// alone does not reach it either. Both halves are properties Unicode publishes, so the width of
/// this gate is a fact a reader can check against the standard rather than against a list this
/// crate maintains.
///
/// # 🔴 What this deliberately does **not** widen
///
/// The class is about scalars that **render as nothing**. It is not about scalars a reader can see:
/// every letter of every script passes, and `crates/gx-adapter-mcp/tests/r26_invisible_edge_axis.rs`
/// drives real Japanese, Chinese, Korean, Arabic, Hebrew, Cyrillic and Devanagari tool names
/// through all five positions as the negative control `req/325` item 4 makes HARD. A class one
/// careless range too wide would refuse names half the world writes, which is a worse defect than
/// the one it repairs.
///
/// Canonical equivalence and NFKC confusables stay open and stay declared: they need a
/// normalisation table this build does not carry, and they are not this axis. A right-to-left
/// override (U+202E) is in the class and so is closed **at either edge** — inside a name it is
/// still accepted, exactly as U+200B is, and `docs/LIMITS.md` says so.
const INVISIBLE_SCALAR_RANGES: [(char, char); 25] = [
    ('\u{00AD}', '\u{00AD}'),
    ('\u{034F}', '\u{034F}'),
    ('\u{0600}', '\u{0605}'),
    ('\u{061C}', '\u{061C}'),
    ('\u{06DD}', '\u{06DD}'),
    ('\u{070F}', '\u{070F}'),
    ('\u{0890}', '\u{0891}'),
    ('\u{08E2}', '\u{08E2}'),
    ('\u{115F}', '\u{1160}'),
    ('\u{17B4}', '\u{17B5}'),
    ('\u{180B}', '\u{180F}'),
    ('\u{200B}', '\u{200F}'),
    ('\u{202A}', '\u{202E}'),
    ('\u{2060}', '\u{206F}'),
    ('\u{3164}', '\u{3164}'),
    ('\u{FE00}', '\u{FE0F}'),
    ('\u{FEFF}', '\u{FEFF}'),
    ('\u{FFA0}', '\u{FFA0}'),
    ('\u{FFF0}', '\u{FFFB}'),
    ('\u{110BD}', '\u{110BD}'),
    ('\u{110CD}', '\u{110CD}'),
    ('\u{13430}', '\u{1343F}'),
    ('\u{1BCA0}', '\u{1BCA3}'),
    ('\u{1D173}', '\u{1D17A}'),
    ('\u{E0000}', '\u{E0FFF}'),
];

/// Whether `c` is a scalar that renders as nothing — see [`INVISIBLE_SCALAR_RANGES`].
#[must_use]
pub fn is_an_invisible_scalar(c: char) -> bool {
    INVISIBLE_SCALAR_RANGES
        .iter()
        .any(|(lo, hi)| c >= *lo && c <= *hi)
}

/// 🔴 **`req/329` M-02 (`req/38` §233 ruling 3)** — the scalars whose appearance the standard does
/// not fix: `General_Category=Cc` and `General_Category=Co`.
///
/// # Why this is a second predicate rather than more rows in the table above
///
/// [`is_an_invisible_scalar`] answers *does this render as nothing*, and that is a claim about
/// glyphs. Exactly one of these two categories can be described that way. Folding both into it
/// would make the older predicate's name false, and it is `pub`.
///
/// * **`Cc`** (U+0000–U+001F, U+007F–U+009F) does render as nothing, or as a control picture. R26's
///   class missed all of it — `Cc` is not `Cf` and is not Default_Ignorable — except the six members
///   `char::is_whitespace` happens to answer for, which is why U+0085 was refused and U+0007 was
///   taken. That split had nothing to do with the axis the gate names for itself.
/// * **`Co`** (the three private-use areas) is not a claim of invisibility, and this crate does not
///   make one: a private-use scalar renders as whatever a private agreement says. The question the
///   catalogue asks is whether a person approving a file can read the name it declares, and a name
///   whose glyph is outside the standard is not a fact the page carries. The table above stops at
///   `E0000-E0FFF`, which is plane 14's tag range — watch the digit count against the BMP area
///   `E000-F8FF`, and note that neither supplementary plane was reached either.
///
/// Both are categories Unicode publishes, so the width of the gate stays a fact a reader checks
/// against the standard rather than against a list this crate maintains — which was the whole of
/// R26's argument for a class, applied to the part of it R26 did not reach.
const NO_ASSIGNED_APPEARANCE_RANGES: [(char, char); 5] = [
    ('\u{0000}', '\u{001F}'),
    ('\u{007F}', '\u{009F}'),
    ('\u{E000}', '\u{F8FF}'),
    ('\u{F0000}', '\u{FFFFD}'),
    ('\u{100000}', '\u{10FFFD}'),
];

/// Whether `c` is a scalar the standard fixes no appearance for — see
/// [`NO_ASSIGNED_APPEARANCE_RANGES`].
#[must_use]
pub fn has_no_assigned_appearance(c: char) -> bool {
    NO_ASSIGNED_APPEARANCE_RANGES
        .iter()
        .any(|(lo, hi)| c >= *lo && c <= *hi)
}

/// Whether `c` is invisible when it sits at the edge of a name: whitespace, or a scalar that
/// renders as nothing ([`INVISIBLE_SCALAR_RANGES`]).
///
/// 🔴 [`INVISIBLE_EDGE_SCALARS`] is kept beside this and is still true — those five are five
/// members of the class — because it is `pub` and because the record of what R25 closed is not the
/// record of what this closes.
#[must_use]
fn is_invisible_at_an_edge(c: char) -> bool {
    // 🔴 **`req/329` M-02 (`req/38` §233 ruling 3)** — the third half, and why it is here rather
    // than inside [`is_an_invisible_scalar`]: see [`NO_ASSIGNED_APPEARANCE_RANGES`]. This is the
    // one predicate the trim and the tests both use, so widening it widens both together.
    c.is_whitespace() || is_an_invisible_scalar(c) || has_no_assigned_appearance(c)
}

/// 🔴 **`req/316` L-02 (R24), widened by `req/320` L-01 (R25)** — whether a name has an edge a
/// reader cannot see.
///
/// `char::is_whitespace` rather than `u8::is_ascii_whitespace`: U+00A0 and U+3000 are as invisible
/// as U+0020. 🔴 And [`INVISIBLE_EDGE_SCALARS`] beside it, because the axis the doc declares is not
/// the `White_Space` property — see that constant for the audit that measured the gap. See
/// [`TOOL_NAME_CARRIES_EDGE_WHITESPACE`] for what this does and does not close.
///
/// # 🔴 A name that is **only** invisible characters is not this fault
///
/// It is the *unnamed* fault, and this crate already has three sentences for it
/// ([`RESTORE_TOOL_UNNAMED`], [`CAS_READ_TOOL_UNNAMED`], and `req/279` L-03's read-face form).
/// Answering `true` here would put this sentence in front of all three, and a reader whose file
/// says `"restored_by": "   "` would be told to trim a name they never wrote instead of to write
/// one. The full-workspace floor is what found that: five arms that assert those three sentences
/// went red, and every one of them was right to. The widening keeps that property by construction —
/// the trim uses the same predicate the test does — so `"\u{200b}"` alone is still *unnamed*.
#[must_use]
pub fn carries_edge_whitespace(name: &str) -> bool {
    let trimmed = name.trim_matches(is_invisible_at_an_edge);
    !trimmed.is_empty() && trimmed != name
}

/// 🔴 **`req/320` L-01 (R25)** — whether a declared name names **nothing**, on the same axis
/// [`carries_edge_whitespace`] draws.
///
/// The three *unnamed* gates in this file asked `str::trim().is_empty()`, which is the `White_Space`
/// property -- so `"   "` named no tool and a name of only zero-width scalars named one. Both render
/// as nothing on the page an operator approves, and the two predicates have to be the **same**
/// predicate or widening one opens a hole in the other: an all-invisible name would be neither
/// *unnamed* (its `trim()` is not empty) nor *an invisible edge* (nothing is left after the edges
/// come off). That hole was measured on this build before this sweep, and `req/38` §227 ruling 2 is
/// the standing rule it is closed under: the same question, asked in one place, by every gate.
#[must_use]
fn names_nothing(name: &str) -> bool {
    name.trim_matches(is_invisible_at_an_edge).is_empty()
}

/// Whether a name carries a scalar from [`COMBINING_MARK_BLOCKS`].
#[must_use]
pub fn carries_a_combining_mark(name: &str) -> bool {
    name.chars().any(|c| {
        let c = c as u32;
        COMBINING_MARK_BLOCKS
            .iter()
            .any(|(lo, hi)| c >= *lo && c <= *hi)
    })
}

/// 🔴 **`req/38` §227 ruling 1** — why [`Catalogue::writes_per_this_file`] answered yes.
///
/// The two sets are both this file talking about itself, and a gate that refuses needs to say
/// which one it fell into: the sentences differ, and a reader who is told "an effect" about a
/// `restored_by` value has been told something the file does not say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritesBecause<'a> {
    /// A **key** of `restores`: a tool this catalogue declares as an effect that needs undoing.
    Effect,
    /// A **value** of `restored_by`: the call that puts an object back, named as the inverse of
    /// the effect carried here. A restore tool writes by construction.
    Inverse {
        /// The effect this tool is declared to undo.
        of: &'a str,
    },
}

/// One declaration: which tool undoes this one, and (optionally) how its arguments are built.
///
/// `deny_unknown_fields` because this value configures "what undoes what": a misspelt (sem: SEM-gx-adapter-mcp-135)
/// `"arguments"` silently becoming the legacy form would be a declaration the operator did not
/// make, discovered at the first failed undo instead of at parse time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RestoreSpec {
    restored_by: String,
    /// `None` is the v0.1 `{contents, uri}` convention, unchanged -- the module doc's second
    /// section says why both forms exist.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    arguments: Option<RestoreTemplate>,
    /// 🔴 **DR-46-9 A-3**: `None` is `resources/read`, which is every catalogue written before
    /// this window and the whole of the fs/notes/github-contents road (backward compatible by
    /// construction: the member is absent, so the bytes of a shipped catalogue file parse to the
    /// same value they always did).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    read_by: Option<PriorRead>,
}

impl RestoreSpec {
    /// The restore tool's name.
    #[must_use]
    pub fn restored_by(&self) -> &str {
        &self.restored_by
    }

    /// The template, when the deployment declared one.
    #[must_use]
    pub fn template(&self) -> Option<&RestoreTemplate> {
        self.arguments.as_ref()
    }

    /// 🔴 Where the prior comes from, when the deployment declared a read tool for it
    /// (**DR-46-9 A-3**). `None` means `resources/read`.
    #[must_use]
    pub fn read_by(&self) -> Option<&PriorRead> {
        self.read_by.as_ref()
    }

    /// 🔴 **`req/279` H-01** — what can be judged about **this one declaration** without a call.
    ///
    /// # The hole this closes, measured
    ///
    /// [`Self::arguments`] is optional and `None` means v0.1's `{contents, uri}` convention, which
    /// is right for a restore tool that takes a resource's bytes and *wrong* for every tool-only
    /// road. A declaration that names a `read_by` and no template took that fallback: `invert`
    /// built `restore_arguments(position.resource(), &contents)` where `contents` is **the read
    /// tool's answer** — a JSON document about the object, not the object.
    ///
    /// The nineteenth audit measured the consequence with no adversary in it. The catalogue
    /// parsed, `gx wrap` started, the verdict was `true`, and the escrow was an 87-byte call to
    /// `doc.restore` carrying `{"id":"doc:d1","text":"the document as it was\n","etag":"w/123"}`.
    /// Applying that escrow left the object holding the *answer document* where its text had been.
    /// DR-46-15's identity check passed the whole way, correctly: the read really was about this
    /// object. Nobody asked whether what came back was a **prior**.
    ///
    /// So the two members are a pair. A read face says where the prior comes from; a template says
    /// how a restore call is built out of it. Declaring the first without the second is declaring
    /// that a document is a body, and it is a file-local fact — judged here, before a session.
    ///
    /// # Errors
    /// A human sentence. [`Catalogue::from_json`] turns it into a parse error and
    /// [`crate::invert`] refuses on it for a catalogue built in code (`req/269` M-05's argument:
    /// a value that never met a parser is a value the parser never checked).
    pub fn soundness(&self) -> core::result::Result<(), String> {
        self.fault().map_err(|fault| fault.why().to_string())
    }

    /// 🔴 **`req/303` M-03** — [`Self::soundness`], with the **face** each fault is about.
    ///
    /// The same predicate in the same order; what is added is [`DeclarationFace`], so that the
    /// sentence a caller wraps this in can name the half of the declaration that is wrong.
    /// [`Self::soundness`] is this function with the face dropped, which is what every caller
    /// written before R22 reads.
    ///
    /// # Errors
    /// The first fault, with its face.
    pub fn fault(&self) -> core::result::Result<(), DeclarationFault> {
        // 🔴 **`req/291` M-04** — the name comes before everything the name is called with, and it
        // is asked first for that reason: a declaration with no restore tool has nothing for the
        // rest of these questions to be about.
        if names_nothing(&self.restored_by) {
            // 🔴 `req/303` M-03: the **restore** face. This entry may declare no read face at all,
            // and a fault about a missing restore-tool name is not a fault about a read.
            return Err(DeclarationFault::new(
                DeclarationFace::RestoreFace,
                format!(
                    "the declaration's `restored_by` is {:?}, {RESTORE_TOOL_UNNAMED}",
                    bounded(&self.restored_by)
                ),
            ));
        }
        match (self.read_by(), self.arguments.as_ref()) {
            // R18's `req/279` H-01, unchanged and still first for the read-face road: a read with
            // no template is a different fault from a template with no prior, and the two say so.
            (Some(read), None) => {
                return Err(DeclarationFault::new(
                    DeclarationFace::ReadFace,
                    format!(
                    "it names the read face {:?} and no `arguments` template, so the escrow would \
                     fall back to the v0.1 `{{contents, uri}}` convention and carry the read \
                     tool's own answer document as this object's prior contents. What to fix: \
                     declare the `arguments` the restore tool takes, built from the forward call \
                     and the prior (`req/279` H-01: a read face says where a prior comes from; a \
                     template says how a restore call is built out of it, and one without the \
                     other is not a declaration)",
                    read.by_tool()
                ),
                ));
            }
            // 🔴 **`req/291` H-01 / DR-46-19** — the template exists; does it draw on anything the
            // forward call does not already carry?
            (read_face, Some(template)) => {
                let carries_prior = template.arguments().values().any(ArgSource::is_prior);
                // A read face exists to *produce* the prior. A template beside one that draws no
                // prior member is a read gx performs and discards, so the wider test below is not
                // offered on that road.
                let sound = if read_face.is_some() {
                    carries_prior
                } else {
                    // 🔴 The test is the one [`TEMPLATE_NAMES_NO_PRIOR`] states: **not** a function
                    // of the forward call alone. See that constant's "the members that are not the
                    // prior and still make an inverse" for why it is this and not `is_prior`.
                    //
                    // 🔴 **`req/303` H-01** — asked as a **positive** set
                    // ([`ArgSource::draws_from_outside_the_forward_call`]) rather than as
                    // `!matches!(source, ArgSource::Forward(_))`. The negative spelling admitted
                    // `git_blob_sha1_of_forward`, which that variant's own doc calls
                    // *derivable-from-forward*: a template of forward members and a hash of one of
                    // them is a function of the forward call alone, and it emptied an object under
                    // a signed commit.
                    template
                        .arguments()
                        .values()
                        .any(ArgSource::draws_from_outside_the_forward_call)
                };
                if !sound {
                    let members: Vec<&str> =
                        template.arguments().keys().map(String::as_str).collect();
                    let declared = if members.is_empty() {
                        "it declares an `arguments` template with no members at all".to_string()
                    } else {
                        format!(
                            "it declares an `arguments` template whose members are all built from \
                             the forward call ({})",
                            bounded(&members.join(", "))
                        )
                    };
                    let face = match read_face {
                        Some(read) => format!(
                            " and names the read face {:?}, whose answer nothing in the template \
                             draws on",
                            read.by_tool()
                        ),
                        None => String::new(),
                    };
                    // 🔴 `req/303` M-03: the face is the **template** unless a read face is what
                    // makes the template wrong. Where one is declared, the fault is that the read
                    // it performs is thrown away — a fact about the read face — and the sentence
                    // above already names it.
                    let subject = if read_face.is_some() {
                        DeclarationFace::ReadFace
                    } else {
                        DeclarationFace::Template
                    };
                    return Err(DeclarationFault::new(
                        subject,
                        format!("{declared}{face}, {TEMPLATE_NAMES_NO_PRIOR}"),
                    ));
                }
            }
            // The v0.1 `{contents, uri}` convention: no template, no read face. Unchanged — the
            // prior travels by construction on that road, which is why it needs no member to say so.
            (None, None) => {}
        }
        match self.read_by() {
            Some(read) => read
                .soundness()
                .map_err(|why| DeclarationFault::new(DeclarationFace::ReadFace, why)),
            None => Ok(()),
        }
    }
}

/// 🔴 **DR-46-12** (`req/38` §196): what an escrow does when the prior will not be read.
///
/// The two answers are not symmetric and the default is the conservative one. `Refuse` is what
/// every deployment already had — `ToolTransport::read`'s `Err` has always travelled out of
/// `invert` and failed the commit closed, so declaring nothing keeps the behaviour byte for byte.
/// `Unknown` is the deployment saying, in writing, that it would rather have the effect than the
/// escrow; the reversibility of that transformation is then [`Reversibility::Unknown`] and the
/// escrow row is `Unavailable` — an inverse was asked for and there is none.
///
/// Making the **relaxation** the opt-in is the whole ruling: a default that let effects through
/// when a network read failed would turn "gx escrows before it applies" into a claim that is true
/// only while the network is up, and nobody would be told which runs were which.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnReadFailure {
    /// Refuse the effect. The default, and the pre-existing behaviour.
    #[default]
    Refuse,
    /// Take the effect, and record the reversibility as [`Reversibility::Unknown`].
    Unknown,
}

impl OnReadFailure {
    /// The word a catalogue file spells, for a start-up line and a report line.
    ///
    /// 🔴 **`req/269` M-01**: the posture is a property of the **whole catalogue** and it moves
    /// the fail-closed default on **every** declaration in the file, including declarations that
    /// name no read tool at all and take the `resources/read` road they always took. Until this
    /// window it was printed nowhere, so an operator could not see that one line had tipped a
    /// whole file. `gx wrap`'s start-up JSON carries this value beside `restorable_tools`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            OnReadFailure::Refuse => "refuse",
            OnReadFailure::Unknown => "unknown",
        }
    }
}

// 🔴 **DR-46-26** -- `pub enum Reversibility` and its `ALL`/`as_str` stood here, from DR-46-9 A-4
// until the widening of `SubstrateAdapter::invert`. It is now [`gx_core::Reversibility`], and the
// move is **forced rather than preferred**: this crate depends on `gx-substrate`, so the trait that
// now returns the value cannot name a type declared here without a dependency cycle -- and
// `gx-witness`, which seats the value on a receipt (`req/38`'s S1 ruling 5), does not depend on
// `gx-substrate` at all. `gx-core` is the one crate both of them already name.
//
// Nothing about the value changed: three values, the same `ALL` in C-25's order, the same
// `as_str`. `crate::invert::invert_with_verdict` is still the only thing in the workspace that
// works the answer out, which is the half of "the data comes down, the computation stays up" that
// stays up here.
pub use gx_core::Reversibility;

/// 🔴 **DR-46-28**: the declaration face names the same two types the attest face does.
pub use gx_core::{BoundaryStage, DeterminismBoundary};

/// 🔴 **DR-V4B-2b** (`req/38` §123 ruling 2, `req/189`): the catalogue file's one reserved
/// metadata slot — `"$server"` — where a deployment pins which server (and version) the
/// declarations were written against.
///
/// A catalogue is a claim about a specific server's tools ("`create_or_update_file` is undone by
/// `create_or_update_file` with these arguments" is true of github-mcp-server v1.9.0 and of nothing
/// in general); until this slot existed the file could not say which. `$`-prefixed so that it can
/// never collide with a tool name (MCP tool names do not begin with `$`), and read as **metadata**
/// rather than as a `RestoreSpec` — which is what keeps `RestoreSpec`'s `deny_unknown_fields`
/// honest: the map's *values* still refuse a misspelt member; only this one *key* is not a tool.
/// The contents are free-form (`{"name": ..., "version": ...}` by convention) and are carried,
/// never interpreted: gx does not verify the pin against the live server (a `tools/list` cross-check
/// is `req/182` M-25's candidate box, not this window's).
pub const SERVER_METADATA_KEY: &str = "$server";

/// 🔴 **DR-46-12** (`req/38` §196): the catalogue file's **second** reserved slot —
/// `"$on_read_failure"` — where a deployment opts out of the fail-closed default.
///
/// `"refuse"` (the default, and what an absent slot means) or `"unknown"`; anything else is a
/// parse error rather than a value quietly read as the default, for [`SERVER_METADATA_KEY`]'s
/// reason one slot over — a misspelt relaxation that silently meant "refuse" would be a
/// deployment believing it had opted in. `$`-prefixed so it can never collide with a tool name.
pub const ON_READ_FAILURE_KEY: &str = "$on_read_failure";

/// 🔴 **DR-46-28** (`req/38` §255 ruling 4, `req/459` ruling 1) — the catalogue file's **fourth**
/// reserved slot: `"$determinism_boundary"`, where a deployment declares **where its inputs come
/// from**.
///
/// # Why the slot declares one stage and not the boundary
///
/// `req/459` splits the work in two: the catalogue **declares** and the receipt **attests**. The
/// boundary itself has two stages — input generation and verdict derivation — and only one of them
/// is a deployment's to speak about. The other is gx's own arithmetic: a file that could declare
/// "gx's verdict derivation is replay-deterministic" would be a self-claim gx then signed, which is
/// the shape `req/444` §1 refuses (its ban on over-claiming). So this slot carries a
/// [`gx_core::BoundaryStage`] — one of `"deterministic_replay"`, `"llm_originated"`,
/// `"unknown"` — and [`Catalogue::declared_boundary`] is the one place the two halves meet.
///
/// An absent slot means `"unknown"`, which is what every catalogue written before this window meant
/// without having a way to say it — and `unknown` is a **value** here rather than a silence
/// (`req/459` ruling 3). A value that is not one of the three words is a parse error for
/// [`ON_READ_FAILURE_KEY`]'s reason one slot over: a misspelt declaration that quietly meant
/// "unknown" would be a deployment believing it had said where its inputs come from. `$`-prefixed
/// so it can never collide with a tool name.
pub const DETERMINISM_BOUNDARY_KEY: &str = "$determinism_boundary";

/// 🔴 **DR-46-16** (`req/38` §218 ruling 1, design fork (b)) — the catalogue file's **third**
/// reserved slot: `"$cas_read"`, where a deployment says how the **compare-and-set half** reads an
/// object on a server that publishes no `resources/read` face for it.
///
/// # What was open, and where it was open
///
/// DR-46-9 A-3 gave the **escrow** half a second road: a declaration naming a read tool, taken
/// *instead of* `resources/read` ([`PriorRead`], `crate::invert`). The crate root has said since
/// that window that the other half did not move — [`crate::McpAdapter::snapshot`],
/// `precondition` and the post-apply observation went through `resources/read` unconditionally,
/// so a **tools-only** server was still one `gx wrap` refuses to plan on however complete its
/// escrow declaration was. `tests/notion_page_catalogue.rs` carries that measurement on the real
/// notion-mcp-server (`initialize` → `{"tools":{}}`, `resources/read` → `-32601`). This slot is
/// that half, and `req/38` §123 ruling 1 (b)'s open ground closes **for declared locators only**.
///
/// # Why this is a top-level slot and not a member of [`RestoreSpec`]
///
/// A restore declaration is keyed by **forward tool**, and the three CAS positions have no forward
/// tool to be keyed by: `snapshot(locator)` and `precondition(snap)` receive a locator and nothing
/// else, and 41 §4's seven-method boundary (**N-08**, `gx-substrate/tests/adapter_spec.rs`) is
/// what says they never will. So the declaration is keyed by what those two *do* carry — the
/// **resource URI** — and the whole of it is resolved inside `McpAdapter` (`crate::cas`), which is
/// the reason design fork (b) was adopted over widening the trait: nothing here reaches the frozen
/// face.
///
/// # The key is a prefix, and the longest one wins
///
/// A deployment cannot enumerate its objects — `notion://page/<uuid>` is one URI per page — so an
/// exact-match key would be a declaration nobody can write. The key is a **prefix of the
/// normalised resource URI**, and when several match, the **longest** does. Ties are impossible
/// (two distinct prefixes of one string cannot share a length), so the choice is a function of the
/// file and not of the map's order.
///
/// ```text
/// "$cas_read": {
///   "notion://page/": {
///     "by_tool": "API-retrieve-a-page",
///     "arguments": { "page_id": "resource_suffix" }
///   }
/// }
/// ```
///
/// # 🔴 What this slot does **not** do, said here rather than found later
///
/// * A locator matching **no** pattern takes `resources/read` exactly as it always did, and a
///   tools-only resource with no pattern is still refused at plan time. The slot unlocks the
///   locators an operator wrote down and no others (`docs/LIMITS.md`).
/// * It does **not** bind the read's answer to the object, the way [`ObjectIdentity`] binds the
///   escrow half's (DR-46-15). That symmetric gap is **DR-46-21** (`req/38` §218 ruling 2,
///   `req/305`) and is one lane along; until it lands, a CAS read tool that answers about another
///   object produces this object's digest out of that object's bytes. `docs/LIMITS.md` says so on
///   the page a buyer reads.
pub const CAS_READ_KEY: &str = "$cas_read";

/// 🔴 **DR-46-16** — the sentence a CAS read declaration that names no tool carries.
///
/// The third member of the family `req/279` L-03 and `req/291` M-04 opened: a read face has a name
/// before it has arguments, and so does a restore face, and so does this. The audit measurement
/// behind L-03 was an empty tool name reaching the wire and the server answering `-32602`; on this
/// road it would reach the wire from `snapshot`, which runs **before a plan exists**, so the harm
/// is one gate earlier than the one that was measured.
pub const CAS_READ_TOOL_UNNAMED: &str = "which names no tool: a compare-and-set read face has a \
     name before it has arguments, and a nameless one reaches the wire from `snapshot` -- before \
     this transformation has a plan, let alone an admission. What to fix: name the read tool, or \
     drop the `$cas_read` entry and let this locator take `resources/read` (DR-46-16, the third \
     member of `req/279` L-03's family)";

/// 🔴 **DR-46-16** — the sentence an empty `$cas_read` pattern carries.
///
/// The empty string is a prefix of every resource URI, so an empty key is not a declaration about
/// some locators: it is a declaration that **every** object on this deployment is read through one
/// tool, written in the one spelling that looks like it says nothing. A deployment that means that
/// can spell its scheme (`"notion://"`), which is a prefix a reader can check.
pub const CAS_READ_PATTERN_EMPTY: &str =
    "is the empty string, which is a prefix of every resource \
     URI: it does not declare a CAS read face for some locators, it declares one for all of them, \
     in the spelling least likely to be read that way. What to fix: spell the prefix the \
     deployment means (a scheme, `\"notion://\"`, or a whole URI) (DR-46-16)";

/// 🔴 **`req/324` M-04 (`req/38` §231 ruling 2)** — the sentence a `$cas_read` prefix made of
/// nothing but invisible scalars carries.
///
/// Slot 4 of the five, and one of the two nobody had swept. [`CAS_READ_PATTERN_EMPTY`] refuses the
/// prefix that *is* the empty string; a prefix of three zero-width joiners is the empty string on
/// the page an operator approves and a three-scalar key to the parser, so it took the entry and
/// then governed no locator at all. The remedy is the empty prefix's remedy because the fault a
/// reader has is the empty prefix's fault: the file does not say which locators this face is for.
pub const CAS_READ_PREFIX_NAMES_NOTHING: &str =
    "which names nothing: every scalar in it renders as nothing, so on the page this file is \
     approved on it is the empty string -- and the empty string is a prefix of every resource URI. \
     To the parser it is a three-character key that matches no URI at all, so the declared read \
     road is silently not taken. What to fix: spell the prefix the deployment means (a scheme, \
     `\"notion://\"`, or a whole URI), or drop the entry (`req/324` M-04, the fourth of the five \
     slots a name is declared in)";

/// 🔴 **`req/324` M-04 (`req/38` §231 ruling 2)** — the sentence an **effect** tool name made of
/// nothing but invisible scalars carries.
///
/// Slot 5 of the five, and the other one nobody had swept. [`RESTORE_TOOL_UNNAMED`] is the same
/// fault on the right-hand side of the colon; this is the key. `req/38` §227 ruling 2's standing
/// rule is that the same question is asked in one place by every gate, and a name that is blank on
/// the page is the same question wherever the name sits.
pub const EFFECT_TOOL_UNNAMED: &str =
    "which names no tool: every scalar in it renders as nothing, \
     so the entry declares what undoes a tool whose name is blank on the page this file was \
     approved on. The sets this file draws about itself are compared by their bytes, so such a key \
     is a tool to gx and no tool to its reader. What to fix: name the effect tool, or drop the \
     entry (`req/324` M-04, the fifth of the five slots a name is declared in)";

/// 🔴 **DR-46-16** — the sentence a CAS read face that this same catalogue declares as an
/// **effect** carries.
///
/// [`Catalogue::entry_soundness`]'s `req/279` M-04, one road over and one gate earlier. There the
/// contradiction reached the wire through the escrow road, which carries no [`crate::Admitted`]
/// and runs before `apply`; here it reaches the wire through `snapshot`, which runs before the
/// transformation is even planned — so a file that says both is asking gx to make an unadmitted
/// change while it is working out what the world currently holds.
pub const CAS_READ_FACE_IS_AN_EFFECT: &str = "and this same catalogue declares that tool as an \
     effect that needs undoing. gx calls a CAS read face from `snapshot` and `precondition`, which \
     carry no admission and run before this transformation is planned, so a file that says both is \
     asking gx to make an unadmitted change while it is establishing what the object currently \
     holds. What to fix: name a read face this catalogue does not declare as an effect (DR-46-16, \
     the `snapshot` road's form of `req/279` M-04)";

/// 🔴 **`req/312` H-01 (R23)** — the sentence a CAS read face this same catalogue names as an
/// **inverse** carries.
///
/// [`CAS_READ_FACE_IS_AN_EFFECT`]'s other half, and the hole the twenty-second adversarial audit
/// drove through it. That gate asked one question — is this tool a **key** of `restores`? — and a
/// catalogue file names the tools it writes with in **two** places. The second is `restored_by`:
/// the call that puts an object back, which is a write by construction and which this file says so
/// about itself. So a deployment could follow the remedy [`CAS_READ_FACE_IS_AN_EFFECT`] prints,
/// *verbatim* — "name a read face this catalogue does not declare as an effect" — and arrive at
/// exactly the harm that constant names.
///
/// What was measured on a real road (`req/312` §1 H-01): `snapshot` called the restore tool six
/// times before the agent's own effect reached the server, and the document was empty afterwards.
/// On the admit road the commit was signed and the undo gx printed did not recover it; on a road
/// where gx **refused before a verdict** — zero receipts, the agent's call never sent — the object
/// was destroyed anyway, and the sentence the agent was handed said *nothing was sent to the
/// server*.
pub const CAS_READ_FACE_IS_AN_INVERSE: &str = "a restore tool is the call that puts an object \
     back, so it writes by construction, and this file is the thing that says so. gx calls a CAS \
     read face from `snapshot` and `precondition`, which carry no admission and run before this \
     transformation is planned, so a file that says both is asking gx to write the object while it \
     is establishing what that object currently holds. What to fix: name a read face this \
     catalogue declares neither as an effect nor as the inverse of one (`req/312` H-01, the half \
     of DR-46-16's soundness gate that only asked about `restores` keys)";

/// 🔴 **`req/316` L-02 (R24)** — the sentence a tool name with whitespace at an edge carries.
///
/// The soundness sets [`Catalogue::writes_per_this_file`] compares are drawn by **byte** equality,
/// and the twenty-third audit walked through that: a catalogue declaring `notes.restore` as an
/// inverse and `"notes.restore "` as a read face was accepted, because those are two byte strings.
/// A trailing space is not visible in the file an operator approves, and it is not visible in the
/// refusal a gate would have printed, so the two spellings are one name to every reader and two
/// names to the gate.
///
/// This closes the whitespace axis of that finding, and only that axis. The other axis the audit
/// measured — spellings that are **canonically equivalent** under Unicode without being equal as
/// bytes, `notes.wr\u{212b}ite` against `notes.wr\u{c5}ite` — needs a normalisation table this
/// crate does not carry, and `req/38` §224 ruling 2 put that behind a queued lane rather than
/// inside this one. `docs/LIMITS.md` says which of the two is closed.
pub const TOOL_NAME_CARRIES_EDGE_WHITESPACE: &str =
    "starts or ends with whitespace or with a zero-width \
     scalar. The sets this \
     file draws about itself — the tools it calls effects, and the tools it names as inverses — \
     are compared by their bytes, so a name with an invisible edge is a second name to gx and one \
     name to the person who approved the file: a read face spelled with a trailing space walks \
     past the gate that refuses the same tool spelled without one. What to fix: spell the tool \
     name with no leading or trailing whitespace, no zero-width scalar and no scalar the \
     standard fixes no appearance for — the control and private-use categories — at either end \
     (`req/316` L-02, widened by `req/320` L-01 and by `req/329` M-02)";

/// 🔴 **`req/320` M-04 (`req/38` §229 ruling 2)** — the sentence a `$cas_read` **prefix** with an
/// invisible edge carries.
///
/// # The quietest of the five positions
///
/// R24 closed [`TOOL_NAME_CARRIES_EDGE_WHITESPACE`] in the four positions a **tool name** is
/// spelled. Its sibling gate — `req/312` M-02's decomposition check — walks **five**, and the fifth
/// is the `$cas_read` key. The twenty-fourth audit walked into the gap: a prefix with a leading
/// space parsed, and then `prefix_governs` matched **no locator at all**, so the declared
/// tools-only read road was silently not taken and every read fell back to `resources/read`
/// (`A24_WS_PREFIX accepted=true governs_a_matching_locator=false`).
///
/// That is the failure mode `from_json` already names elsewhere in this file — *a deployment that
/// believes it opted in and did not* — and it is worse here than in the four tool-name positions
/// for one reason: a misspelt **tool name** fails loudly at call time, and a misspelt **prefix**
/// fails by nothing happening. On a server with no `resources/read` face the fall-back cannot
/// answer either, so the deployment gets a refusal about the substrate rather than about its file.
pub const CAS_READ_PREFIX_CARRIES_EDGE_WHITESPACE: &str =
    "starts or ends with whitespace or with a zero-width scalar. A prefix is matched against \
     resource URIs by its bytes, and a URI does not begin with a space — so a prefix with an \
     invisible edge governs nothing, this file's declared read road is silently not taken, and \
     every read falls back to `resources/read` on a deployment that believes it opted in. That is \
     the one fault in this file which produces no error at the moment it matters. What to fix: \
     spell the prefix with no leading or trailing whitespace, no zero-width scalar and no scalar \
     the standard fixes no appearance for — the control and private-use categories — at either \
     end (`req/320` M-04, widened by `req/329` M-02)";

/// 🔴 **`req/316` L-01 (R24)** — the sentence a declaration written as a JSON **array** carries.
///
/// `serde`'s derived `Deserialize` accepts a struct as a sequence as well as a map, so
/// `{"notes.write": ["notes.restore", {…}, null]}` and `{"$cas_read": {"doc://": ["doc.read", …]}}`
/// both parsed, and the twenty-third audit measured the second one reaching the real read road.
/// `#[serde(deny_unknown_fields)]` does not reach this: its argument is about a **misspelt member
/// name** silently becoming a declaration the operator did not make, and a spelling with no member
/// names at all is the same argument with the volume turned up — the file says nothing about which
/// slot each value landed in, and the reader's only way to know is to count the fields of a Rust
/// struct they cannot see.
pub const DECLARATION_IS_POSITIONAL: &str =
    "is a JSON array. A declaration in this file is a JSON \
     object whose member names say which slot each value fills; written as an array the slots are \
     decided by position in a Rust struct the operator approving this file cannot see, and a value \
     that lands in the wrong one is a declaration nobody made. What to fix: write the declaration \
     as a JSON object with named members (`req/316` L-01)";

/// 🔴 **`req/312` M-02 (R23)** — the sentence a decomposed `$cas_read` **prefix** carries.
///
/// [`TOOL_NAME_IS_DECOMPOSED`]'s argument about a key an operator approves by reading, one slot
/// over and about a resource-URI prefix rather than a tool name. The audit wrote `doc://café/`
/// (U+00E9) and the decomposed spelling of the same thing into one `$cas_read` map, and got **two**
/// declarations that render as one line — and which of them governs an object is decided by a byte
/// that does not appear on the page. The width is [`TOOL_NAME_IS_DECOMPOSED`]'s width, for its
/// reason.
pub const CAS_READ_PREFIX_IS_DECOMPOSED: &str = "carries a combining mark, so it is a decomposed \
     spelling of a prefix that has a composed spelling rendering exactly the same. Two `$cas_read` \
     keys that look identical in the file are two reading roads an operator cannot tell apart \
     while approving them, and which one governs an object is then decided by a byte the page does \
     not show. What to fix: spell the prefix in its composed (NFC) form (`req/312` M-02)";

/// 🔴 **`req/312` L-01 (R23)** — the sentence two `$cas_read` prefixes that normalise to one carry.
///
/// L-01's repair normalises each declared prefix the way every resource URI is normalised, so that
/// a prefix spelled `Doc://Host/Page/` stops being inert against `doc://host/Page/1`. Two spellings
/// can now meet in the normalised map, and the one thing that must not happen there is a silent
/// drop: a file declaring two reading roads and getting one, with the loser invisible, is
/// `req/269` M-01's defect (a line that decides the file's behaviour and that the operator cannot
/// see) reproduced by the repair for it.
pub const CAS_READ_PREFIXES_COLLIDE: &str = "and this file declares another `$cas_read` prefix \
     that normalises to the same string. Prefixes are compared against normalised resource URIs, \
     so two spellings of one normal form are two declarations of one reading road and only one of \
     them could survive -- silently, with the loser deciding nothing and looking as though it \
     did. What to fix: keep one of the two spellings (`req/312` L-01)";

/// Where one argument of a **CAS read** call comes from.
///
/// 🔴 A separate vocabulary from [`ArgSource`], and the separation is the point rather than an
/// omission. Every word of [`ArgSource`] draws on material this road does not have: `forward` and
/// `git_blob_sha1_of_forward` name members of a **forward call** (`snapshot` has none — 41 §4
/// hands it a locator), `prior_contents_utf8` and `prior_json` name the **prior** (this read is
/// what produces it), and `do_result` names an **applied call's answer** (there has been no call).
/// Re-using the type would have offered five words that can only fail, and failing at resolution
/// time is exactly the fault `req/269` M-05 measured. What is left is the locator, and these are
/// the ways a URI is spelled into a tool's arguments.
///
/// Externally tagged for [`ArgSource`]'s reason: `{"const": "…"}` and `"resource_suffix"` are one
/// tag with one meaning, and nothing is guessed from shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CasArgSource {
    /// `{"const": "text"}` — a string the declaration supplies.
    Const(String),
    /// `{"const_json": <any JSON>}` — a typed constant, for a tool whose argument is a number or a
    /// boolean. DR-V4B-2's word, for DR-V4B-2's reason: `{"const": "true"}` sends the string.
    ConstJson(serde_json::Value),
    /// `"resource"` — the whole normalised resource URI.
    Resource,
    /// `"resource_suffix"` — what the resource URI holds **after** the `$cas_read` prefix that
    /// matched it. For `"notion://page/"` against `notion://page/abc-123`, the string `abc-123`.
    ResourceSuffix,
    /// `"resource_suffix_number"` — the same suffix as [`ResourceSuffix`](Self::ResourceSuffix),
    /// parsed as a JSON **integer** and sent as a JSON number rather than a string. For
    /// `"github://octo/demo/issues/"` against `github://octo/demo/issues/42`, the number `42`.
    ///
    /// 🔴 **DR-46-38** (`req/38` §378-3, G-15, `req/658`). Every other per-locator word resolves to
    /// a JSON string, and `issue_read` refuses a string `issue_number` with *"must be a number"*
    /// (`req/602` measured the gap end to end), so a faithful github issue `$cas_read` had no
    /// expressible road. This is the one word that carries a numeric argument from the locator, and
    /// `ConstJson` — which can hold a number — cannot stand in, because it is a **constant** and the
    /// issue number varies per call.
    ///
    /// 🔴 **Fail-closed, and the reason this word can refuse where the others cannot.** A suffix that
    /// is not a JSON integer — empty, `not-a-number`, a decimal `1.5`, a leading-zero `042`, a value
    /// past `i64` — is refused at resolution rather than sent as `0` or as the string a numeric
    /// argument would itself reject; a silent fallback would be a read of the wrong object, or a
    /// value the tool refuses, dressed as success. The line is `serde_json`'s own number grammar: a
    /// suffix is admitted when it parses as JSON and is an integer that fits `i64`, so `42` and `-5`
    /// pass while `042`, `+42`, `1.5` and `"42"` (which are not bare JSON integers) do not. Range is
    /// the tool's own to enforce — a negative is a valid `i64`, and `issue_read`'s `minimum: 1` is
    /// where that is refused — so this word is not fitted to one tool's bounds.
    ResourceSuffixNumber,
    /// `"server"` — the server endpoint, normalised. Rarely what a tool wants, and here because a
    /// declaration that needs it should not have to spell it as a constant that can drift from the
    /// locator the call is actually about.
    Server,
}

/// How a CAS read tool's arguments are built, member by member.
///
/// A `BTreeMap` for [`RestoreTemplate`]'s reason: the resolved JSON object carries its members in
/// key order, so two runs of one declaration produce the same bytes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CasTemplate {
    arguments: BTreeMap<String, CasArgSource>,
}

impl CasTemplate {
    /// An empty template: a read call with no arguments at all.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Declare one argument.
    #[must_use]
    pub fn with(mut self, name: impl Into<String>, source: CasArgSource) -> Self {
        self.arguments.insert(name.into(), source);
        self
    }

    /// The declared members, for a report line.
    #[must_use]
    pub fn arguments(&self) -> &BTreeMap<String, CasArgSource> {
        &self.arguments
    }

    /// Build the read call's arguments **from the locator alone**: UTF-8 JSON, the same encoding
    /// every other tool call's arguments travel in.
    ///
    /// 🔴 Every word of [`CasArgSource`] except
    /// [`ResourceSuffixNumber`](CasArgSource::ResourceSuffixNumber) is total over a parsed
    /// [`crate::locator::Position`]: a declaration built only of those words, once it parsed,
    /// resolves for every locator that matched its prefix, and there is no shape of the world in
    /// which resolution returns `Err` — which is what lets `snapshot` keep the one failure mode it
    /// has always had, the server not answering.
    ///
    /// [`ResourceSuffixNumber`](CasArgSource::ResourceSuffixNumber) is the deliberate exception, and
    /// its partiality is DR-46-38's point rather than a regression: a numeric word cannot invent a
    /// number from a suffix that is not one, and sending `0` or the raw string there would be a read
    /// of the wrong object or an argument the tool refuses, dressed as success. It refuses
    /// fail-closed instead, and `crate::cas::read_subject` folds that refusal into `Unreadable` the
    /// same way it folds an unsound declaration — the catalogue's prefix promised numeric suffixes
    /// and this locator's is not one.
    ///
    /// # Errors
    /// A human sentence when a [`ResourceSuffixNumber`](CasArgSource::ResourceSuffixNumber) suffix is
    /// not a JSON integer (see that variant), and — as before — when the resolved object has no JSON
    /// form, which `serde_json` reaches only for a map with non-string keys; kept as an error rather
    /// than an `expect` because a panic inside `snapshot` would abort a session over a declaration.
    pub fn resolve(
        &self,
        server: &str,
        resource: &str,
        matched_prefix: &str,
    ) -> core::result::Result<Vec<u8>, String> {
        let mut resolved = serde_json::Map::new();
        for (name, source) in &self.arguments {
            let value = match source {
                CasArgSource::Const(text) => serde_json::Value::String(text.clone()),
                CasArgSource::ConstJson(value) => value.clone(),
                CasArgSource::Resource => serde_json::Value::String(resource.to_string()),
                CasArgSource::ResourceSuffix => serde_json::Value::String(
                    resource
                        .strip_prefix(matched_prefix)
                        .unwrap_or(resource)
                        .to_string(),
                ),
                CasArgSource::ResourceSuffixNumber => {
                    // The same suffix as `ResourceSuffix`, but carried as a JSON **integer**. The
                    // suffix must parse as JSON and be an integer that fits `i64`; anything else is
                    // refused fail-closed rather than sent as `0` or as the string a numeric argument
                    // rejects (DR-46-38). `serde_json`'s number grammar draws the line, so `042`,
                    // `+42`, `1.5` and `"42"` are not admitted and `42`, `-5` are.
                    let suffix = resource.strip_prefix(matched_prefix).unwrap_or(resource);
                    let parsed: serde_json::Value = serde_json::from_str(suffix).map_err(|_| {
                        format!(
                            "the resource suffix {suffix:?} is not a JSON number, and \
                             `resource_suffix_number` sends a number rather than the string a \
                             numeric argument would refuse"
                        )
                    })?;
                    if !parsed.is_i64() {
                        return Err(format!(
                            "the resource suffix {suffix:?} parses as {parsed} but not as a JSON \
                             integer, and `resource_suffix_number` sends an integer rather than a \
                             value the tool would refuse"
                        ));
                    }
                    parsed
                }
                CasArgSource::Server => serde_json::Value::String(server.to_string()),
            };
            resolved.insert(name.clone(), value);
        }
        serde_json::to_vec(&serde_json::Value::Object(resolved))
            .map_err(|e| format!("the resolved CAS read arguments have no JSON form: {e}"))
    }
}

/// 🔴 **DR-46-16** — "the object at a locator under this prefix is read by *this* tool, called
/// with these arguments", for the compare-and-set half.
///
/// `deny_unknown_fields` for [`RestoreSpec`]'s and [`PriorRead`]'s reason: a misspelt `by_tool`
/// that parsed would be a read the operator did not declare, found at the first refused plan.
///
/// 🔴 **There is no `identity` member here, and its absence is DR-46-21 and not an omission this
/// lane made quietly.** [`PriorRead`] carries one because the escrow road's read answers about
/// *whatever tool it named* while the compare-and-set attests the *locator* — two objects, and
/// DR-46-15 is the predicate that they are one. The same two objects exist on this road, so the
/// same predicate is owed; `req/38` §218 ruling 2 put it in its own lane (one lane, one invariant)
/// and `req/305` is that lane. Until it lands, this declaration is a deployment's word that the
/// tool it names answers for the locator it is keyed by, and `docs/LIMITS.md` says exactly that.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CasRead {
    by_tool: String,
    #[serde(default)]
    arguments: CasTemplate,
}

impl CasRead {
    /// "call `by_tool` with the arguments this template builds, and what it answers is this
    /// object's contents".
    #[must_use]
    pub fn new(by_tool: impl Into<String>, arguments: CasTemplate) -> Self {
        Self {
            by_tool: by_tool.into(),
            arguments,
        }
    }

    /// The read tool's name, as the deployment declared it.
    #[must_use]
    pub fn by_tool(&self) -> &str {
        &self.by_tool
    }

    /// The declared arguments, for a report line and for the AC-051 derivation.
    #[must_use]
    pub fn arguments(&self) -> &CasTemplate {
        &self.arguments
    }

    /// What can be judged about this declaration **without a call**.
    ///
    /// # Errors
    /// A human sentence. [`Catalogue::from_json`] turns it into a parse error, and `crate::cas`
    /// asks it again for a catalogue built in code (`req/269` M-05's argument: a value that never
    /// met a parser is a value the parser never checked).
    pub fn soundness(&self) -> core::result::Result<(), String> {
        if names_nothing(&self.by_tool) {
            return Err(format!(
                "the CAS read declaration's `by_tool` is {:?}, {CAS_READ_TOOL_UNNAMED}",
                bounded(&self.by_tool)
            ));
        }
        Ok(())
    }
}

/// The restore tools a deployment declares.
///
/// A `BTreeMap` rather than a hash map because this value is small, is read far more often than it is
/// written, and has a deterministic `Debug` -- which matters when it appears in a refusal message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    restores: BTreeMap<String, RestoreSpec>,
    /// DR-V4B-2b: the `$server` metadata, when the file carried one. `None` for a catalogue built
    /// in code or read from a file without the slot (every catalogue before v0.4-l).
    server: Option<serde_json::Value>,
    /// DR-46-12: what an escrow does when the prior will not be read. `Refuse` unless the file
    /// said otherwise, which is what every catalogue before this window meant without saying it.
    on_read_failure: OnReadFailure,
    /// 🔴 DR-46-16: the CAS read faces, keyed by resource-URI **prefix** ([`CAS_READ_KEY`]). Empty
    /// for every catalogue written before this window, and an empty map is `resources/read`
    /// everywhere — which is what those files meant without having a way to say it.
    cas_reads: BTreeMap<String, CasRead>,
    /// 🔴 DR-46-28: where this deployment says its inputs come from ([`DETERMINISM_BOUNDARY_KEY`]).
    /// [`BoundaryStage::Unknown`] for every catalogue written before this window, which is what
    /// those files meant without having a way to say it.
    input_generation: BoundaryStage,
}

impl Catalogue {
    /// A catalogue that declares nothing: every tool call is irreversible as far as gx knows.
    ///
    /// The default, and deliberately the conservative one. An adapter built with it plans and applies
    /// exactly as one built with a full catalogue; what changes is that every `invert` answers
    /// `Ok(None)` and every change is escalated (**E-M3-4**) instead of being undoable.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// "a call to `tool` is undone by a call to `restored_by`, handed the resource's prior contents". (sem: SEM-gx-adapter-mcp-136)
    ///
    /// The arguments the inverse hands over are [`crate::restore_arguments`]'s -- canonical DAG-CBOR of
    /// `{contents, uri}`, which is MCP's own `resources/read` shape (`delta.rs`, "the restore (sem: SEM-gx-adapter-mcp-137)
    /// convention"). The right form for a restore tool that reads that shape, and the wrong one for a (sem: SEM-gx-adapter-mcp-138)
    /// structured-argument API tool -- the module doc's second section, and [`Self::with_restore_template`].
    #[must_use]
    pub fn with_restore(mut self, tool: impl Into<String>, restored_by: impl Into<String>) -> Self {
        self.restores.insert(
            tool.into(),
            RestoreSpec {
                restored_by: restored_by.into(),
                arguments: None,
                read_by: None,
            },
        );
        self
    }

    /// DR-V4B-2b: pin the server this catalogue was written against (see [`SERVER_METADATA_KEY`]).
    #[must_use]
    pub fn with_server(mut self, server: serde_json::Value) -> Self {
        self.server = Some(server);
        self
    }

    /// 🔴 **DR-46-9 A-3**: declare where `tool`'s prior comes from, for a server whose
    /// `resources/read` will not answer for it.
    ///
    /// A no-op for a tool this catalogue declares no restore for, and deliberately so: a read
    /// exists to feed an inverse, and a prior read for a tool with no inverse would be a call gx
    /// makes for nothing. `crate::invert` never reaches it either — the missing declaration is
    /// answered before any read happens.
    #[must_use]
    pub fn with_prior_read(mut self, tool: &str, read: PriorRead) -> Self {
        if let Some(spec) = self.restores.get_mut(tool) {
            spec.read_by = Some(read);
        }
        self
    }

    /// 🔴 **DR-46-16**: declare how the **compare-and-set half** reads objects whose resource URI
    /// begins with `pattern`, for a server that publishes no `resources/read` face for them.
    ///
    /// Unlike [`Self::with_prior_read`] this is **not** keyed by a tool and is not a no-op for an
    /// undeclared one: `snapshot` and `precondition` run for every locator this adapter is asked
    /// about, including locators no catalogue entry mentions, so the declaration is about the
    /// object rather than about the change.
    #[must_use]
    pub fn with_cas_read(mut self, pattern: impl Into<String>, read: CasRead) -> Self {
        self.cas_reads.insert(pattern.into(), read);
        self
    }

    /// 🔴 **DR-46-16** — the CAS read face that governs `resource`, and the prefix that matched.
    ///
    /// The **longest** matching prefix, which is a function of the file: two distinct prefixes of
    /// one string cannot share a length, so there is no tie for an iteration order to break.
    /// `None` is `resources/read`, unchanged and still the default.
    #[must_use]
    pub fn cas_read_for(&self, resource: &str) -> Option<(&str, &CasRead)> {
        self.cas_reads
            .iter()
            .filter(|(pattern, _)| prefix_governs(pattern.as_str(), resource))
            .max_by_key(|(pattern, _)| pattern.len())
            .map(|(pattern, read)| (pattern.as_str(), read))
    }

    /// How many locator prefixes carry a CAS read face. Printed beside [`Self::declared`] so that a
    /// run against a catalogue with none cannot look like a run against one with some.
    #[must_use]
    pub fn cas_reads_declared(&self) -> usize {
        self.cas_reads.len()
    }

    /// 🔴 **DR-46-12**: what this deployment does when the prior will not be read.
    #[must_use]
    pub fn with_on_read_failure(mut self, posture: OnReadFailure) -> Self {
        self.on_read_failure = posture;
        self
    }

    /// The posture this catalogue declared (see [`ON_READ_FAILURE_KEY`]).
    #[must_use]
    pub fn on_read_failure(&self) -> OnReadFailure {
        self.on_read_failure
    }

    /// 🔴 **DR-46-28**: where this deployment says its inputs come from.
    #[must_use]
    pub fn with_declared_input_generation(mut self, stage: BoundaryStage) -> Self {
        self.input_generation = stage;
        self
    }

    /// The stage this catalogue declared (see [`DETERMINISM_BOUNDARY_KEY`]).
    #[must_use]
    pub fn declared_input_generation(&self) -> BoundaryStage {
        self.input_generation
    }

    /// 🔴 **DR-46-28** — the one place the declaration face and the attest face meet.
    ///
    /// The caller supplies the half it observed (did a gate derive a verdict, and is that
    /// derivation replay-deterministic); this supplies the half the file declared; the join is
    /// `req/459`'s taxonomy. Nothing here reads the world, and nothing here decides: the arithmetic
    /// is `gx_core::DeterminismBoundary::of_stages` and this is the two arguments meeting.
    ///
    /// **Not wired to `gx-engine` in v0**, and that is structural rather than pending: `gx-engine`
    /// does not depend on this crate, and a boundary derived from anything outside Σ could not be
    /// reproduced by 43 §7-3b's rebuild. `crates/gx-engine/src/pipeline.rs`'s `attested_boundary`
    /// says so where it writes `unknown` for this stage.
    #[must_use]
    pub fn declared_boundary(&self, verdict_derivation: BoundaryStage) -> DeterminismBoundary {
        DeterminismBoundary::of_stages(self.input_generation, verdict_derivation)
    }

    /// 🔴 **C-25 / DR-46-9 A-4** — whether a call to `tool` can be undone, in three values, as far
    /// as the **declaration** goes.
    ///
    /// This is the static half and it says so: [`Reversibility::True`] here means "a tool is
    /// declared to undo this one", not "the undo will succeed". The runtime half — a template this
    /// call carries no material for, a prior no read would answer, a body over the ceiling — is
    /// [`crate::McpAdapter::reversibility`], which reads the world before it answers.
    #[must_use]
    pub fn declared_reversibility(&self, tool: &str) -> Reversibility {
        if self.restores.contains_key(tool) {
            Reversibility::True
        } else {
            Reversibility::False
        }
    }

    /// "a call to `tool` is undone by a call to `restored_by`, whose arguments are built as (sem: SEM-gx-adapter-mcp-139)
    /// `template` declares" -- the tool-aware form (req/38 §92, ruling 1, module doc's second section). (sem: SEM-gx-adapter-mcp-140)
    #[must_use]
    pub fn with_restore_template(
        mut self,
        tool: impl Into<String>,
        restored_by: impl Into<String>,
        template: RestoreTemplate,
    ) -> Self {
        self.restores.insert(
            tool.into(),
            RestoreSpec {
                restored_by: restored_by.into(),
                arguments: Some(template),
                read_by: None,
            },
        );
        self
    }

    /// The tool that undoes a call to `tool`, if the deployment declared one.
    #[must_use]
    pub fn restore_for(&self, tool: &str) -> Option<&str> {
        self.restores
            .get(tool)
            .map(|spec| spec.restored_by.as_str())
    }

    /// The whole declaration for `tool`, template included -- what `crate::invert` reads.
    #[must_use]
    pub fn spec_for(&self, tool: &str) -> Option<&RestoreSpec> {
        self.restores.get(tool)
    }

    /// How many tools this catalogue can undo. Printed by `tests/mcp_conformance.rs` so that a run
    /// against an empty catalogue cannot look like a run against a full one.
    #[must_use]
    pub fn declared(&self) -> usize {
        self.restores.len()
    }

    /// DR-V4B-2b: the `$server` metadata the file carried, verbatim, or `None`.
    #[must_use]
    pub fn server(&self) -> Option<&serde_json::Value> {
        self.server.as_ref()
    }

    /// 🔴 **`req/279` M-04 + H-01 + L-03** — everything about one entry that can be judged
    /// **without a call**, in the context of the whole file.
    ///
    /// [`RestoreSpec::soundness`] judges a declaration on its own; this adds the one question that
    /// needs its neighbours. The audit's `s2` catalogue declared `doc.write` as an effect and, in
    /// the same file, named `doc.write` as the **read face** of that same effect. It parsed. gx
    /// then reached that tool through [`crate::transport::ToolTransport::read_prior_by_tool`] —
    /// the escrow road, which takes no `Admitted`, runs before `apply`, and runs twice per forward
    /// call. Measured: the server held `"the read face wrote this\n"` under a verdict that said
    /// the effect had been **refused**.
    ///
    /// "This tool is an effect that needs undoing" and "this tool is safe to call for a snapshot"
    /// are contradictory claims, and when one file makes both, a machine can say so. It does not
    /// close the general case — `docs/LIMITS.md` has said since v0.5-d that nothing marks a tool
    /// read-only that a server cannot misstate, and a read face declared in *another* file, or a
    /// writing tool this catalogue simply never declares, is still the deployment's to get right.
    /// What it closes is the case where the contradiction is written down.
    ///
    /// # Errors
    /// A human sentence. A no-op for a tool this catalogue does not declare.
    pub fn entry_soundness(&self, tool: &str) -> core::result::Result<(), String> {
        self.entry_fault(tool).map_err(|f| f.why().to_string())
    }

    /// 🔴 **`req/38` §227 ruling 1** — **the** question "does this file, in what it writes down,
    /// say that `tool` writes?", asked in one place.
    ///
    /// # Why this is a function rather than a predicate each gate spells for itself
    ///
    /// Three consecutive adversarial audits found the same shape, and the third one made the
    /// diagnosis structural rather than per-site. `req/291` found a gate whose printed sentence and
    /// whose implementation asked different questions. `req/312` found that the `$cas_read` gate
    /// asked only about the **keys** of `restores` and never about the `restored_by` **values**.
    /// `req/316` then found that the repair had landed on that one gate while its **sibling** — the
    /// escrow road's `read_by` gate in [`Self::entry_fault`], which asks the identical question —
    /// still carried the old half. What that cost, measured on a real server: a paragraph a person
    /// had written was emptied on a road where gx refused before reaching a verdict, by gx's own
    /// read, with the agent's own effect never sent.
    ///
    /// So the repair is not a third site spelled correctly. It is that **there is one site**, and
    /// every gate calls it. A gate that re-spells the question is the defect whatever answer the
    /// re-spelling happens to give, and `tests/r24_predicate_unification.rs` holds the number of
    /// spellings in this file to the two it is allowed to have.
    ///
    /// # The two sets, and why both are this file talking about itself
    ///
    /// * a **key** of `restores` is a tool this file calls an effect that needs undoing;
    /// * a **value** of `restored_by` is the call that puts an object back, which writes by
    ///   construction — and this file is the thing that says so.
    ///
    /// Both are material a gate that runs before any call can use, which is what makes `req/279`
    /// M-04's question answerable at parse time at all. Keys are asked first, so a tool that is in
    /// both sets keeps the sentence it has always had.
    ///
    /// [`Self::declared_reversibility`] also reads `restores` and is deliberately **not** a caller:
    /// it asks a different question (*is there a declared inverse **for** this tool*), and folding
    /// two questions into one function is the failure this one exists to undo.
    #[must_use]
    pub fn writes_per_this_file(&self, tool: &str) -> Option<WritesBecause<'_>> {
        if self.restores.contains_key(tool) {
            return Some(WritesBecause::Effect);
        }
        self.restores
            .iter()
            .find(|(_, spec)| spec.restored_by() == tool)
            .map(|(effect, _)| WritesBecause::Inverse {
                of: effect.as_str(),
            })
    }

    /// 🔴 **`req/303` M-03** — [`Self::entry_soundness`], with the [`DeclarationFace`] each fault
    /// is about. `crate::invert` reads the face to choose which closing sentence it may append.
    ///
    /// # Errors
    /// The entry's first fault, with its face. A no-op for a tool this catalogue does not declare.
    pub fn entry_fault(&self, tool: &str) -> core::result::Result<(), DeclarationFault> {
        let Some(spec) = self.restores.get(tool) else {
            return Ok(());
        };
        spec.fault()?;
        if let Some(read) = spec.read_by() {
            // 🔴 **`req/38` §227 ruling 1** — the sibling gate, asking the one predicate.
            //
            // What stood here was `self.restores.contains_key(read.by_tool())`: the same half of
            // the question `req/312` H-01 found in `cas_read_soundness`, left behind when R23
            // widened that one. `req/316` H-01 followed the remedy the first sentence below prints,
            // verbatim, arrived at a `read_by` naming this file's own `restored_by`, and measured
            // gx emptying a person's paragraph from the escrow road before any verdict existed.
            match self.writes_per_this_file(read.by_tool()) {
                Some(WritesBecause::Effect) => {
                    return Err(DeclarationFault::new(
                        DeclarationFace::ReadFace,
                        format!(
                    "it names {:?} as its read face, and this same catalogue declares {:?} as an \
                     effect that needs undoing. gx calls a read face through the escrow road, \
                     which carries no admission and runs before the change is applied, so a file \
                     that says both is asking gx to make an unadmitted change while it is deciding \
                     whether to allow one. What to fix: name a read face this catalogue does not \
                     declare as an effect (`req/279` M-04)",
                    read.by_tool(),
                    read.by_tool()
                ),
                    ));
                }
                // The sentence is [`CAS_READ_FACE_IS_AN_INVERSE`]'s, re-used rather than rewritten:
                // `req/314` §2 made it the rule that a sentence with existing arms is widened by
                // gaining a subject, not by editing the words those arms hold.
                Some(WritesBecause::Inverse { of }) => {
                    return Err(DeclarationFault::new(
                        DeclarationFace::ReadFace,
                        format!(
                            "it names {:?} as its read face, and this same catalogue declares that \
                             tool as the inverse of {:?}: {CAS_READ_FACE_IS_AN_INVERSE}",
                            read.by_tool(),
                            bounded(of)
                        ),
                    ));
                }
                None => {}
            }
        }
        Ok(())
    }

    /// 🔴 **DR-46-16** — everything about one `$cas_read` entry that can be judged **without a
    /// call**, in the context of the whole file.
    ///
    /// Two questions, and the second is the one that needs the file's other entries: [`CasRead`]
    /// judges the declaration alone, and this asks whether the tool it names is a tool this same
    /// catalogue declares as an **effect that needs undoing**. `snapshot` runs before a plan
    /// exists, so a contradiction written down here is one gate earlier than the one `req/279`
    /// M-04 measured on the escrow road.
    ///
    /// # Errors
    /// A human sentence. A no-op for a prefix this catalogue does not declare.
    pub fn cas_read_soundness(&self, pattern: &str) -> core::result::Result<(), String> {
        let Some(read) = self.cas_reads.get(pattern) else {
            return Ok(());
        };
        if pattern.is_empty() {
            return Err(format!(
                "the `{CAS_READ_KEY}` pattern {CAS_READ_PATTERN_EMPTY}"
            ));
        }
        read.soundness()?;
        // 🔴 **`req/38` §227 ruling 1** — the same one predicate the escrow gate asks.
        //
        // R23 wrote the two halves of this question out here by hand, and `req/316` H-01 measured
        // what that cost one function up. Both gates now ask [`Self::writes_per_this_file`]; the
        // two sentences are unchanged, and which set a face fell into is what the answer carries.
        match self.writes_per_this_file(read.by_tool()) {
            Some(WritesBecause::Effect) => {
                return Err(format!(
                "it names {:?} as the CAS read face for locators under {:?}, {CAS_READ_FACE_IS_AN_EFFECT}",
                read.by_tool(),
                bounded(pattern)
            ));
            }
            Some(WritesBecause::Inverse { of }) => {
                return Err(format!(
                "it names {:?} as the CAS read face for locators under {:?}, and this same \
                 catalogue declares that tool as the inverse of {:?}: {CAS_READ_FACE_IS_AN_INVERSE}",
                read.by_tool(),
                bounded(pattern),
                bounded(of)
            ));
            }
            None => {}
        }
        Ok(())
    }

    /// 🔴 Every entry through [`Self::entry_soundness`], in the file's own order.
    ///
    /// # Errors
    /// The first entry that is not sound, named.
    pub fn soundness(&self) -> core::result::Result<(), String> {
        for tool in self.restores.keys() {
            // 🔴 **`req/303` M-03** — the subject is the **face the fault is about**, not the
            // words R18 wrote when a read face was the only thing an entry could get wrong. The
            // sentence this used to print, verbatim, was *"the read declaration of entry {tool} is
            // not sound"*, and it was printed over a catalogue that declared no `read_by` at all.
            self.entry_fault(tool).map_err(|fault| {
                format!(
                    "{} of entry {tool:?} is not sound: {}",
                    fault.face().subject(),
                    fault.why()
                )
            })?;
        }
        // 🔴 DR-46-16, and after the restores rather than before: `cas_read_soundness` asks
        // whether a read face is an **effect this file declares**, so an entry checked while the
        // restores were half-read would be checked against half a file — `from_json`'s own
        // argument for checking the map after it is built, one slot over.
        for pattern in self.cas_reads.keys() {
            self.cas_read_soundness(pattern).map_err(|why| {
                format!("the `{CAS_READ_KEY}` declaration for {pattern:?} is not sound: {why}")
            })?;
        }
        Ok(())
    }

    /// Read a whole catalogue from a deployment's JSON file (`gx wrap --restore-catalogue`,
    /// `gx undo --mcp-restore-catalogue`): a map from forward tool to [`RestoreSpec`], e.g.
    ///
    /// ```text
    /// {
    ///   "create_or_update_file": {
    ///     "restored_by": "create_or_update_file",
    ///     "arguments": {
    ///       "owner":   { "forward": "owner" },
    ///       "content": "prior_contents_utf8",
    ///       "sha":     { "git_blob_sha1_of_forward": "content" }
    ///     }
    ///   },
    ///   "notes.write": { "restored_by": "notes.restore" }
    /// }
    /// ```
    ///
    /// An entry with no `"arguments"` is the v0.1 `{contents, uri}` form, so one file can declare
    /// both kinds. Parsed here rather than in `gx-cli` so that the format has exactly one reader
    /// and the tests that pin it live beside the type it builds.
    ///
    /// # Errors
    /// A human sentence, when the bytes are not this map. The caller decides which of its own
    /// error vocabularies that is (`gx-cli` reads it as a usage error).
    ///
    /// 🔴 DR-V4B-2 (`req/189`): two additions, both backward compatible — a member may be
    /// `{"const_json": <any JSON>}` (a typed constant; `"const"` is unchanged), and the top-level
    /// key `"$server"` ([`SERVER_METADATA_KEY`]) is metadata rather than a tool declaration.
    pub fn from_json(bytes: &[u8]) -> core::result::Result<Self, String> {
        // 🔴 **R36 / `req/476` L-02** — the duplicate key, refused before anything is read out of
        // the map.
        //
        // `serde_json::from_slice::<BTreeMap<_, _>>` is last-one-wins: a file declaring
        // `$determinism_boundary` twice parses, the first declaration is dropped without a word,
        // and the deployment is judged under a value it did not write last on purpose. That is the
        // exact shape this module's header rules out one paragraph up — "a misspelling is a parse
        // error, not a silent default" — and the reason it gives applies with more force here,
        // because a duplicate is not even a typo the author can see by reading the value back.
        //
        // The audit that found it graded it `L` and said plainly that it had **not driven** it
        // (`req/476` §3-2); `tests/r36_catalogue_duplicate_key.rs` drives it, and measured
        // `Ok("llm_originated")` over a file whose first declaration was `deterministic_replay`.
        //
        // The scan is over the **top level**, which is where every reserved slot and every tool
        // name lives. Duplicates nested inside one entry's `arguments` are not caught here and are
        // recorded as a residue rather than implied to be covered.
        let mut raw: BTreeMap<String, serde_json::Value> = serde_json::from_slice(bytes)
            .map_err(|e| format!("not a restore catalogue (a JSON map from tool to {{restored_by, arguments?}}): {e}"))?;
        if let Some(duplicate) = first_duplicate_key(bytes) {
            return Err(format!(
                "the key {:?} is declared more than once in this catalogue. A JSON map takes the \
                 last value for a repeated key and says nothing, so the earlier declaration would \
                 be dropped silently — and for a reserved slot that is a deployment judged under a \
                 value it did not mean. Delete one of them (req/476 L-02)",
                bounded(&duplicate)
            ));
        }
        let server = raw.remove(SERVER_METADATA_KEY);
        // DR-46-12: the second reserved slot. A value that is not one of the two words is a parse
        // error, because the alternative is a deployment that believes it opted in and did not.
        let on_read_failure = match raw.remove(ON_READ_FAILURE_KEY) {
            None => OnReadFailure::Refuse,
            Some(value) => serde_json::from_value(value).map_err(|e| {
                format!(
                    "the reserved slot {ON_READ_FAILURE_KEY:?} is {:?} or {:?}: {e}",
                    "refuse", "unknown"
                )
            })?,
        };
        // 🔴 DR-46-16: the third reserved slot. A malformed value is a parse error for
        // `ON_READ_FAILURE_KEY`'s reason — a `$cas_read` that quietly meant "nothing" would be a
        // deployment believing its tools-only server was readable when every plan still refuses.
        // 🔴 DR-46-28: the fourth reserved slot. Parsed against `BoundaryStage::ALL` rather than
        // through serde, because the three words a **file** spells are `req/459`'s snake_case
        // vocabulary and the words the **wire** spells are the variant names 42 §2.1 already
        // carries. One type, two spellings, and the vocabulary constant is what keeps them tied.
        let input_generation = match raw.remove(DETERMINISM_BOUNDARY_KEY) {
            None => BoundaryStage::Unknown,
            Some(value) => {
                let word = value.as_str().unwrap_or_default();
                match word {
                    "deterministic_replay" => BoundaryStage::DeterministicReplay,
                    "llm_originated" => BoundaryStage::LlmOriginated,
                    "unknown" => BoundaryStage::Unknown,
                    _ => {
                        return Err(format!(
                        "the reserved slot {DETERMINISM_BOUNDARY_KEY:?} is one of {:?}; it is {:?}",
                        BoundaryStage::ALL,
                        bounded(&value.to_string())
                    ))
                    }
                }
            }
        };
        let declared_cas_reads: BTreeMap<String, CasRead> = match raw.remove(CAS_READ_KEY) {
            None => BTreeMap::new(),
            Some(value) => {
                // 🔴 **`req/316` L-01 (R24)** — asked on the **raw** value, before serde is given
                // the chance to read a sequence as a struct. See [`DECLARATION_IS_POSITIONAL`].
                for (pattern, entry) in value.as_object().into_iter().flatten() {
                    if entry.is_array() {
                        return Err(format!(
                            "the `{CAS_READ_KEY}` declaration for {:?} {DECLARATION_IS_POSITIONAL}",
                            bounded(pattern)
                        ));
                    }
                }
                serde_json::from_value(value).map_err(|e| {
                    format!(
                        "the reserved slot {CAS_READ_KEY:?} is a JSON map from a resource-URI \
                         prefix to {{by_tool, arguments?}}: {e}"
                    )
                })?
            }
        };
        // 🔴 **`req/312` M-02 and L-01 (R23)** — the two questions about a `$cas_read` key, asked
        // before it is put in a map that is keyed by it.
        //
        // M-02 first, and on the **written** spelling: the decomposition check is about what the
        // operator read while approving the file, so normalising first would answer it about a
        // string nobody saw. Then L-01: the pattern is compared against resource URIs that have
        // been through `locator::normalize` on every road, and until this line it was compared
        // exactly as written — so `Doc://Host/Page/` parsed, looked live, and unlocked nothing.
        let mut cas_reads: BTreeMap<String, CasRead> = BTreeMap::new();
        for (pattern, read) in declared_cas_reads {
            if carries_a_combining_mark(&pattern) {
                return Err(format!(
                    "the `{CAS_READ_KEY}` prefix {:?} {CAS_READ_PREFIX_IS_DECOMPOSED}",
                    bounded(&pattern)
                ));
            }
            // 🔴 **`req/320` M-04 (R25)** — position 1 of **five**, and the one R24 did not walk.
            //
            // Asked here rather than after the tool name for the reason M-02's decomposition check
            // is asked here: the key is what a reader meets first, and it is asked on the
            // **written** spelling, before `locator::normalize` produces a string nobody saw.
            //
            // 🔴 Fault ordering, stated because it was checked: a pattern that is *only* invisible
            // characters is not this fault (`carries_edge_whitespace` answers `false` for it, by
            // the same argument the tool-name positions make), so it still travels to the place
            // that judges it. The empty pattern is untouched — `""` has no edge — and reaches
            // `CAS_READ_PATTERN_EMPTY` in `cas_read_soundness` exactly as before.
            if carries_edge_whitespace(&pattern) {
                return Err(format!(
                    "the `{CAS_READ_KEY}` prefix {:?} {CAS_READ_PREFIX_CARRIES_EDGE_WHITESPACE}",
                    bounded(&pattern)
                ));
            }
            // 🔴 **`req/324` M-04 (`req/38` §231 ruling 2)** — slot 4 of five for the *unnamed*
            // question, which until this window was asked in three.
            //
            // The empty prefix is left to `cas_read_soundness`'s [`CAS_READ_PATTERN_EMPTY`]: that
            // sentence is about a key that is a prefix of everything and it is the one a reader who
            // wrote `""` needs, so this gate takes only the names that *look* empty without being
            // it. `carries_edge_whitespace` answers `false` for an all-invisible name by
            // construction (nothing is left after the edges come off), which is exactly the hole
            // `req/320` L-01 closed for the three slots it swept.
            if !pattern.is_empty() && names_nothing(&pattern) {
                return Err(format!(
                    "the `{CAS_READ_KEY}` prefix {:?} {CAS_READ_PREFIX_NAMES_NOTHING}",
                    bounded(&pattern)
                ));
            }
            if carries_a_combining_mark(read.by_tool()) {
                return Err(format!(
                    "the tool name {:?} {TOOL_NAME_IS_DECOMPOSED}",
                    bounded(read.by_tool())
                ));
            }
            // 🔴 **`req/316` L-02 (R24)** — position 2 of 5. See `carries_edge_whitespace`.
            if carries_edge_whitespace(read.by_tool()) {
                return Err(format!(
                    "the tool name {:?} {TOOL_NAME_CARRIES_EDGE_WHITESPACE}",
                    bounded(read.by_tool())
                ));
            }
            // The empty pattern is `soundness`'s to refuse (`CAS_READ_PATTERN_EMPTY`), and it must
            // reach that sentence rather than this one: normalising "" produces "" and the map
            // takes it, so nothing is lost by leaving the check where it already is.
            let normalised = crate::locator::normalize(&pattern);
            if let Some(previous) = cas_reads.insert(normalised.clone(), read) {
                return Err(format!(
                    "the `{CAS_READ_KEY}` prefix {:?} normalises to {:?} (the read face \
                     {:?}), {CAS_READ_PREFIXES_COLLIDE}",
                    bounded(&pattern),
                    bounded(&normalised),
                    bounded(previous.by_tool())
                ));
            }
        }
        let mut restores = BTreeMap::new();
        for (tool, spec) in raw {
            // 🔴 **`req/303` L-05** — asked before the entry is parsed, because the question is
            // about the **key** and a reader meets the key first. See [`TOOL_NAME_IS_DECOMPOSED`]
            // for why the refusal is the decomposed spelling rather than the collision, and for
            // what it does not cover.
            if carries_a_combining_mark(&tool) {
                return Err(format!(
                    "the tool name {:?} {TOOL_NAME_IS_DECOMPOSED}",
                    bounded(&tool)
                ));
            }
            // 🔴 **`req/316` L-02 (R24)** — position 3 of 5.
            if carries_edge_whitespace(&tool) {
                return Err(format!(
                    "the tool name {:?} {TOOL_NAME_CARRIES_EDGE_WHITESPACE}",
                    bounded(&tool)
                ));
            }
            // 🔴 **`req/324` M-04 (`req/38` §231 ruling 2)** — slot 5 of five for the *unnamed*
            // question. `RestoreSpec::fault` asks it of `restored_by`, `PriorRead` and `CasRead`
            // ask it of their `by_tool`; the **key** — the effect this entry is about — was never
            // asked, so `{"\u{200b}": {"restored_by": "notes.restore"}}` declared an inverse for a
            // tool whose name is blank on the page.
            if names_nothing(&tool) {
                return Err(format!(
                    "the tool name {:?} {EFFECT_TOOL_UNNAMED}",
                    bounded(&tool)
                ));
            }
            // 🔴 **`req/316` L-01 (R24)** — the same question the `$cas_read` slot is asked, on the
            // raw value and for the same reason.
            if spec.is_array() {
                return Err(format!(
                    "the declaration for {:?} {DECLARATION_IS_POSITIONAL}",
                    bounded(&tool)
                ));
            }
            let spec: RestoreSpec = serde_json::from_value(spec).map_err(|e| {
                format!(
                    "not a restore catalogue (a JSON map from tool to {{restored_by, arguments?}}): \
                     entry {tool:?}: {e}"
                )
            })?;
            // 🔴 **`req/312` M-02 (R23)** — the other two places this file spells a **tool name**.
            //
            // `req/303` L-05's gate was asked about the key alone, and `docs/LIMITS.md` v0.5-i
            // said of it *"the width is exactly this"* — a sentence about which marks are covered,
            // written beside a check that ran in one of the five positions a tool is named. The
            // audit measured the other four accepting the decomposed spelling. The question is
            // about a name an operator approves by reading, and it does not become a different
            // question because the name is on the right-hand side of the colon.
            for named in [
                spec.restored_by(),
                spec.read_by().map_or("", PriorRead::by_tool),
            ] {
                if carries_a_combining_mark(named) {
                    return Err(format!(
                        "the tool name {:?} {TOOL_NAME_IS_DECOMPOSED}",
                        bounded(named)
                    ));
                }
                // 🔴 **`req/316` L-02 (R24)** — positions 4 and 5 of 5, the same loop the
                // decomposition gate walks, for the same reason it walks it.
                if carries_edge_whitespace(named) {
                    return Err(format!(
                        "the tool name {:?} {TOOL_NAME_CARRIES_EDGE_WHITESPACE}",
                        bounded(named)
                    ));
                }
            }
            restores.insert(tool, spec);
        }
        // 🔴 `req/269` M-05 / DR-46-15 / `req/279` H-01, M-04, L-03 — what can be judged without a
        // call is judged here, so a file carrying it never starts a session. The audit measured the
        // alternative twice: a declaration that named a prior member inside its own read passed
        // `gx wrap`, ran, and refused the first effect in words that read like the server had
        // failed with zero arrivals on the server's side; and a declaration naming its own effect
        // as its read face passed, ran, and wrote the server through the escrow road.
        //
        // 🔴 After the whole map is built, not during: M-04 asks whether a read face is an entry
        // **of this file**, and an entry checked while the map was half-built would be checked
        // against half a file — the first tool in the map could name the last one and pass.
        let catalogue = Self {
            restores,
            server,
            on_read_failure,
            cas_reads,
            input_generation,
        };
        catalogue.soundness()?;
        Ok(catalogue)
    }
}

/// 🔴 **DR-46-38** (`req/38` §378-3, G-15, `req/658`) — [`CasArgSource::ResourceSuffixNumber`], the
/// one per-locator word that supplies a **numeric** argument.
///
/// G-15 (`crates/gx-cli/tests/rmcp1_github_p1.rs`) measured the gap this closes: every other
/// per-locator word resolves to a JSON **string**, and `issue_read` refuses a string `issue_number`
/// with *"must be a number"*, so a faithful github issue `$cas_read` was not expressible. These arms
/// hold the word to the wire truth that finding rests on and to its fail-closed refusal.
#[cfg(test)]
mod dr46_38_resource_suffix_number {
    use super::*;

    /// A github issue `$cas_read` wiring the locator's suffix to `issue_read`'s numeric
    /// `issue_number`. Before the word exists this file does not parse.
    const GITHUB_ISSUE_CATALOGUE: &[u8] = br#"{
      "$cas_read": {
        "github://octo/demo/issues/": {
          "by_tool": "issue_read",
          "arguments": { "issue_number": "resource_suffix_number" }
        }
      }
    }"#;

    const SERVER: &str = "https://mcp.example/gh";
    const PREFIX: &str = "github://octo/demo/issues/";

    fn issue(number: &str) -> String {
        format!("{PREFIX}{number}")
    }

    /// 🔴 **red-first / positive control** — the declaration parses, and the locator's suffix reaches
    /// `issue_number` as a JSON **number**. Red at base `3fac6f57`: `from_json` refuses the unknown
    /// variant `resource_suffix_number`, so this panics at the first `expect`.
    #[test]
    fn the_word_wires_a_numeric_issue_number_from_the_locator() {
        let catalogue = Catalogue::from_json(GITHUB_ISSUE_CATALOGUE)
            .expect("🔴 DR-46-38: the github issue $cas_read declaration parses");
        let uri = issue("42");
        let (prefix, read) = catalogue
            .cas_read_for(&uri)
            .expect("the issue locator matches the declared prefix");
        let built = read
            .arguments()
            .resolve(SERVER, &uri, prefix)
            .expect("the numeric word resolves for a numeric suffix");
        let value: serde_json::Value =
            serde_json::from_slice(&built).expect("resolved arguments are JSON");
        println!(
            "DR4638_GREEN issue_number={} is_number={} is_i64={}",
            value["issue_number"],
            value["issue_number"].is_number(),
            value["issue_number"].is_i64()
        );
        assert_eq!(
            value["issue_number"],
            serde_json::json!(42),
            "🔴 the suffix reaches issue_number as the number 42"
        );
        assert!(
            value["issue_number"].is_i64(),
            "and specifically as a JSON integer, not the string every other per-locator word sends"
        );
    }

    /// Resolve a one-member `{issue_number: resource_suffix_number}` template against a locator and
    /// hand back what `issue_number` became, or the refusal.
    fn resolve_issue_number(uri: &str) -> core::result::Result<serde_json::Value, String> {
        let built = CasTemplate::new()
            .with("issue_number", CasArgSource::ResourceSuffixNumber)
            .resolve(SERVER, uri, PREFIX)?;
        let value: serde_json::Value =
            serde_json::from_slice(&built).expect("resolved arguments are JSON");
        Ok(value
            .get("issue_number")
            .cloned()
            .expect("the declared member is present"))
    }

    /// 🔴 **refusal probe + its negative control, one body** (`req/658` §4-2). A non-numeric suffix
    /// is refused fail-closed — not resolved to `0`, an empty string, or the raw text — and a numeric
    /// suffix on the same code path is not. Being a discrimination, it cannot pass on a word that
    /// silently swallows the fault.
    #[test]
    fn a_non_numeric_suffix_is_refused_fail_closed_and_a_numeric_one_is_not() {
        let refused = resolve_issue_number(&issue("not-a-number"))
            .expect_err("🔴 a suffix that is not a number has no numeric value to send");
        println!("DR4638_REFUSE {refused}");
        assert!(
            refused.contains("not-a-number"),
            "the refusal names the suffix it could not turn into a number: {refused}"
        );
        assert!(
            refused.contains("resource_suffix_number"),
            "and names the word, so a reader knows which declaration to fix: {refused}"
        );

        let ok = resolve_issue_number(&issue("42"))
            .expect("🔴 the negative control: a numeric suffix on the same path resolves");
        println!("DR4638_REFUSE_CONTROL ok={ok}");
        assert_eq!(
            ok,
            serde_json::json!(42),
            "and it is the number, so the refusal above is the suffix and not a dead word"
        );
    }

    /// 🔴 The admitted/refused line is `serde_json`'s own integer grammar, measured suffix by suffix.
    /// This is where the reqdef's edge questions — decimals, negatives, leading zeros, oversize — are
    /// fixed against the wire type rather than guessed (`req/658` §2 decode-strict).
    #[test]
    fn the_json_integer_grammar_draws_the_admitted_line() {
        for (suffix, want) in [
            ("42", 42_i64),
            ("-5", -5),
            ("0", 0),
            ("2147483648", 2_147_483_648),
        ] {
            let got = resolve_issue_number(&issue(suffix))
                .unwrap_or_else(|e| panic!("{suffix:?} is a JSON integer and must resolve: {e}"));
            println!("DR4638_GRAMMAR_OK suffix={suffix:?} -> {got}");
            assert_eq!(
                got,
                serde_json::json!(want),
                "{suffix:?} resolves to {want}"
            );
            assert!(got.is_i64(), "{suffix:?} is carried as an integer");
        }

        // Refused, each for a reason the grammar states: a decimal is not an integer; a leading
        // zero, a leading `+`, and scientific form are not bare JSON integers; a value past `i64`
        // does not fit the argument the fixture consumes with `as_i64`; the empty and the
        // non-numeric are not numbers at all.
        for suffix in [
            "",
            "not-a-number",
            "1.5",
            "042",
            "+42",
            "1e3",
            "0x2a",
            "99999999999999999999999999",
        ] {
            let refused = resolve_issue_number(&issue(suffix)).expect_err(&format!(
                "🔴 {suffix:?} is not a JSON integer and is refused"
            ));
            println!("DR4638_GRAMMAR_REFUSE suffix={suffix:?} -> {refused}");
            assert!(
                refused.contains("resource_suffix_number"),
                "the refusal of {suffix:?} carries the word: {refused}"
            );
        }
    }

    /// 🔴 **additive only** (`req/658` §4-3). The word is a new arm; the four other members of the
    /// vocabulary produce exactly what they produced before, member for member and byte for byte.
    #[test]
    fn the_existing_words_are_unchanged() {
        let uri = issue("42");
        let built = CasTemplate::new()
            .with("number", CasArgSource::ResourceSuffixNumber)
            .with("suffix", CasArgSource::ResourceSuffix)
            .with("uri", CasArgSource::Resource)
            .with("endpoint", CasArgSource::Server)
            .with("note", CasArgSource::Const("gx cas".to_string()))
            .with("depth", CasArgSource::ConstJson(serde_json::json!(2)))
            .resolve(SERVER, &uri, PREFIX)
            .expect("every member resolves for this locator");
        let value: serde_json::Value =
            serde_json::from_slice(&built).expect("resolved arguments are JSON");
        println!("DR4638_ADDITIVE {}", String::from_utf8_lossy(&built));
        assert_eq!(
            value["suffix"],
            serde_json::json!("42"),
            "🔴 `resource_suffix` still sends the string, unchanged"
        );
        assert_eq!(value["uri"], serde_json::json!(uri), "`resource` unchanged");
        assert_eq!(
            value["endpoint"],
            serde_json::json!(SERVER),
            "`server` unchanged"
        );
        assert_eq!(
            value["note"],
            serde_json::json!("gx cas"),
            "`const` unchanged"
        );
        assert_eq!(
            value["depth"],
            serde_json::json!(2),
            "`const_json` unchanged"
        );
        // And the new word beside them, so the arm is additive rather than a rename.
        assert_eq!(value["number"], serde_json::json!(42));
        assert!(
            value["suffix"].is_string() && value["number"].is_i64(),
            "🔴 the string word and the number word coexist on one template: {value}"
        );
    }
}
