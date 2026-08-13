//! Scope arithmetic: the half of `Fingerprint` that lives above gx-core (**E-M4-1**).
//!
//! 41 §2 files this module under `gx-substrate/src/{lib,adapter,delta,fingerprint}.rs` and 42 §0
//! files the `Fingerprint` type here too. **E-M4-1** moved the type to gx-core and re-read this row:
//!
//! > 「`Fingerprint` 実型(substrate/scope/digest)は **gx-core**・**scope 決定と比較の計算は
//! > gx-substrate**(E-M2-1「型は下層・計算は上層」・M3-13 先例)…42 §0 の fingerprint.rs 行は「計算の
//! > 所在」と読み替える」
//!
//! So the file exists after all, and this is what is in it: the arithmetic gx-core cannot do. Today
//! that is one function, because M4 hand 4 needs one; `cas_eq` stayed with the type because a
//! comparison of two values is not arithmetic on a scope, and moving it would have made gx-witness
//! depend on an adapter crate to compare two fields.
//!
//! # Why the elision is here rather than beside the bound
//!
//! **M4H1-2 採(a)** (req/38 §29) asks for two things: 「gx-core にも scope 文字列の上限+超過時 digest 化
//! (gx-gate M3-21/ASM-60-4 と同形・1024 byte)」. The bound is in gx-core, at
//! [`gx_core::Fingerprint::new`], where it cannot be routed around. The digest cannot be: A-1 gives
//! gx-canon the only hash in the workspace and 41 §2 keeps gx-core's dependencies at 「serde,
//! thiserror程度」, so a gx-core that elided would be a gx-core that hashes.
//!
//! The split costs nothing and buys the stronger half. An adapter calls [`elide_scope`] and then
//! constructs; an adapter that forgets is refused rather than silently carrying a kilobyte of path
//! into a receipt gx-witness signs. M4 hand 4 raises the divergence from the ruling's literal wording
//! as an起票 in `req/73` §2 rather than deciding it.

use gx_canon::cid::{self, IdentityView};
use gx_core::MAX_SCOPE_BYTES;

use crate::error::{Error, Result};

/// Fold a scope down to the bound, keeping its identity (**M4H1-2**, 42 §3.8's 「長文は digest 化」).
///
/// Under [`gx_core::MAX_SCOPE_BYTES`] the scope is itself. Over it, the text is replaced by a line
/// naming how many bytes were elided and the CID of what they were -- the same shape
/// `gx_gate::verdict::elide` gives an over-long message under M3-21, and for the same reason: the
/// record still identifies the state without carrying the whole spelling into the bytes a receipt is
/// hashed over.
///
/// # It has to be a function of the text, and that is not a stylistic point
///
/// A scope is **compared** and never parsed: 42 §3.5's equality asks whether two fingerprints name
/// one state, and [`gx_core::Fingerprint::cas_eq`] refuses outright when two scopes differ
/// (**E-M4-15**). Two reads of one long path therefore have to elide to the same line, or a CAS
/// check would answer 「その比較は意味を持たない」 about a file comparing with itself. The digest goes
/// through [`cid::compute`] for that reason and for 41 §6's: one encoder, so an operator holding the
/// original path can recompute the line and confirm it is the one this produced.
///
/// # Errors
/// [`Error::NotDigestible`] when gx-canon cannot encode the text. A `String` always has a canonical
/// DAG-CBOR form, so the arm is unreachable today; it is typed rather than unwrapped because 41 §6
/// counts a panic as a bug -- the same judgement `gx_gate::verdict::elide` records.
pub fn elide_scope(scope: String) -> Result<String> {
    if scope.len() <= MAX_SCOPE_BYTES {
        return Ok(scope);
    }
    let digest = cid::compute(&ScopeText(&scope)).map_err(|e| Error::NotDigestible {
        detail: e.to_string(),
    })?;
    Ok(format!(
        "<elided: {} bytes over the {MAX_SCOPE_BYTES}-byte bound of M4H1-2, {}>",
        scope.len(),
        cid::to_text(&digest)
    ))
}

/// A scope too long to carry, as something with an identity.
struct ScopeText<'t>(&'t str);

impl IdentityView for ScopeText<'_> {
    type View<'a>
        = &'a str
    where
        Self: 'a;

    fn identity_view(&self) -> Self::View<'_> {
        self.0
    }
}
