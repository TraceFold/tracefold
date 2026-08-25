// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The append-only tree: leaves, entries, tiles, and the hash arithmetic over them.
//!
//! Spec: 42 §3.11 for every field table and for the two hash rules, 32 FR-021/FR-024 for what the
//! crate owes, 34 AC-021/AC-024 for how that is judged, RFC 6962 §2.1 for the tree definition
//! 42 §3.11 says it reuses ("reuses the design from Certificate Transparency (RFC 6962) as-is") (sem: SEM-gx-log-076).
//!
//! # Written here, not imported
//!
//! **M2H1-6 → hand 2 ruling** (`req/38_ERRATA_2026-08-07.md` §9): the merkle tree is gx's own. Three (sem: SEM-gx-log-077)
//! reasons are recorded there — gx hashes with BLAKE3 (35 DR-3) while `rs_merkle` pulls a second
//! SHA-256 implementation into a workspace that needs none (req/50 §6 measured the duplicated
//! `sha2` subtree), 42 §3.11's domain separation is no library's default, and the licence claim
//! did not check out at three points. `rs_merkle` was observed and **no line of it was copied**;
//! what is transcribed below is the tree definition from RFC 6962 §2.1, which is a specification.
//!
//! # The tree
//!
//! `MTH({})` is undefined here rather than defined as the hash of nothing: 42 §3.11's `Checkpoint`
//! carries a `root_hash` and a log of zero entries has no head worth signing, so [`TileLog::root`]
//! answers `None` and every proof over an empty tree is an error. RFC 6962 does define a value for
//! it; gx has nowhere to put one, and minting a spelling for a value nothing consumes is how a
//! second spelling gets into a canon.
//!
//! For `n > 1` the tree splits at `k`, the largest power of two **strictly** below `n`:
//!
//! ```text
//! MTH(D[1])   = leaf_hash(d[0])
//! MTH(D[n])   = node_hash(MTH(D[0..k]), MTH(D[k..n]))
//! ```
//!
//! The split is what makes the tree append-only in shape as well as in API: a left subtree is
//! always complete, so appending never rewrites a hash that was already published — which is what
//! lets [`TileLog::tile`] hand out immutable, cacheable blocks (42 §3.11: "a fixed-size chunk
//! that a CDN can cache" (sem: SEM-gx-log-078)).

use gx_canon::cid;
use gx_core::{Cid, Timestamp, TransformationId};
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Leaves per full tile (42 §3.11: `width` is 1..=256, "the default tile width is 256") (sem: SEM-gx-log-079).
///
/// The upstream vocabulary calls the same number a "tile height" (sem: SEM-gx-log-080) of 8, meaning 2^8 leaves; AC-024
/// compares both readings against a reference vector.
pub const TILE_WIDTH: u16 = 256;

/// 42 §3.11 states the range, and nothing else in this crate does.
///
/// # Why this one compile-time assertion and not a dozen
///
/// M2 hand 6 went looking for length and layout invariants worth asserting at compile time
/// (req/49 §5, batch 9) and found that almost all of them are already refused by the type checker: a (sem: SEM-gx-log-081)
/// `Cid` is `[u8; 32]` by construction, a key seed is `[u8; SECRET_KEY_LENGTH]` taken from
/// ed25519-dalek's own constant, both alphabets are `&[u8; N]`, and `Tile::width` is a `u16` so
/// `u16::try_from(hashes.len())` in [`TileLog::tile`] cannot exceed it. A `const _: () =
/// assert!(...)` over any of those would restate what already fails to compile.
///
/// This one is different because the constraint is a **range stated in prose in the canonical doc** (sem: SEM-gx-log-082) and no
/// type expresses it. `TILE_WIDTH` is a `u16`, so `512` compiles, resolves, and passes every test
/// in this crate -- the tile arithmetic is width-agnostic -- while silently leaving 42 §3.11's
/// "1..=256" (sem: SEM-gx-log-083) and putting AC-024's comparison against the upstream vector out of reach. So it
/// stops being a build.
const _: () = assert!(
    TILE_WIDTH >= 1 && TILE_WIDTH <= 256,
    "42 §3.11: `width` is 1..=256" // (sem: SEM-gx-log-084)
);

/// What a leaf hash is taken over (42 §3.11: "the equivalent of `LedgerEntry`'s IdentityView") (sem: SEM-gx-log-085).
///
/// # Field order
///
/// Declared in encoded-key order — `index`, `receipt_digest`, `transformation` — while 42 §3.11
/// lists them as `transformation, receipt_digest, index`. The *set* is 42's; the order is the one
/// the encoder writes, which E-42-3 (`req/38_ERRATA_2026-08-07.md` §4) settles as being over the
/// **encoded** key: length first, letters second. For this leaf the two readings agree, since all
/// three names are of different lengths and already in letter order.
///
/// # This convention is a readability rule, not a checked one
///
/// Hand 2 recorded a safety net for it (req/51 §3.6: "even with the wrong order, `encode`
/// fails with `NotCanonicalizable`" (sem: SEM-gx-log-086)). Hand 3 measured it and there is none: `serde_ipld_dagcbor`
/// sorts a struct's keys itself, so a misordered declaration produces the same canonical bytes and
/// fails nothing. `tests/map_key_order.rs` holds the measurement and req/52 §4 H3-1 the ticket.
/// Writing the fields in encoded order is still worth doing — it makes the canonical form the
/// obvious form — but it is a convention a reader keeps, not one a build keeps.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LedgerLeaf {
    /// Leaf number, from 0, in append order.
    pub index: u64,
    /// BLAKE3 digest of the `Receipt` (the whole DSSE envelope). 42 §3.11 keeps the receipt body
    /// out of the leaf so a leaf stays small.
    pub receipt_digest: Cid,
    pub transformation: TransformationId,
}

