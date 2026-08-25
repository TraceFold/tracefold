// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! `invert`: a compensating call, carrying the body, built while the pre-state is still live.
//!
//! Spec: 41 §4 for the method and DR-1(a), 42 §5 for why an escrow carries a body, 34 AC-048 for what
//! is measured. The rulings are **E-M4-30** (the escrow is constructed **before** `apply`, 43 T-10b),
//! **E-M4-3** (the round trip is quantified at the one `pre` handed in), **E-M4-32** (which facts may
//! take the `Ok(None)` form), **M4-21** (the escrow ceiling) and **E-M4-5** (an engine folds
//! `Some`/`None` into `GateInput.invert_available` at verify time).
//!
//! # 🔴 The inverse of a tool call is another tool call, and only the deployment knows which
//!
//! There is nothing in the MCP protocol that says how to undo `tools/call`. A filesystem inverse is
//! derivable (write the old bytes) and a git inverse is derivable (move the reference back); this one
//! is **declared**, by whoever runs the server, in a [`crate::catalogue::Catalogue`]. The crate root
//! and `catalogue.rs` argue why that is a declaration rather than a second gate.
//!
//! What the inverse carries is the **prior contents of the resource**, which is 42 §5 in as many words:
//! "since a digest-only form makes an actual undo physically impossible". So this module reads the resource -- and 43 T-10b (sem: SEM-gx-adapter-mcp-045)
//! requires the caller to have done so before `apply`, because after the call the prior contents are
//! gone.
//!
//! # The three reasons for `Ok(None)`, and all are real
//!
//! **E-M4-32** fixes which facts may take the form: "`Ok(None)` is limited to 'a legitimate construction of the same object is impossible' (over the (sem: SEM-gx-adapter-mcp-046)
//! ceiling, or the old content already discarded)". This adapter has one of each, and A2 (req/38 §92, ruling 1) added a (sem: SEM-gx-adapter-mcp-047)
//! third of the same family:
//!
//! * **No restore tool is declared** for the tool being inverted. The change is one gx cannot undo, and
//!   **E-M3-4** asks a person before it happens. An empty catalogue makes every change this shape,
//!   which is the conservative direction and the default.
//! * **The prior contents exceed [`crate::delta::MAX_INVERSE_PAYLOAD_BYTES`]** -- **M4-21**'s own
//!   instance, the one `gx-adapter-git` argued it could never reach. Here it is reachable, because an
//!   MCP server offers no content-addressed store to leave the body in.
//! * **A declared [`RestoreTemplate`](crate::catalogue::RestoreTemplate) cannot be resolved from this
//!   call's material** -- the forward call carries no member the template names, or the prior contents
//!   are not the text a member derives from. This is "a legitimate construction is impossible" read for the tool-aware form: (sem: SEM-gx-adapter-mcp-048)
//!   the declaration is sound for the calls it was written for, *this* call is not one of them, and
//!   the conservative answer is the same escalation an undeclared tool gets rather than an escrow that
//!   would fail at the server (req/152 §5 measured exactly that failure for the shape-blind form).

use gx_core::{ObjectSnapshot, ReadEntry, SubstrateKind};
use gx_substrate::{Error, InvertOutcome, PlannedDelta, Result};

use crate::catalogue::{self, Catalogue, IdentityFault, OnReadFailure};
use crate::delta::{restore_arguments, McpDelta, McpOp, MAX_INVERSE_PAYLOAD_BYTES};
use crate::locator;
use crate::transport::ToolTransport;

/// 🔴 **DR-46-12** — the sentence a refused effect carries, verbatim.
///
/// A constant rather than a `format!` at the site, for `req/262` M-02's reason: `req/263` §6 G6
/// measured that a refusal is only worth having if the remedy it names can be executed without a
/// reader adding a word, and a probe can only hold the wording to that standard if there is one
/// wording to hold. `tests/github16_read_by_tool.rs` compares the refusal against this constant
/// and `docs/LIMITS.md` v0.5-d carries the same two remedies in prose.
/// 🔴 **`req/269` M-04** (`req/38` §198 ruling (b), §199 ruling 3): the closing clause used to
/// promise "reversibility **recorded as** unknown", and there is no place on a receipt where that
/// word is recorded — `unknown` and `false` are byte-identical in a receipt's payload and its
/// signature (the audit compared them). The sentence now says exactly what §198 (b) ruled the
/// public face may say and no more, until DR-46-13 gives the payload a seventh word.
pub const READ_FAILURE_REFUSAL: &str = "the prior contents this restore declaration draws from \
     could not be read, so no inverse could be escrowed and the effect is refused rather than \
     applied unescrowed. What to fix: make the declared read face answer, or add \
     `\"$on_read_failure\": \"unknown\"` to the restore catalogue to accept the effect with its \
     reversibility unknown -- which is recorded in this refusal text, in the adapter's verdict, and \
     on the receipt itself, where unknown and false are no longer the same bytes (DR-46-26)";