/// One appended record (42 §3.11).
///
/// `leaf_cid` is a cache of [`leaf_hash`] over [`LedgerEntry::leaf`], stored because a reader that
/// walks the log should not have to re-encode every entry. **No verifier reads it**:
/// `proof::verify_inclusion` recomputes the hash from the fields, because an entry whose contents
/// and whose cached hash were both edited would otherwise verify (AC-022's tamper case).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LedgerEntry {
    /// Metadata, and only that (ASM-4). It is not part of the leaf, so the same commit appended at
    /// two times hashes the same.
    pub appended_at: Timestamp,
    pub index: u64,
    /// `leaf_hash(self.leaf())`, cached.
    pub leaf_cid: Cid,
    pub receipt_digest: Cid,
    pub transformation: TransformationId,
}

impl LedgerEntry {
    /// The projection a leaf hash is taken over (42 §3.11).
    ///
    /// A method rather than a field, for the reason `IdentityView` is a projection in gx-canon: a
    /// stored copy could disagree with the entry it came from, and then two answers would exist to
    /// "what is this leaf" (sem: SEM-gx-log-087).
    #[must_use]
    pub fn leaf(&self) -> LedgerLeaf {
        LedgerLeaf {
            index: self.index,
            receipt_digest: self.receipt_digest,
            transformation: self.transformation,
        }
    }
}

/// A block of hashes at one level (42 §3.11, Rekor v2 / C2SP tlog-tiles layout).
///
/// `hashes` holds the roots of the complete subtrees at `level` covered by this tile, so
/// `hashes.len() == width`. A partial tile is shorter than [`TILE_WIDTH`] and says so; the ragged
/// tail of the tree has no hash at any level above 0 until it completes, which is what makes a
/// written tile final.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Tile {
    /// Roots of the complete subtrees this tile covers, in index order.
    pub hashes: Vec<Cid>,
    /// Tile number at this level: the first tile covers subtrees `0..256`, the second `256..512`.
    pub index: u64,
    /// 0 = the leaf layer, N = the layer of complete 2^N-leaf subtrees.
    ///
    /// # gx's `level` is a tree **height**, and the upstream's is not (**E-M2-27**)
    ///
    /// `req/38_ERRATA_2026-08-07.md` §15 verbatim: "(b) gx's `subtree_span(level)=2^level` 'level' is
    /// **the tree's height**, and it is a different thing at an 8x scale from a primary tile level (1
    /// level = 8 steps of height = 256x) -- **document it, and consider renaming the field to `height`**
    /// (adopt the rename if the wire stays unchanged)" (sem: SEM-gx-log-088).
    ///
    /// One gx level is one doubling: level `N` holds the roots of complete `2^N`-leaf subtrees. One
    /// C2SP `tlog-tiles` level is **eight** doublings, because a tile there is 256 wide and each
    /// level consumes a whole tile's worth of them -- so its level 1 covers `2^8` leaves per entry
    /// and its level `L` covers `2^(8L)`. The two vocabularies agree at 0 and at nothing else:
    /// gx level 8 is upstream level 1.
    ///
    /// **The field is not renamed.** 42 §3.11's table names it `level`, and the name is on the wire:
    /// it is a map key in the canonical DAG-CBOR of a `Tile`, so `height` would be a different byte
    /// string for the same value. The ruling adopts the rename only "if the wire stays unchanged" (sem: SEM-gx-log-089), and here it is
    /// not -- `tests/tile_wire.rs` holds the golden that says so, and the rename is taken where the
    /// wire genuinely does not see it: the private `subtree_span`, whose argument is a height and
    /// is now spelled one.
    pub level: u8,
    /// 1..=[`TILE_WIDTH`].
    pub width: u16,
}

/// `leaf_hash = BLAKE3(0x00 || canonical_dagcbor(LedgerLeaf))` (42 §3.11).
///
/// Delegates to gx-canon's mint (**E-M2-12**), which is what keeps 41 §6's "all canonical encoding
/// goes through gx-canon alone" (sem: SEM-gx-log-090) true of this crate: gx-log names neither a codec nor a hash.
///
/// # Errors
/// [`Error::Canon`] if the leaf has no canonical form. It always does today — three ids and an
/// integer — so this is the encoder's refusal surfacing rather than a case a caller meets.
pub fn leaf_hash(leaf: &LedgerLeaf) -> Result<Cid> {
    Ok(cid::mint_leaf(leaf)?)
}

/// `node_hash = BLAKE3(0x01 || left_hash || right_hash)` (42 §3.11).
///
/// Order-bearing and infallible; both children are already digests.
#[must_use]
pub fn node_hash(left: &Cid, right: &Cid) -> Cid {
    cid::mint_node(left, right)
}

/// The append-only log, in memory.
///
/// Durability is [`crate::store`]'s (NFR-009 / AC-069): this type holds the tree and computes over
/// it, and nothing here survives a process. Keeping the two apart is what lets the proofs be tested
/// without a filesystem.
///
/// So is idempotence. ASM-43-1 keys `ledger.append` on `TransformationId`, and the key index that
/// makes that decidable lives in [`crate::store::LedgerStore`]; **this type will append the same
/// transformation twice**. A caller who wants the ledger's contract has to hold the ledger. That
/// split is deliberate — a tree with a key index would carry state no proof consumes — and it is
/// raised as H3-2 in req/52 §4 because it is bypassable rather than enforced.
///
/// 🔴 **H3-2's ruling** (`req/38_ERRATA_2026-08-07.md` §12): "do not make `TileLog` a production
/// public-write road" (sem: SEM-gx-log-091). [`crate::store::LedgerStore`] is the only write road a deployment may expose — this
/// type is the tree a proof is computed over, and reaching it directly bypasses ASM-43-1's
/// exactly-once (INV-S3). M6, which decides what the CLI and the HTTP surface publish, is where the
/// rule has to hold; it is written here because this is the type it is about.
///
/// # Why the leaf hashes are kept beside the entries
///
/// A root is a fold over leaf hashes, and re-encoding every entry to recompute one would make a
/// proof O(n) encodes instead of O(n) hashes. The vector is derived state, written only by
/// [`TileLog::append`], and `entries[i].leaf_cid == leaves[i]` holds by construction.
///
/// # 🔴 **FR-M7-2 option A** — the completed tiles are kept too, and that is what makes a proof cheap (sem: SEM-gx-log-092)
///
/// `req/97` §3.1 measured `prove_inclusion` and found it **linear in `n`**: 1,000 → 64,000 leaves
/// cost 68× for 64× the leaves, while the proof itself grew from 10 hashes to 16. The cost was never
/// the proof's size — it was that every sibling on the path was a `mth` **recomputed over a slice**,
/// so one proof hashed the whole tree and `n` commits each producing one was O(n²). At n ≈ 22.8k that
/// one function was 3.4 ms of a 3.770 ms commit stage (§54 M6H7-3, confirmed numerically in §55).
///
/// `req/38` §56 additional ruling a adopted **option A** as an M7 implementation requirement: cache the root of (sem: SEM-gx-log-093)
/// every **completed** tile. 42 §3.11's tile layout already says a completed tile is final (a left
/// subtree is always complete, so appending never rewrites a published hash), which is exactly the
/// property that makes the cache safe to keep and impossible to invalidate.
///
/// * `tile_roots[t]` is `MTH(leaves[256t .. 256t+256])`, pushed by `TileLog::commit` (private) the moment
///   the 256th leaf of that tile arrives — 255 node hashes per 256 appends, i.e. **~1 hash per
///   append amortised**, against a proof that used to hash the whole tree.
/// * The partial tail is **not** cached, because it is not final.
/// * Extra state is 32 bytes per 256 leaves (8 KB at n = 64k) and it is derived: a log rebuilt by
///   replay computes the same vector from the same appends.
///
/// 🔴 **The cache is authoritative, not a shortcut.** [`TileLog::root`], [`TileLog::root_at`] and the
/// audit path all go through `TileLog::mth_range` (private), which reads `tile_roots` for any aligned power-
/// of-two block of whole tiles. There is no second road that recomputes the same answer: an
/// implementation that stopped maintaining the cache would produce a **wrong root**, not a slow one,
/// and `the_cached_fold_is_the_recursive_fold` is red on the first size past 256. A cache with a
/// fallback would have been a cache no test could tell was gone.
///
/// **`InclusionProof`'s wire form does not move**: `{leaf_index, tree_size, audit_path}`, the same
/// values in the same order, since the tree is the same tree. `tests/incremental_inclusion.rs`
/// compares against an independent transcription of RFC 6962 §2.1.1 rather than against this code.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TileLog {
    entries: Vec<LedgerEntry>,
    leaves: Vec<Cid>,
    /// `MTH` of each **completed** 256-leaf block, in index order (**FR-M7-2 option A**). (sem: SEM-gx-log-094)
    tile_roots: Vec<Cid>,
}