/// 🔴 **`req/269` M-05** (`req/38` §199 ruling 3) — the sentence a **declaration** failure carries,
/// verbatim, and it is deliberately not [`READ_FAILURE_REFUSAL`].
///
/// The audit measured what happened when the two shared a sentence. A read declaration that named
/// a prior member (`{"prior_json": ...}` inside its own `read_by`) is a catalogue the operator
/// wrote wrong; the server has done nothing. The road folded it into the read-failure answer
/// "because from the escrow's side the two failures are one fact", so the operator was told to
/// *make the declared read face answer* — a face that had never been called, with **zero arrivals**
/// on the server's side of the wire — or to add `"$on_read_failure": "unknown"`, which is
/// executable, and which turns a typo into a permanent `unknown` on every call while never reading
/// anything.
///
/// So: the cause first, one remedy, and the relaxation is **not printed**, because it is not a
/// remedy for this.
/// 🔴 **Narrowed by `req/303` M-03, not rewritten.** Every word above and below stands, and the
/// sentence is now appended only where its subject exists — an entry fault whose
/// [`crate::DeclarationFace`] is `ReadFace`. R20 added two entry faults that are about a
/// `restored_by` and an `arguments` template, and appending *"The declared read face was never
/// called"* to a catalogue that declares no read face at all made a true mechanism print a false
/// record with an unexecutable remedy. Those take
/// [`DECLARATION_WITHOUT_A_READ_FACE_REFUSAL`] instead.
pub const DECLARATION_UNSOUND_REFUSAL: &str = "this restore catalogue's read declaration is not \
     sound, so no read was attempted and the effect is refused. The declared read face was never \
     called and the server did not fail. What to fix: correct the read declaration in the restore \
     catalogue";

/// 🔴 **`req/303` M-03** (`req/38` §221 ruling 5) — the sentence a declaration fault that is
/// **not about a read face** carries, verbatim.
///
/// # Why a sixth constant rather than a wider fifth
///
/// [`DECLARATION_UNSOUND_REFUSAL`] makes two claims that are only true of a read face: that the
/// declaration in question is *the read declaration*, and that *the declared read face was never
/// called*. The twenty-first audit ran a catalogue with no `read_by` anywhere in it — a
/// `restored_by` of `" "` — and read both claims back, followed by a remedy (*correct the read
/// declaration*) with nothing in the file to correct. Widening the old sentence to cover both would
/// have cost its precision on the road where it is right; this is `req/269` M-05's own argument for
/// splitting a read failure from a declaration failure, applied one level down.
///
/// The two faces this covers are [`crate::DeclarationFace::RestoreFace`] and
/// [`crate::DeclarationFace::Template`]. The wrapping sentence already names which, so this
/// sentence says only what is common: nothing was read, nothing was sent, and the file is what to
/// fix. The relaxation (`"$on_read_failure": "unknown"`) is **not printed**, for the same reason it
/// is not printed by [`DECLARATION_UNSOUND_REFUSAL`]: it is a decision about a server that will not
/// answer, and offering it here is offering a way to make a typo permanent.
pub const DECLARATION_WITHOUT_A_READ_FACE_REFUSAL: &str = "this restore catalogue's declaration \
     for that tool is not sound, so no inverse could be built and the effect is refused. This \
     entry declares no read face, nothing was read, and nothing was sent to the server. What to \
     fix: correct that entry in the restore catalogue";