impl TileLog {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one record and return it (FR-021).
    ///
    /// One of the two `&mut self` methods this crate offers, and the other is
    /// [`crate::store::LedgerStore::append`] — which is the shape AC-021 checks: every writer
    /// appends, so there is no road from a caller to an entry that is already in the log.
    ///
    /// The index is assigned here rather than taken as an argument — a caller that could choose it
    /// could write two entries at one position. `appended_at` *is* taken as an argument, because
    /// 41 §6 injects time at the engine boundary and a log that read the clock itself would make
    /// replay non-deterministic.
    ///
    /// # Errors
    /// [`Error::Canon`] if the leaf cannot be encoded.
    pub fn append(
        &mut self,
        transformation: TransformationId,
        receipt_digest: Cid,
        appended_at: Timestamp,
    ) -> Result<&LedgerEntry> {
        let entry = self.stage(transformation, receipt_digest, appended_at)?;
        self.commit(entry);
        Ok(self.entries.last().expect("just committed"))
    }

    /// The entry [`TileLog::append`] would produce, without producing it.
    ///
    /// Split out for [`crate::store`], which has to put the record on the disk *before* it becomes
    /// visible here (NFR-009): staging computes the leaf hash and touches nothing, so a write that
    /// fails leaves the tree exactly as it was. An append-only log has no way to take an entry
    /// back, so the alternative — commit, write, undo on failure — has no undo to reach for.
    ///
    /// This is also the weaker form of the typestate `crate::store`'s module docs decline: the
    /// ordering it would enforce is expressible as two calls, and the borrow checker already
    /// refuses to commit what was never staged.
    ///
    /// # Errors
    /// [`Error::Canon`] if the leaf has no canonical form.
    pub(crate) fn stage(
        &self,
        transformation: TransformationId,
        receipt_digest: Cid,
        appended_at: Timestamp,
    ) -> Result<LedgerEntry> {
        let index = self.len();
        let leaf = LedgerLeaf {
            index,
            receipt_digest,
            transformation,
        };
        // Hashed before anything is stored: a leaf with no canonical form must leave the log
        // exactly as it was, not half-appended.
        let leaf_cid = leaf_hash(&leaf)?;
        Ok(LedgerEntry {
            appended_at,
            index,
            leaf_cid,
            receipt_digest,
            transformation,
        })
    }

    /// Put a staged entry into the tree.
    ///
    /// Crate-private, and it appends: the index is not a parameter and an entry whose index is not
    /// the next one is a bug in the caller, not an insertion point. Debug-asserted rather than
    /// returned, because the only two callers are [`TileLog::append`] and the store's replay, both
    /// of which get the index from this very log.
    ///
    /// # Panics
    /// Debug-asserts that `entry.index` is the length of the log.
    pub(crate) fn commit(&mut self, entry: LedgerEntry) {
        debug_assert_eq!(
            entry.index,
            self.len(),
            "an append-only tree takes the next index and no other"
        );
        self.leaves.push(entry.leaf_cid);
        self.entries.push(entry);
        // **FR-M7-2 option A**: the tile this leaf completed, folded once and kept. 255 node hashes per (sem: SEM-gx-log-095)
        // 256 appends, and never again — 42 §3.11's completed tile is final.
        let width = usize::from(TILE_WIDTH);
        if self.leaves.len().is_multiple_of(width) {
            let start = self.leaves.len() - width;
            let root = mth(&self.leaves[start..]).expect("a full tile is not empty");
            self.tile_roots.push(root);
        }
    }

    /// The roots of the completed tiles, in index order (**FR-M7-2 option A**). (sem: SEM-gx-log-096)
    ///
    /// Derived state, exposed so that its maintenance can be **measured** rather than trusted:
    /// `tests/incremental_inclusion.rs` compares this vector with a fold computed from the leaves
    /// and compares its length with `len() / 256`. That is the same shape `SHIPPED_PACKS` has in
    /// gx-gate — a declaration and the thing it declares, checked against each other — and it is why
    /// this accessor is not the consumer-less kind **M6-21** armed a probe against.
    #[must_use]
    pub fn cached_subtree_roots(&self) -> &[Cid] {
        &self.tile_roots
    }

    /// How many entries the log holds.
    #[must_use]
    pub fn len(&self) -> u64 {
        self.entries.len() as u64
    }

    /// Whether the log holds none.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The entry at `index`, if the log has reached it.
    #[must_use]
    pub fn entry(&self, index: u64) -> Option<&LedgerEntry> {
        self.entries.get(usize::try_from(index).ok()?)
    }

    /// Every entry, in append order.
    #[must_use]
    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Every leaf hash, in append order.
    #[must_use]
    pub fn leaf_hashes(&self) -> &[Cid] {
        &self.leaves
    }

    /// The current root, or `None` for an empty log (see the module docs).
    #[must_use]
    pub fn root(&self) -> Option<Cid> {
        self.mth_range(0, self.leaves.len())
    }

    /// The root the log had when it held `tree_size` entries.
    ///
    /// This is what makes a checkpoint from the past checkable against the log as it stands: the
    /// prefix of an append-only tree is a tree, and its root is a fold over the same leaves.
    /// `None` for `tree_size == 0` or for a size the log has not reached.
    #[must_use]
    pub fn root_at(&self, tree_size: u64) -> Option<Cid> {
        let size = usize::try_from(tree_size).ok()?;
        if size == 0 || size > self.leaves.len() {
            return None;
        }
        self.mth_range(0, size)
    }

    /// `MTH(leaves[start..end])`, reading the tile cache wherever the range is a whole number of
    /// **aligned** tiles (**FR-M7-2 option A**). (sem: SEM-gx-log-097)
    ///
    /// # The two conditions, and the derivation that says they are enough
    ///
    /// `MTH` splits at `k`, the largest power of two **strictly** below the length. For a range of
    /// `256·m` leaves that `k` is `256 · split_point(m)`: multiplying by 256 shifts the binary
    /// expansion by eight places and moves neither the position of the highest set bit nor the
    /// strictness of "below" (sem: SEM-gx-log-098). So a range that starts on a tile boundary and holds `m` whole tiles
    /// splits into two ranges that each start on a tile boundary and each hold a whole number of
    /// tiles — for **every** `m`, not only for powers of two — down to `m = 1`, where the range is
    /// one tile and the recursion's leaf **is** the cached root.
    ///
    /// ∴ `MTH` over the leaves of such a range and `MTH` over the corresponding slice of
    /// `tile_roots` are the same tree, and the two conditions are alignment and divisibility.
    /// (An earlier draft of this function also required `m` to be a power of two. It is correct and
    /// it is unnecessary: the derivation above holds for `m = 3` exactly as it does for `m = 4`, and
    /// requiring it would have left two thirds of the sizes on the slow road. Written down because a
    /// condition nobody can say why is a condition the next hand copies.)
    ///
    /// 🔴 The **alignment** test is the opposite case: it is redundant for every call the two entry
    /// points make (`an_unaligned_range_is_folded_from_the_leaves` carries the derivation) and it is
    /// kept, because this is a general range fold and a caller that does not preserve the property
    /// would otherwise get a wrong answer silently. The battery's point (b) removes it, and the
    /// probe that makes that point red is the one that calls this function outside the recursion.
    ///
    /// A range that is neither splits normally and its halves are asked the same question; the
    /// ragged tail below 256 leaves is folded from the leaves. So one proof costs `O(n/256)` node
    /// hashes over cached blocks plus at most a few hundred for the tails, instead of `O(n)`.
    ///
    /// `the_cached_fold_is_the_recursive_fold` checks the claim against [`mth`] at every prefix of
    /// every size it runs, rather than leaving it to the paragraph above.
    ///
    /// `None` only for an empty range, exactly as [`mth`] answers.
    fn mth_range(&self, start: usize, end: usize) -> Option<Cid> {
        let n = end.checked_sub(start)?;
        let width = usize::from(TILE_WIDTH);
        match n {
            0 => None,
            1 => Some(self.leaves[start]),
            _ => {
                if start.is_multiple_of(width) && n.is_multiple_of(width) {
                    let (first, last) = (start / width, end / width);
                    if last <= self.tile_roots.len() {
                        return mth(&self.tile_roots[first..last]);
                    }
                }
                let k = split_point(n);
                let left = self.mth_range(start, start + k)?;
                let right = self.mth_range(start + k, end)?;
                Some(node_hash(&left, &right))
            }
        }
    }

    /// The sibling hashes from leaf `index` up to the root of the first `tree_size` leaves
    /// (RFC 6962 §2.1.1's `PATH`), with every sibling folded through [`TileLog::mth_range`].
    ///
    /// Bottom-up, the sibling nearest the leaf first — the order `verify_inclusion` walks, and the
    /// same order the free [`audit_path`] produces. This is the whole of **FR-M7-2**: the recursion
    /// is unchanged and only the fold under it moved.
    pub(crate) fn audit_path_at(&self, index: usize, tree_size: usize, out: &mut Vec<Cid>) {
        self.walk_to_root(index, 0, tree_size, out);
    }

    fn walk_to_root(&self, index: usize, start: usize, end: usize, out: &mut Vec<Cid>) {
        let n = end - start;
        if n <= 1 {
            return;
        }
        let k = split_point(n);
        if index < start + k {
            self.walk_to_root(index, start, start + k, out);
            out.push(
                self.mth_range(start + k, end)
                    .expect("the right half of a split is non-empty"),
            );
        } else {
            self.walk_to_root(index, start + k, end, out);
            out.push(
                self.mth_range(start, start + k)
                    .expect("the left half of a split is non-empty"),
            );
        }
    }

    /// The `index`-th tile of `level` (42 §3.11).
    ///
    /// `None` when the tile holds nothing yet: past the end of the level, or at a level no
    /// complete subtree has reached. An absent tile and an empty one are different answers, and
    /// conflating them would make "the log has no 16-leaf subtree yet" look like "here is an
    /// empty block" (sem: SEM-gx-log-099).
    ///
    /// `level` is gx's, which is a tree height: one level is one doubling, not the eight a C2SP
    /// `tlog-tiles` level covers ([`Tile::level`], E-M2-27).
    #[must_use]
    pub fn tile(&self, level: u8, index: u64) -> Option<Tile> {
        let span = subtree_span(level)?;
        let complete = self.leaves.len() / span;
        let start = usize::try_from(index.checked_mul(u64::from(TILE_WIDTH))?).ok()?;
        if start >= complete {
            return None;
        }
        let end = complete.min(start + usize::from(TILE_WIDTH));

        let hashes = (start..end)
            .map(|i| mth(&self.leaves[i * span..(i + 1) * span]).expect("a complete subtree"))
            .collect::<Vec<_>>();
        let width = u16::try_from(hashes.len()).expect("at most TILE_WIDTH");

        Some(Tile {
            hashes,
            index,
            level,
            width,
        })
    }
}