/// 🔴 **DR-46-15** (`req/38` §199 ruling 2) — the sentence a read whose answer is about **another
/// object** carries, verbatim.
///
/// `crate::catalogue::ObjectIdentity` holds the argument. In one line: the compare-and-set
/// attests the locator's resource and the escrow carries whatever the read tool answered about, so
/// unless a declaration says how the second names the first, an undo can overwrite a change nobody
/// ever attested. This is the refusal when it does not.
pub const OBJECT_IDENTITY_REFUSAL: &str = "the declared read answered about a different object \
     than the one this change is about, so the prior it carries is not this object's prior and no \
     inverse was escrowed. What to fix: point the change at the object the read answers for, or \
     correct the read declaration's `identity` in the restore catalogue (DR-46-15: an escrow is \
     the prior of the object the compare-and-set attests, or it is not an escrow)";

/// 🔴 **`req/279` M-01** — the sentence a read whose answer could **not be read at all** carries,
/// and it is deliberately not [`OBJECT_IDENTITY_REFUSAL`].
///
/// The nineteenth audit measured five shapes — an answer that is not JSON (a rate-limit HTML page
/// is the realistic one), not UTF-8, empty, nested past the parser's recursion limit, and one
/// whose `identity` pointer resolves to nothing — and every one of them was refused with
/// DR-46-15's sentence: *the declared read answered about a different object*. All five are
/// **false**. Nothing was read, so nothing named an object, so nothing named a different one; and
/// the remedy that sentence prints, "point the change at the object the read answers for", cannot
/// be executed by a reader who has not been told what the read answered for, because it did not
/// answer.
///
/// This is `req/269` M-05's species one gate further along — a fault wearing another fault's face
/// — and it takes the repair R17 chose there: the cause first, then the remedies that are
/// executable for *this* cause. The relaxation is not printed for the same reason it is not
/// printed by [`DECLARATION_UNSOUND_REFUSAL`]: `"$on_read_failure": "unknown"` does reach this
/// road, but offering it here is offering a way to make an unreadable answer permanent.
pub const OBJECT_UNNAMED_REFUSAL: &str = "no object could be read out of the declared read's \
     answer, so nothing established which object the escrowed prior would belong to and no \
     inverse was escrowed. What to fix: correct the read declaration's `identity` in the restore \
     catalogue, or check what the declared read face actually answers (DR-46-15: an escrow is the \
     prior of the object the compare-and-set attests, or it is not an escrow)";

/// 🔴 **`req/279` M-03** — the sentence an `identity` that never reads its answer carries.
///
/// [`crate::catalogue::ObjectIdentity::soundness`] is a parse-time check and it is syntactic: it
/// asks whether an `answer` part is **present**. The audit wrote one that is present and spells
/// nothing — `{"answer": "/pad"}` against a server whose `/pad` is always the empty string — so
/// the resource was assembled out of the forward call alone, matched the locator (of course: the
/// locator came from the same call), and gx answered `true` while escrowing a stranger's text
/// against this object. That is `req/269` M-03 restored intact behind the check meant to close it.
///
/// A declaration fault rather than a read failure, so the remedy is the declaration and the
/// relaxation is not offered — the same shape as [`DECLARATION_UNSOUND_REFUSAL`], and unlike it
/// this one cannot be judged at parse time: whether a member spells anything is a fact about the
/// server's answer, not about the file.
pub const IDENTITY_IGNORES_THE_ANSWER_REFUSAL: &str = "the read declaration's `identity` names a \
     member of the read's answer that spelled nothing, so the object was named by the change's own \
     call and the read's answer was never checked -- which is the binding this declaration exists \
     to make. No inverse was escrowed. What to fix: point `identity` at a member the read face \
     actually answers with (`req/279` M-03: naming the answer is syntax, reading it is the check)";

/// 🔴 **DR-46-21** (`req/38` §218 ruling 2, §417/§421, `req/593` §11-§16) — the sentence a
/// **content-addressed** CAS read whose answer does not hash to the CID its locator names carries,
/// verbatim.
///
/// `crate::cas::read_subject` holds the mechanism, and it is deliberately **not** one of the escrow
/// constants above. Where the escrow road binds a read's answer with
/// [`crate::catalogue::ObjectIdentity`] (DR-46-15), a CAS read's answer is the object's **raw
/// bytes** and carries no `/id` to bind on — `tests/dr46_21_identity_binding_gap.rs` arm 2 measured
/// that the escrow predicate cannot even parse it. The binding a content-addressed read already has
/// is the one content-addressing gives: the object's identity **is** its digest. So when the value
/// keying the read is a `gx1:` CID, `read_subject` re-verifies `content_digest(answer)` against it
/// and refuses a mismatch fail-closed — `req/269` H-01 one road over, the dishonest server that
/// answers a neighbour's bytes. Reusing any escrow sentence here would tell a reader to correct an
/// `identity` member this road has no field for, or to fix an inverse this road never escrows, so
/// the different fact takes a different sentence (the census `r22` holds that it does).
pub const CAS_OBJECT_DIGEST_REFUSAL: &str = "the declared read answered bytes whose digest is not \
     the digest this content-addressed locator names, so its answer is about a different object \
     than the compare-and-set is keyed to and gx refused it rather than recording a stranger's \
     bytes as this object's state. What to fix: make the read face answer the object whose digest \
     keys this locator, or key the locator by the digest of the object the read answers (DR-46-21: \
     a content-addressed read's answer is the object its digest names, or it is not that object)";