/// How many leaves one node at `height` covers, or `None` if that is more than a `usize` holds.
///
/// **The argument is a height** (**E-M2-27**). It was called `level` until this fix, and the word is
/// overloaded: 42 §3.11's `Tile.level` counts one doubling per step, while the C2SP `tlog-tiles`
/// level this crate's tile design follows counts eight of them (256 leaves per entry, per level). A
/// reader who took the upstream's meaning would read `2^level` as off by a factor of 256 at every
/// level above 0. gx's number is the height of the subtree a node roots, and `2^height` is its leaf
/// count, which is what this function returns.
///
/// The rename stops at this function's argument. `Tile.level` is 42 §3.11's field name and travels
/// on the wire as a map key, so renaming *it* would change the canonical bytes -- which the ruling's
/// "adopt the rename if the wire stays unchanged" (sem: SEM-gx-log-100) forbids, and `tests/tile_wire.rs` measures.
fn subtree_span(height: u8) -> Option<usize> {
    if u32::from(height) >= usize::BITS {
        return None;
    }
    Some(1usize << height)
}

/// `MTH(D[n])` — the merkle tree hash of a run of leaf hashes (RFC 6962 §2.1, 42 §3.11).
///
/// `None` for the empty run; see the module docs for why that is not an error but an absence.
pub(crate) fn mth(leaves: &[Cid]) -> Option<Cid> {
    match leaves.len() {
        0 => None,
        1 => Some(leaves[0]),
        n => {
            let k = split_point(n);
            let left = mth(&leaves[..k])?;
            let right = mth(&leaves[k..])?;
            Some(node_hash(&left, &right))
        }
    }
}

/// The largest power of two **strictly** less than `n` (RFC 6962 §2.1's `k`).
///
/// Strictly: `split_point(4) == 2`, not 4. That is what makes the left subtree of every tree
/// complete and therefore stable under append — the property the whole tile layout rests on.
///
/// # Panics
/// Debug-asserts `n >= 2`. `mth` handles 0 and 1 before reaching here, and a caller that did not
/// would be asking for the split of a tree that has none.
pub(crate) fn split_point(n: usize) -> usize {
    debug_assert!(n >= 2, "a tree of {n} leaves does not split");
    1usize << (usize::BITS - 1 - (n - 1).leading_zeros())
}

/// The sibling hashes from leaf `index` up to the root of `leaves` (RFC 6962 §2.1.1's `PATH`).
///
/// Bottom-up: the sibling nearest the leaf first, which is the order `verify_inclusion` walks.
///
/// 🔴 **This is now the reference and not the road** (**FR-M7-2**). Until M7 hand 5 it was what
/// `prove_inclusion_at` called; the shipped path is [`TileLog::audit_path_at`], which folds each
/// sibling through the tile cache. This function stays, compiled **for tests only**, because a
/// re-implementation needs an oracle and the oracle that matters is the code that was believed
/// before — `the_cached_audit_path_is_the_recursive_path` compares the two at every index of every
/// size it runs. It is not deleted and it is not shipped: "do not delete" and "do not ship two roads" (sem: SEM-gx-log-101) are
/// both satisfied by `#[cfg(test)]`.
#[cfg(test)]
pub(crate) fn audit_path(index: usize, leaves: &[Cid], out: &mut Vec<Cid>) {
    let n = leaves.len();
    if n <= 1 {
        return;
    }
    let k = split_point(n);
    if index < k {
        audit_path(index, &leaves[..k], out);
        out.push(mth(&leaves[k..]).expect("the right half of a split is non-empty"));
    } else {
        audit_path(index - k, &leaves[k..], out);
        out.push(mth(&leaves[..k]).expect("the left half of a split is non-empty"));
    }
}

/// RFC 6962 §2.1.2's `SUBPROOF(m, D[n], b)`, the recursion `PROOF` is built from.
///
/// `b` records whether the subtree being described is the whole of the old tree. When it is not —
/// `b == false` at a leaf of the recursion — the old subtree's own root has to be named, because
/// the verifier cannot derive it from anything it already holds.
pub(crate) fn subproof(m: usize, leaves: &[Cid], b: bool, out: &mut Vec<Cid>) {
    let n = leaves.len();
    if m == n {
        if !b {
            out.push(mth(leaves).expect("a non-empty subtree"));
        }
        return;
    }
    let k = split_point(n);
    if m <= k {
        subproof(m, &leaves[..k], b, out);
        out.push(mth(&leaves[k..]).expect("the right half of a split is non-empty"));
    } else {
        subproof(m - k, &leaves[k..], false, out);
        out.push(mth(&leaves[..k]).expect("the left half of a split is non-empty"));
    }
}

/// Turn a `u64` size into the `usize` the slices need, refusing rather than truncating.
pub(crate) fn size_to_usize(size: u64, what: &str) -> Result<usize> {
    usize::try_from(size).map_err(|_| Error::Malformed {
        detail: format!("{what} {size} does not fit this machine's address space"),
    })
}

/// [`split_point`] over sizes that need not fit a `usize` (**H2-5**).
///
/// The same `k` as RFC 6962 §2.1 — the largest power of two strictly below `n` — computed on the
/// declared size of a tree rather than on a slice this machine holds. A verifier checking a proof
/// about a log of 2^40 leaves has no such slice, and refusing the proof because the *prover's*
/// tree does not fit in memory would be answering a different question. `tile.rs`'s unit tests
/// assert the two agree wherever both are defined.
///
/// # Panics
/// Debug-asserts `n >= 2`, as [`split_point`] does.
pub(crate) fn split_point_u64(n: u64) -> u64 {
    debug_assert!(n >= 2, "a tree of {n} leaves does not split");
    1u64 << (u64::BITS - 1 - (n - 1).leading_zeros())
}

/// How many siblings an audit path for leaf `index` of a tree of `tree_size` leaves must hold
/// (**H2-5**, `req/38_ERRATA_2026-08-07.md` §11).
///
/// `None` when there is no such leaf — an empty tree, or an index at or past the end — because
/// then there is no lawful length and no proof to have one.
///
/// The number is fixed by the two indices and by nothing else: the tree splits at
/// [`split_point_u64`], the leaf falls in one half, and each split contributes exactly one
/// sibling. It is not `ceil(log2(n))` — a ragged tree gives its rightmost leaves shorter paths
/// than its leftmost ones, and rounding would reject valid proofs at every size that is not a
/// power of two.
///
/// `O(log n)` integer operations and no hash, which is the point: `verify_inclusion` decides
/// whether a path *could* be right before spending BLAKE3 on whether it *is*, so the work a
/// verifier does is chosen by the tree and not by whoever sent the proof.
pub(crate) fn audit_path_len(index: u64, tree_size: u64) -> Option<usize> {
    if tree_size == 0 || index >= tree_size {
        return None;
    }
    let (mut i, mut n, mut len) = (index, tree_size, 0usize);
    while n > 1 {
        let k = split_point_u64(n);
        if i < k {
            n = k;
        } else {
            i -= k;
            n -= k;
        }
        len += 1;
    }
    Some(len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two split points are one function measured twice (H2-5).
    #[test]
    fn the_u64_split_point_agrees_with_the_usize_one() {
        for n in 2..=4_096usize {
            assert_eq!(split_point_u64(n as u64), split_point(n) as u64, "size {n}");
        }
    }

    /// A perfect tree gives every leaf the same path length, and it is the exponent.
    #[test]
    fn a_perfect_tree_has_one_path_length() {
        for exponent in 0..12u32 {
            let size = 1u64 << exponent;
            for index in 0..size {
                assert_eq!(
                    audit_path_len(index, size),
                    Some(exponent as usize),
                    "leaf {index} of {size}"
                );
            }
        }
    }

    /// A ragged tree does not: the last leaf of a tree of three is one step from the root while
    /// the first two are two. Written out because this is the case `ceil(log2(n))` gets wrong.
    #[test]
    fn a_ragged_tree_has_two_path_lengths() {
        assert_eq!(audit_path_len(0, 3), Some(2));
        assert_eq!(audit_path_len(1, 3), Some(2));
        assert_eq!(audit_path_len(2, 3), Some(1));
    }

    /// The lawful length is the length the generator produces, over every shape up to 64.
    #[test]
    fn the_lawful_length_is_the_generated_length() {
        let mut leaves: Vec<Cid> = Vec::new();
        for size in 1..=64u64 {
            let mut raw = [0u8; 32];
            raw[..8].copy_from_slice(&size.to_be_bytes());
            leaves.push(Cid(raw));
            for index in 0..size {
                let mut path = Vec::new();
                audit_path(index as usize, &leaves, &mut path);
                assert_eq!(
                    Some(path.len()),
                    audit_path_len(index, size),
                    "leaf {index} of {size}"
                );
            }
        }
    }

    /// A leaf the tree does not have has no lawful path length.
    #[test]
    fn a_leaf_outside_the_tree_has_no_length() {
        assert_eq!(audit_path_len(0, 0), None);
        assert_eq!(audit_path_len(8, 8), None);
        assert_eq!(audit_path_len(u64::MAX, 8), None);
    }

    // -----------------------------------------------------------------------
    // 🔴 FR-M7-2 option A — the cache against the recursion it replaced (sem: SEM-gx-log-102)
    // -----------------------------------------------------------------------

    /// A log of `n` leaves whose hashes are distinguishable without being a hash of anything.
    fn tree_of(n: usize) -> TileLog {
        let mut log = TileLog::default();
        for i in 0..n {
            let mut raw = [0u8; 32];
            raw[..8].copy_from_slice(&(i as u64).to_be_bytes());
            log.commit(LedgerEntry {
                appended_at: Timestamp(i as i64),
                index: i as u64,
                leaf_cid: Cid(raw),
                receipt_digest: Cid([1u8; 32]),
                transformation: TransformationId(Cid([2u8; 32])),
            });
        }
        log
    }

    /// The sizes worth walking every index of: both sides of a tile boundary, both sides of two, and
    /// the small ragged shapes where a wrong split point still produces *a* root.
    const CACHE_SIZES: [usize; 14] = [1, 2, 3, 4, 5, 7, 8, 255, 256, 257, 511, 512, 513, 700];

    /// 🔴 The cached fold **is** the recursive fold, at every prefix of every size.
    ///
    /// This is the probe that makes the cache authoritative rather than optional: a `commit` that
    /// stopped pushing a tile root, or a `mth_range` whose alignment guard was widened, answers a
    /// different root here — not a slower one. The reference is [`mth`], which reads leaves and
    /// knows nothing about tiles.
    #[test]
    fn the_cached_fold_is_the_recursive_fold() {
        for size in CACHE_SIZES {
            let log = tree_of(size);
            assert_eq!(log.root(), mth(&log.leaves), "root of {size}");
            for prefix in 1..=size {
                assert_eq!(
                    log.root_at(prefix as u64),
                    mth(&log.leaves[..prefix]),
                    "prefix {prefix} of {size}"
                );
            }
        }
    }

    /// 🔴 The cached audit path is the recursive audit path — same siblings, same order.
    ///
    /// `audit_path` is the code `prove_inclusion_at` called until M7 hand 5, kept for exactly this
    /// comparison (`#[cfg(test)]`). Every index of every size in [`CACHE_SIZES`], and every prefix
    /// tree of the two sizes where a prefix can straddle a tile boundary.
    #[test]
    fn the_cached_audit_path_is_the_recursive_path() {
        for size in CACHE_SIZES {
            let log = tree_of(size);
            for index in 0..size {
                let mut cached = Vec::new();
                log.audit_path_at(index, size, &mut cached);
                let mut reference = Vec::new();
                audit_path(index, &log.leaves, &mut reference);
                assert_eq!(cached, reference, "leaf {index} of {size}");
            }
        }
        // A proof issued against a tree the log has passed takes the same road.
        let log = tree_of(700);
        for tree_size in [1usize, 255, 256, 257, 512, 699, 700] {
            for index in 0..tree_size {
                let mut cached = Vec::new();
                log.audit_path_at(index, tree_size, &mut cached);
                let mut reference = Vec::new();
                audit_path(index, &log.leaves[..tree_size], &mut reference);
                assert_eq!(cached, reference, "leaf {index} of prefix {tree_size}");
            }
        }
    }

    /// 🔴 **The alignment guard, measured where the recursion cannot reach.**
    ///
    /// The battery's point (b) — drop `start.is_multiple_of(width)` from `mth_range`'s fast path —
    /// **survived its first run**, and the reason is a derivation rather than a missing case in a
    /// table: inside `mth_range`'s own recursion the start of any range whose length is a multiple of
    /// 256 is *already* a multiple of 256. (Every range's start is a multiple of its parent's split
    /// `k`; a range of `256·m` leaves has `k = 256·split_point(m)` for `m > 1`, and for `m = 1` the
    /// range is a child whose parent's `k` was at least 256. So the guard is implied for every call
    /// the two entry points can make.)
    ///
    /// That makes the mutation **equivalent for today's callers** and not for the function, which is
    /// a general range fold with a stated precondition. `mth_range(100, 356)` is a legal call, is
    /// wrong under the mutation, and no probe made it — so this one does, and the survivor is closed
    /// rather than classified. The alternative was to delete a guard nothing needs *yet*, which is
    /// how the next caller inherits a silent wrong answer.
    #[test]
    fn an_unaligned_range_is_folded_from_the_leaves() {
        let log = tree_of(1_000);
        let width = usize::from(TILE_WIDTH);
        for (start, end) in [
            (100, 100 + width),
            (1, 1 + width),
            (255, 255 + 2 * width),
            (300, 300 + width),
        ] {
            assert_eq!(
                log.mth_range(start, end),
                mth(&log.leaves[start..end]),
                "an unaligned range of {} leaves at {start}",
                end - start
            );
        }
    }

    /// A completed tile is cached and a partial one is not, and each cached value is that tile's
    /// own fold.
    #[test]
    fn only_completed_tiles_are_cached() {
        let width = usize::from(TILE_WIDTH);
        for size in [0usize, 1, 255, 256, 257, 511, 512, 513, 700] {
            let log = tree_of(size);
            assert_eq!(
                log.cached_subtree_roots().len(),
                size / width,
                "cached tiles at {size}"
            );
            for (t, cached) in log.cached_subtree_roots().iter().enumerate() {
                assert_eq!(
                    Some(*cached),
                    mth(&log.leaves[t * width..(t + 1) * width]),
                    "tile {t} of {size}"
                );
            }
        }
    }
}