/// Build the delta that undoes `delta` from the state `pre` (41 §4, DR-1(a)).
///
/// # Errors
/// [`Error::LocatorMismatch`] when `pre` is a snapshot of another object (**E-M4-32**): a delta and a
/// snapshot of two different objects is a wiring bug in whoever assembled the call, and answering
/// `Ok(None)` would send it down the escalation path wearing the face of a legitimate business
/// condition (**E-M4-27**'s argument). [`Error::ForeignDelta`] for another adapter's delta,
/// [`Error::PayloadUnreadable`] for bytes this grammar did not write, [`Error::Unimplemented`] for a
/// sequence v0.1 does not run, [`Error::NotAPosition`] for a payload whose locator is not a position,
/// and [`Error::Unreadable`] when the server will not answer for the resource.
// 🔴 **DR-46-26** — this function has no caller left inside the crate: `McpAdapter::invert` now
// hands the whole `InvertOutcome` up, so the fold to a bare `Option` happens (where it still
// happens at all) in the engine and in the conformance harness. It is kept rather than deleted
// (no-delete) because it is the one place the `Option`-only reading of this road is written down,
// and `AC-D2` records that the *other* adapters' free `invert`s were likewise left alone.
#[allow(dead_code)]
pub(crate) fn invert(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    delta: &PlannedDelta,
    pre: &ObjectSnapshot,
) -> Result<Option<PlannedDelta>> {
    invert_with_verdict(transport, catalogue, delta, pre).map(InvertOutcome::into_inverse)
}

/// [`invert`], with **C-25's three-valued answer** beside the inverse (`req/38` §196, DR-46-9 A-4).
///
/// One function rather than two so that the verdict is the one this call actually produced and not
/// a second derivation of it — and so that answering the verdict costs the same single read the
/// escrow costs, never two.
///
/// | answer | what happened |
/// |---|---|
/// | [`Reversibility::True`] + `Some` | an inverse was built and is ready to escrow |
/// | [`Reversibility::False`] + `None` | no tool undoes this one, or the declaration cannot be resolved from this call, or the body is over **M4-21**'s ceiling |
/// | [`Reversibility::Unknown`] + `None` | the prior would not be read, and this deployment declared [`OnReadFailure::Unknown`] |
/// | `Err` | the prior would not be read under the default posture: the effect is refused |
///
/// 🔴 **DR-46-26** — the four rows above are now carried by [`InvertOutcome`], and the fourth is
/// **not** a fourth value of C-25: an `Err` here fails the commit closed, so neither a receipt nor
/// an escrow row is written and no downstream reader ever has to tell it apart from the other
/// three. The outcome also carries `reads` — the **one** object this function read, `{digest,
/// locator}`, on whichever of the two roads it took. `req/350` §1's measurement is that there is
/// exactly one of them per escrow; the engine's `ReadSet::from_reads` is what decides whether a set
/// of that size is G3 or G4, and this function does not (`req/441` §4).
///
/// # Errors
/// [`invert`]'s, plus [`Error::Unreadable`] carrying [`READ_FAILURE_REFUSAL`] when a declared read
/// failed under [`OnReadFailure::Refuse`].
pub(crate) fn invert_with_verdict(
    transport: &dyn ToolTransport,
    catalogue: &Catalogue,
    delta: &PlannedDelta,
    pre: &ObjectSnapshot,
) -> Result<InvertOutcome> {
    if delta.substrate() != &SubstrateKind::Mcp {
        return Err(Error::ForeignDelta {
            expected: SubstrateKind::Mcp,
            got: delta.substrate().clone(),
        });
    }
    let decoded = McpDelta::decode(delta.payload())?;
    let op = decoded
        .ops()
        .first()
        .expect("decode refuses the empty sequence");
    let position = op.position()?;

    let named = locator::normalize(pre.locator());
    if named != position.locator() {
        return Err(Error::LocatorMismatch {
            expected: position.locator(),
            got: named,
        });
    }

    let Some(restore) = catalogue.spec_for(op.tool()) else {
        // "gx does not know how to undo this", which is a fact about the deployment's declaration and (sem: SEM-gx-adapter-mcp-049)
        // not about this call. E-M3-4 asks a person.
        return Ok(InvertOutcome::none(Vec::new()));
    };

    // 🔴 **DR-46-9 A-3** — one read, either way. A declaration that names a read tool takes that
    // road *instead of* `resources/read`, never in addition to it, so the critical section T-10b
    // opens is the width it already was: one server round trip before `apply`, and `req/38` §195's
    // clause ⑤ window is not widened by this lane.
    // 🔴 **`req/279` H-01 / M-04 / L-03 + `req/291` H-01 / M-04** — the file-local questions, asked
    // here for `req/269` M-05's reason: a catalogue built in code (`Catalogue::with_restore_template`,
    // `Catalogue::with_prior_read`) never met a parser, so a check that lived only in `from_json`
    // would be a check a binary could ship past. `entry_soundness` is the same predicate `from_json`
    // runs, over the same catalogue, so the two roads refuse in one wording.
    //
    // 🔴 Asked **before** the road forks rather than inside the read-face arm. Until `req/291` the
    // call sat in the `Some(prior_read)` branch, so the whole `resources/read` road — which is
    // where the twentieth audit's forward-only template actually destroyed an object — was checked
    // by the parser alone. A declaration this crate can judge is judged on both roads or on neither.
    // 🔴 **`req/303` M-03** — the closing sentence is chosen by the fault's own face. Until R22 it
    // was always [`DECLARATION_UNSOUND_REFUSAL`], which asserts that a *declared read face* was
    // never called; R20's two new entry faults are about a `restored_by` and an `arguments`
    // template, and a catalogue that declares no read face at all was told about one.
    catalogue
        .entry_fault(op.tool())
        .map_err(|fault| Error::Unreadable {
            locator: position.locator(),
            detail: format!(
                "{}: {}",
                fault.why(),
                if fault.face().is_about_a_read_face() {
                    DECLARATION_UNSOUND_REFUSAL
                } else {
                    DECLARATION_WITHOUT_A_READ_FACE_REFUSAL
                }
            ),
        })?;

    let read = match restore.read_by() {
        None => transport.read(position.server(), position.resource()),
        Some(prior_read) => match prior_read.arguments_from_forward(op.arguments()) {
            Ok(arguments) => {
                transport.read_prior_by_tool(position.server(), prior_read.by_tool(), &arguments)
            }
            // 🔴 **`req/269` M-05** -- a read this **deployment** declared wrong, which is not a
            // read that failed. It leaves here immediately, in its own words, without consulting
            // the posture: `$on_read_failure` is a decision about what to do when a *server* will
            // not answer, and offering it as the remedy for a typo is offering a way to make the
            // typo permanent. The cause comes first because it is the only thing a reader can act
            // on.
            Err(detail) => {
                return Err(Error::Unreadable {
                    locator: position.locator(),
                    detail: format!("{detail}: {DECLARATION_UNSOUND_REFUSAL}"),
                })
            }
        },
    };
    let contents = match read {
        Ok(contents) => contents,
        // 🔴 **DR-46-12** — fail-closed is the default and it is also the pre-existing behaviour:
        // a `resources/read` that failed has always left this function as `Err` and failed the
        // commit closed. What is new is that a deployment may say, in writing, that it would
        // rather have the effect; and when it does, the answer is `Unknown` -- never `False`,
        // because nothing here established that no inverse exists.
        Err(error) => {
            return match catalogue.on_read_failure() {
                OnReadFailure::Refuse if restore.read_by().is_some() => {
                    // 🔴 **`req/324` M-01 / M-02 (`req/38` §231 ruling 2)** — ask first, compose
                    // second, and ask through the **one** funnel.
                    //
                    // M-01: `req/322` §3-1's sibling sweep table has four rows and every one of
                    // them is a consumer of `cas::read_subject`. This is the crate's *other*
                    // function that reaches `ToolTransport::read_prior_by_tool`, it was on no row,
                    // and the same server decision — a JSON-RPC `-32002`, the code equality the
                    // whole discrimination rests on — reached the agent here with no word saying
                    // which of the two preimages it was. `req/38` §227 ruling 2 is the standing
                    // rule: the same question, asked in one place, by every gate.
                    //
                    // M-02: and the order is load-bearing. The token's whole value is its
                    // **position** (`crate::transport::read_answered_absent` asks for it with
                    // `strip_prefix`, so a server cannot forge it from inside its own message), and
                    // the line below wraps the transport's string in gx's own words. Measured: gx
                    // pushed its own token to offset 509 and its own predicate then answered
                    // `false` about it. So the funnel is handed `error` — what the *transport*
                    // wrote — and the composition happens to what the funnel returns.
                    //
                    // 🔴 And the **position** is kept, not merely the words. R24 gave the token an
                    // invariant — it is at offset 0 of `Unreadable.detail`, so a server cannot
                    // forge it from inside its own message — and a road that consumes the token
                    // early and then emits a `detail` without it leaves that invariant true on
                    // some roads and false on others. So on the answered-absent road the named
                    // detail leads and the refusal follows. Only that road is reordered: a read
                    // that genuinely did not answer keeps the sentence a shipped deployment
                    // already sees, which is the zero-regression half of DR-46-12.
                    let absent = match &error {
                        Error::Unreadable { detail, .. } => {
                            crate::transport::read_answered_absent(detail)
                        }
                        _ => false,
                    };
                    let named = crate::adapter::name_the_preimage(error);
                    let detail = match (&named, absent) {
                        (Error::Unreadable { detail, .. }, true) => {
                            format!("{detail} {READ_FAILURE_REFUSAL}")
                        }
                        _ => format!("{READ_FAILURE_REFUSAL} ({named})"),
                    };
                    Err(Error::Unreadable {
                        locator: position.locator(),
                        detail,
                    })
                }
                // The `resources/read` road answers in the transport's own words, exactly as it
                // did before this window: zero regression on the sentence a shipped deployment
                // already sees.
                OnReadFailure::Refuse => Err(error),
                // 🔴 **DR-46-26** — no entry: the read is what failed, so there is no `{digest,
                // locator}` to attest. An `Unknown` with an empty read-set is the honest pair.
                OnReadFailure::Unknown => Ok(InvertOutcome::undetermined(Vec::new())),
            };
        }
    };
    // 🔴 **DR-46-26** — the read this escrow performed, as the entry a receipt attests.
    //
    // It is built **here**, at the one place in this function where a read has answered, so that
    // every road below it carries the same value and none of them can forget to. `locator` is
    // `position.resource()` and not whatever the declared tool called the object: DR-46-15 (below)
    // refuses the call outright when those two differ, so by the time any road returns a `reads`
    // that is non-empty, the two are the same string. `digest` is `content_digest` — the same
    // function `snapshot` uses, so an entry names the object by the value the CAS is watching.
    let reads = vec![ReadEntry {
        digest: crate::adapter::content_digest(&contents),
        locator: position.resource().to_string(),
    }];

    // 🔴 **DR-46-15** (`req/38` §199 ruling 2) -- the escrow is the prior of the object the
    // compare-and-set attests, or it is not an escrow.
    //
    // `snapshot`, `precondition` and the post-apply observation all quantify over
    // `position.resource()`. This read quantified over whatever the declared tool answered about,
    // and until this window nothing asked whether those were one object. `req/269` H-01 measured
    // the consequence on the only deployment the road actually has: an undo silently overwrote a
    // third party's write, because the compare-and-set was watching a file nobody had touched.
    //
    // The declaration says how the answer names the object; this says the name has to be the
    // locator's. It travels through `OnReadFailure` rather than out of the function, because from
    // the escrow's side the fact is the same one DR-46-12 already ruled on: the prior this
    // transformation needed did not arrive.
    if let Some(prior_read) = restore.read_by() {
        let named = prior_read
            .identity()
            .resource_from(&contents, op.arguments())
            .map(|spelled| locator::normalize(&spelled));
        let bound = match &named {
            Ok(spelled) => spelled == position.resource(),
            Err(_) => false,
        };
        if !bound {
            return match catalogue.on_read_failure() {
                OnReadFailure::Refuse => Err(Error::Unreadable {
                    locator: position.locator(),
                    // 🔴 **`req/279` M-01 and M-03** — three causes, three sentences. Until this
                    // window all of them ended in `OBJECT_IDENTITY_REFUSAL`, so an answer nobody
                    // could read and an identity that read nothing were both told they had named
                    // *a different object*, with a remedy neither reader could execute.
                    detail: match named {
                        // A read that named an object, and named the wrong one. DR-46-15's own
                        // case, and the only one whose sentence is still DR-46-15's.
                        //
                        // 🔴 **`req/279` L-04**: `spelled` is the server's text and a server's text
                        // has no length. `bounded` puts a ceiling on what enters the sentence; the
                        // constant is untouched.
                        Ok(spelled) => format!(
                            "the declared read answered for {:?} and this change is about {:?}: \
                             {OBJECT_IDENTITY_REFUSAL}",
                            catalogue::bounded(&spelled),
                            position.resource()
                        ),
                        Err(fault) => {
                            let constant = match fault {
                                IdentityFault::Unread(_) => OBJECT_UNNAMED_REFUSAL,
                                IdentityFault::AnswerNotRead(_) => {
                                    IDENTITY_IGNORES_THE_ANSWER_REFUSAL
                                }
                            };
                            format!("{}: {constant}", fault.detail())
                        }
                    },
                }),
                // 🔴 **DR-46-26** — no entry here either, and for DR-46-15's reason rather than
                // for the arm above's: a read answered, and it answered about a **different**
                // object. Attesting it would put a locator in the signed bytes that this
                // transformation's escrow did not read.
                OnReadFailure::Unknown => Ok(InvertOutcome::undetermined(Vec::new())),
            };
        }
    }
    // Which of the two restore conventions the declaration takes (`catalogue.rs`, "two restore (sem: SEM-gx-adapter-mcp-050)
    // forms"): no template is v0.1's `{contents, uri}` DAG-CBOR, unchanged; a template resolves to (sem: SEM-gx-adapter-mcp-051)
    // the restore tool's own structured JSON arguments, built from the forward call and the prior
    // contents -- both live here and only here, because after `apply` the pre-state is gone
    // (43 T-10b) and the forward arguments are this delta's own payload.
    //
    // 🔴 Two-phase escrow (`req/38` §98, ruling 1): a template naming do-result members resolves (sem: SEM-gx-adapter-mcp-052)
    // everything else now — this function stays the pure function E-M4-5 folds at verify time,
    // deterministic in (delta, pre) — and the do-time members travel as the op's `pending` map.
    // The engine sees the partial through `InverseCompletion::needs_completion` and escrows it
    // `Pending`; nothing here reads a server's do-time answer, because at both of this function's
    // call sites (T-3 and T-10b) none exists yet.
    let op_out = match restore.template() {
        None => McpOp::call(
            position.locator(),
            restore.restored_by().to_string(),
            restore_arguments(position.resource(), &contents)?,
        ),
        Some(template) => match template.resolve_split(op.arguments(), &contents) {
            Ok((arguments, pending)) if pending.is_empty() => McpOp::call(
                position.locator(),
                restore.restored_by().to_string(),
                arguments,
            ),
            Ok((arguments, pending)) => McpOp::call_pending(
                position.locator(),
                restore.restored_by().to_string(),
                arguments,
                pending,
            ),
            // The third `Ok(None)` of the module doc: a declaration this call does not carry the
            // material for. E-M3-4 asks a person, the same direction an undeclared tool takes.
            Err(_unresolvable) => return Ok(InvertOutcome::none(reads)),
        },
    };
    let payload = McpDelta::one(op_out).encode()?;
    if payload.len() > MAX_INVERSE_PAYLOAD_BYTES {
        // **M4-21**: over the ceiling there is no escrow, and E-M3-4 escalates rather than this
        // adapter quietly carrying an unbounded body into a journal.
        //
        // 🔴 **DR-46-26** — the read still happened and is still attested. This is the shape the
        // read-set exists for: "gx read your file, and then declined to escrow an inverse for it"
        // is a different fact from "gx never looked", and a receipt that carried neither would say
        // the second while the first is true.
        return Ok(InvertOutcome::none(reads));
    }
    PlannedDelta::new(SubstrateKind::Mcp, payload)
        .map(|inverse| InvertOutcome::inverted(inverse, reads))
}
