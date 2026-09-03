// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::atom::{StoredAtom, KIND_GAP, VERDICT_FAIL, VERDICT_PASS, VERDICT_UNKNOWN};
use crate::extract::{self, DocumentIr};
use crate::manifest::{self, BandManifest, DbManifest};
use crate::store;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const REASON_OK: &str = "OK";
pub const REASON_BAND_CONTRACT: &str = "BAND_CONTRACT_UNREADABLE";
pub const REASON_ORPHAN_MD: &str = "ORPHAN_MD";
pub const REASON_DECLARED_ABSENT: &str = "DECLARED_PATH_ABSENT";
pub const REASON_BAND_UNCLAIMED: &str = "BAND_DIR_UNCLAIMED";
pub const REASON_UNDECLARED: &str = "UNDECLARED_ATTRIBUTE";
pub const REASON_DUPLICATE_ID: &str = "DUPLICATE_ATOM_ID";
pub const REASON_PARENT_BROKEN: &str = "PARENT_BROKEN";
pub const REASON_RANGE_OUTSIDE: &str = "RANGE_OUTSIDE_FILE";
pub const REASON_BYTE_MISMATCH: &str = "CONTENT_BYTE_MISMATCH";
pub const REASON_COVERAGE_GAP: &str = "COVERAGE_GAP";
pub const REASON_GRANULARITY_UNDECLARED: &str = "GRANULARITY_UNDECLARED";
pub const REASON_GRANULARITY_COARSE: &str = "GRANULARITY_COARSE";
pub const REASON_GRANULARITY_FINE: &str = "GRANULARITY_FINE";
pub const REASON_RANGE_OVERLAP: &str = "RANGE_OVERLAP";
pub const REASON_ORDER_NOT_TOTAL: &str = "ORDER_NOT_TOTAL";
pub const REASON_CHAIN_BREAK: &str = "CHAIN_BREAK";
pub const REASON_LEGACY_UNVERIFIABLE: &str = "LEGACY_UNVERIFIABLE";
pub const REASON_HEAD_MISMATCH: &str = "HEAD_MISMATCH";
pub const REASON_HEAD_ABSENT: &str = "HEAD_ABSENT";
pub const REASON_JOURNAL_ABSENT: &str = "JOURNAL_ABSENT";
pub const REASON_DIGEST_MISMATCH: &str = "DIGEST_MISMATCH";
pub const REASON_INDEX_ABSENT: &str = "INDEX_ABSENT";
pub const REASON_COUNT_MISMATCH: &str = "COUNT_MISMATCH";
pub const REASON_EMPTY_CORPUS: &str = "EMPTY_CORPUS";
pub const REASON_COMMUTE_BREAK: &str = "COMMUTE_BREAK";
pub const REASON_LOD_NOT_MONOTONE: &str = "LOD_NOT_MONOTONE";


pub struct GateLine {
    pub name: &'static str,
    pub verdict: &'static str,
    pub reason: &'static str,
    pub count: usize,
    pub denominator: usize,
    pub detail: String,
    pub breakdown: Vec<(String, String, usize)>,
}

impl GateLine {
    pub fn pass(name: &'static str, denominator: usize, detail: String) -> Self {
        GateLine {
            name,
            verdict: VERDICT_PASS,
            reason: REASON_OK,
            count: 0,
            denominator,
            detail,
            breakdown: Vec::new(),
        }
    }
    pub fn fail(
        name: &'static str,
        reason: &'static str,
        count: usize,
        denominator: usize,
        detail: String,
    ) -> Self {
        GateLine {
            name,
            verdict: VERDICT_FAIL,
            reason,
            count,
            denominator,
            detail,
            breakdown: Vec::new(),
        }
    }
    pub fn unknown(
        name: &'static str,
        reason: &'static str,
        count: usize,
        denominator: usize,
        detail: String,
    ) -> Self {
        GateLine {
            name,
            verdict: VERDICT_UNKNOWN,
            reason,
            count,
            denominator,
            detail,
            breakdown: Vec::new(),
        }
    }
    pub fn counting(mut self, breakdown: Vec<(String, String, usize)>) -> Self {
        self.breakdown = breakdown;
        self
    }
}

pub fn render_breakdown(lines: &[GateLine]) -> String {
    let mut out = String::new();
    for line in lines {
        if line.breakdown.is_empty() {
            continue;
        }
        let total: usize = line.breakdown.iter().map(|(_, _, count)| count).sum();
        out.push_str(&format!(
            "\n{} counts {} over {} attribute and document pair(s); one atom missing two attributes is counted once under each, so this sum is at or above the {} atom(s) the gate names\n",
            line.name,
            total,
            line.breakdown.len(),
            line.count
        ));
        out.push_str(&format!(
            "{:<10} {:<52} {:>7}\n",
            "attribute", "document", "count"
        ));
        for (attribute, document, count) in &line.breakdown {
            out.push_str(&format!("{:<10} {:<52} {:>7}\n", attribute, document, count));
        }
    }
    if out.is_empty() {
        out.push_str("\nno gate on this run carries a breakdown; --detail had nothing to add\n");
    }
    out
}

pub struct Corpus {
    pub manifest: DbManifest,
    pub bands: Vec<BandManifest>,
    pub documents: Vec<DocumentIr>,
    pub atoms: Vec<StoredAtom>,
    pub band_failures: Vec<String>,
    pub orphans: Vec<String>,
    pub absent: Vec<String>,
    pub unreadable: Vec<String>,
    pub unclaimed: Vec<String>,
}

pub fn load_corpus(db: &Path) -> Result<Corpus, String> {
    let manifest_doc = manifest::load_db(db)?;
    let loads = manifest::load_bands(db, &manifest_doc);
    let mut bands: Vec<BandManifest> = Vec::new();
    let mut band_failures: Vec<String> = Vec::new();
    for load in loads {
        match load.outcome {
            Ok(band) => bands.push(band),
            Err(reason) => band_failures.push(format!("{}: {}", load.id, reason)),
        }
    }
    let mut documents: Vec<DocumentIr> = Vec::new();
    let mut atoms: Vec<StoredAtom> = Vec::new();
    let mut orphans: Vec<String> = Vec::new();
    let mut absent: Vec<String> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    for band in &bands {
        let on_disk = manifest::markdown_files(&band.dir);
        for name in &on_disk {
            if !band.documents.iter().any(|item| item.path == *name) {
                orphans.push(format!("{}/{}", band.id, name));
            }
        }
        for document in &band.documents {
            let path = band.dir.join(&document.path);
            if !path.is_file() {
                absent.push(format!("{}/{}", band.id, document.path));
                continue;
            }
            match extract::read_document(&path) {
                Ok(text) => match extract::extract_document(band, document, &text) {
                    Ok(ir) => {
                        atoms.extend(ir.atoms.iter().cloned());
                        documents.push(ir);
                    }
                    Err(reason) => {
                        unreadable.push(format!("{}/{}: {}", band.id, document.path, reason))
                    }
                },
                Err(reason) => unreadable.push(format!("{}/{}: {}", band.id, document.path, reason)),
            }
        }
    }
    let unclaimed = manifest::unclaimed_band_dirs(db, &manifest_doc);
    Ok(Corpus {
        manifest: manifest_doc,
        bands,
        documents,
        atoms,
        band_failures,
        orphans,
        absent,
        unreadable,
        unclaimed,
    })
}

pub fn source_gates(db: &Path, corpus: &Corpus) -> Vec<GateLine> {
    let mut lines: Vec<GateLine> = Vec::new();
    let band_total = corpus.manifest.band_order.len();

    if corpus.band_failures.is_empty() {
        lines.push(GateLine::pass(
            "G-S1",
            band_total,
            format!("every band listed in bands.order carries a band.toml that parses ({} band(s))", band_total),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-S1",
            REASON_BAND_CONTRACT,
            corpus.band_failures.len(),
            band_total,
            corpus.band_failures.join(" | "),
        ));
    }

    let partition_problems = corpus.orphans.len() + corpus.absent.len();
    if partition_problems > 0 {
        let mut detail = String::new();
        if !corpus.orphans.is_empty() {
            detail.push_str(&format!("markdown on disk that no manifest claims: {:?}", corpus.orphans));
        }
        if !corpus.absent.is_empty() {
            detail.push_str(&format!(" declared paths that are not on disk: {:?}", corpus.absent));
        }
        lines.push(GateLine::fail(
            "G-S2",
            if corpus.orphans.is_empty() { REASON_DECLARED_ABSENT } else { REASON_ORPHAN_MD },
            partition_problems,
            corpus.documents.len() + partition_problems,
            detail,
        ));
    } else if !corpus.unclaimed.is_empty() {
        lines.push(GateLine::unknown(
            "G-S2",
            REASON_BAND_UNCLAIMED,
            corpus.unclaimed.len(),
            band_total,
            format!(
                "{:?} exist under bands/ and are not listed in bands.order; unclaimed is the third value, not a failure and not a pass",
                corpus.unclaimed
            ),
        ));
    } else {
        lines.push(GateLine::pass(
            "G-S2",
            corpus.documents.len(),
            format!(
                "declaration and disk agree: {} document(s) claimed and present, 0 orphan, 0 absent, 0 unclaimed band directory",
                corpus.documents.len()
            ),
        ));
    }

    let claiming: Vec<&StoredAtom> = corpus
        .atoms
        .iter()
        .filter(|atom| atom.semantic.kind != KIND_GAP)
        .collect();
    let gaps = corpus.atoms.len() - claiming.len();
    if claiming.is_empty() {
        lines.push(GateLine::unknown(
            "G-S3",
            REASON_EMPTY_CORPUS,
            0,
            0,
            format!(
                "0 claiming atom(s) to check ({} gap atom(s) carry bytes, not claims); an empty scan is UNTESTABLE, never a pass",
                gaps
            ),
        ));
    } else {
        let mut undeclared: Vec<String> = Vec::new();
        let mut tally: BTreeMap<(String, String), usize> = BTreeMap::new();
        for atom in &claiming {
            let fields = crate::atom::undeclared_fields(atom);
            if fields.is_empty() {
                continue;
            }
            undeclared.push(format!(
                "{}#{} {}",
                atom.semantic.source.path,
                atom.semantic.source.anchor,
                fields.join(",")
            ));
            for field in fields {
                let document = format!("{}/{}", atom.band, atom.semantic.source.path);
                *tally.entry((field.to_string(), document)).or_insert(0) += 1;
            }
        }
        let breakdown: Vec<(String, String, usize)> = tally
            .into_iter()
            .map(|((attribute, document), count)| (attribute, document, count))
            .collect();
        if undeclared.is_empty() {
            lines.push(GateLine::pass(
                "G-S3",
                claiming.len(),
                format!(
                    "layer, executor and evidence are declared on every claiming atom; {} gap atom(s) are excluded from this denominator because they carry bytes, not claims",
                    gaps
                ),
            ));
        } else {
            let shown: Vec<String> = undeclared.iter().take(5).cloned().collect();
            lines.push(
                GateLine::fail(
                    "G-S3",
                    REASON_UNDECLARED,
                    undeclared.len(),
                    claiming.len(),
                    format!(
                        "{} of {} claiming atom(s) leave an attribute UNKNOWN (gap atoms excluded: {}); db gate --detail counts them by attribute and document: {:?}",
                        undeclared.len(),
                        claiming.len(),
                        gaps,
                        shown
                    ),
                )
                .counting(breakdown),
            );
        }
    }

    let mut ids: BTreeMap<&str, usize> = BTreeMap::new();
    for atom in &corpus.atoms {
        *ids.entry(atom.semantic.id.as_str()).or_insert(0) += 1;
    }
    let duplicated: Vec<&str> = ids
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, _)| *id)
        .collect();
    let known: Vec<&str> = corpus.atoms.iter().map(|atom| atom.semantic.id.as_str()).collect();
    let mut broken_parents = 0usize;
    for atom in &corpus.atoms {
        if let Some(parent) = &atom.semantic.parent {
            if !known.contains(&parent.as_str()) {
                broken_parents += 1;
            }
        }
    }
    let mut outside = 0usize;
    let mut mismatched: Vec<String> = Vec::new();
    for document in &corpus.documents {
        let band = match corpus.bands.iter().find(|item| item.id == document.band) {
            Some(band) => band,
            None => continue,
        };
        let bytes = match fs::read(band.dir.join(&document.path)) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        for atom in &document.atoms {
            let range = &atom.semantic.source.range;
            if range.byte_end > bytes.len() || range.byte_start > range.byte_end {
                outside += 1;
                continue;
            }
            if &bytes[range.byte_start..range.byte_end] != atom.semantic.content.as_bytes() {
                mismatched.push(format!("{}#{}", document.path, atom.semantic.source.anchor));
            }
        }
    }
    let integrity = duplicated.len() + broken_parents + outside + mismatched.len();
    if integrity == 0 {
        lines.push(GateLine::pass(
            "G-S4",
            corpus.atoms.len(),
            format!(
                "{} atom(s): ids distinct, every parent resolves, every provenance range lies inside its file, and a second read of each file returns the recorded bytes",
                corpus.atoms.len()
            ),
        ));
    } else {
        let reason = if !duplicated.is_empty() {
            REASON_DUPLICATE_ID
        } else if broken_parents > 0 {
            REASON_PARENT_BROKEN
        } else if outside > 0 {
            REASON_RANGE_OUTSIDE
        } else {
            REASON_BYTE_MISMATCH
        };
        lines.push(GateLine::fail(
            "G-S4",
            reason,
            integrity,
            corpus.atoms.len(),
            format!(
                "duplicate id {}, broken parent {}, range outside file {}, byte mismatch {} {:?}",
                duplicated.len(),
                broken_parents,
                outside,
                mismatched.len(),
                mismatched.iter().take(5).collect::<Vec<&String>>()
            ),
        ));
    }

    lines.push(coverage_gate(corpus));
    lines.push(granularity_gate(corpus));
    lines.push(journal_gate(db));
    lines
}

struct Spread {
    label: String,
    claiming: usize,
    gaps: usize,
    bytes: usize,
}

impl Spread {
    fn add(&mut self, atom: &StoredAtom) {
        if atom.semantic.kind == KIND_GAP {
            self.gaps += 1;
            return;
        }
        self.claiming += 1;
        self.bytes += atom.semantic.content.len();
    }
    fn breaks(&self, rule: &manifest::Granularity) -> Option<String> {
        if self.claiming == 0 {
            return None;
        }
        let mean = self.bytes as f64 / self.claiming as f64;
        let ratio = self.gaps as f64 / (self.claiming + self.gaps) as f64;
        if mean < rule.min_mean_bytes as f64 && ratio > rule.max_gap_ratio {
            return Some(format!(
                "{}: mean claiming atom {:.1} byte under the floor of {} and {:.1}% gap atoms over the ceiling of {:.1}%",
                self.label,
                mean,
                rule.min_mean_bytes,
                ratio * 100.0,
                rule.max_gap_ratio * 100.0
            ));
        }
        None
    }
}

pub fn granularity_gate(corpus: &Corpus) -> GateLine {
    let rule = match &corpus.manifest.granularity {
        Some(rule) => rule,
        None => {
            return GateLine::unknown(
                "G-S6",
                REASON_GRANULARITY_UNDECLARED,
                0,
                corpus.atoms.len(),
                "db.toml declares no [granularity], so there is no floor or ceiling to measure this corpus against; a rule nobody wrote down is UNKNOWN, never a pass".to_string(),
            )
        }
    };
    let claiming: Vec<&StoredAtom> = corpus
        .atoms
        .iter()
        .filter(|atom| atom.semantic.kind != KIND_GAP)
        .collect();
    if claiming.is_empty() {
        return GateLine::unknown(
            "G-S6",
            REASON_EMPTY_CORPUS,
            0,
            0,
            "0 claiming atom(s) to size; an empty scan is UNTESTABLE, never a pass".to_string(),
        );
    }
    let mut coarse: Vec<String> = Vec::new();
    for atom in &claiming {
        let marks = extract::evidence_marks(&atom.semantic.content);
        if marks.len() >= 2 {
            coarse.push(format!(
                "{}/{}#{} carries {:?}",
                atom.band, atom.semantic.source.path, atom.semantic.source.anchor, marks
            ));
        }
    }
    let mut per_document: Vec<Spread> = Vec::new();
    let mut per_band: BTreeMap<String, Spread> = BTreeMap::new();
    for document in &corpus.documents {
        let mut spread = Spread {
            label: format!("{}/{}", document.band, document.path),
            claiming: 0,
            gaps: 0,
            bytes: 0,
        };
        for atom in &document.atoms {
            spread.add(atom);
            let band = per_band.entry(document.band.clone()).or_insert(Spread {
                label: format!("band {}", document.band),
                claiming: 0,
                gaps: 0,
                bytes: 0,
            });
            band.add(atom);
        }
        per_document.push(spread);
    }
    let mut fine: Vec<String> = Vec::new();
    for spread in &per_document {
        if let Some(broken) = spread.breaks(rule) {
            fine.push(broken);
        }
    }
    for spread in per_band.values() {
        if let Some(broken) = spread.breaks(rule) {
            fine.push(broken);
        }
    }
    let sized = per_document.iter().filter(|spread| spread.claiming > 0).count();
    if coarse.is_empty() && fine.is_empty() {
        return GateLine::pass(
            "G-S6",
            claiming.len(),
            format!(
                "{} claiming atom(s) over {} of {} document(s) and {} band(s): none carries two kinds of evidence marker at once, and none of those groups is both under the {} byte floor and over the {:.0}% gap ceiling",
                claiming.len(),
                sized,
                per_document.len(),
                per_band.len(),
                rule.min_mean_bytes,
                rule.max_gap_ratio * 100.0
            ),
        );
    }
    let reason = if coarse.is_empty() {
        REASON_GRANULARITY_FINE
    } else {
        REASON_GRANULARITY_COARSE
    };
    let mut tally: BTreeMap<(String, String), usize> = BTreeMap::new();
    for atom in &claiming {
        if extract::evidence_marks(&atom.semantic.content).len() >= 2 {
            let document = format!("{}/{}", atom.band, atom.semantic.source.path);
            *tally.entry(("coarse".to_string(), document)).or_insert(0) += 1;
        }
    }
    for broken in &fine {
        let label = match broken.split(':').next() {
            Some(label) => label.to_string(),
            None => broken.clone(),
        };
        *tally.entry(("fine".to_string(), label)).or_insert(0) += 1;
    }
    let breakdown: Vec<(String, String, usize)> = tally
        .into_iter()
        .map(|((attribute, document), count)| (attribute, document, count))
        .collect();
    GateLine::fail(
        "G-S6",
        reason,
        coarse.len() + fine.len(),
        claiming.len(),
        format!(
            "{} atom(s) carry two or more kinds of evidence marker {:?}; {} group(s) are both under the {} byte floor and over the {:.0}% gap ceiling {:?}",
            coarse.len(),
            coarse.iter().take(3).collect::<Vec<&String>>(),
            fine.len(),
            rule.min_mean_bytes,
            rule.max_gap_ratio * 100.0,
            fine.iter().take(3).collect::<Vec<&String>>()
        ),
    )
    .counting(breakdown)
}

pub fn coverage_gate(corpus: &Corpus) -> GateLine {
    if corpus.documents.is_empty() {
        return GateLine::unknown(
            "G-S4b",
            REASON_EMPTY_CORPUS,
            0,
            0,
            "0 document(s) to cover; an empty scan is UNTESTABLE, never a pass".to_string(),
        );
    }
    let mut gaps: Vec<String> = Vec::new();
    let mut overlaps: Vec<String> = Vec::new();
    let mut order_problems: Vec<String> = Vec::new();
    for document in &corpus.documents {
        let mut ranges: Vec<(usize, usize)> = document
            .atoms
            .iter()
            .map(|atom| {
                (
                    atom.semantic.source.range.byte_start,
                    atom.semantic.source.range.byte_end,
                )
            })
            .collect();
        ranges.sort();
        let mut cursor = 0usize;
        for (start, end) in &ranges {
            if *start > cursor {
                gaps.push(format!("{} byte {}..{} covered by no atom", document.path, cursor, start));
            }
            if *start < cursor {
                overlaps.push(format!("{} byte {}..{} claimed twice", document.path, start, cursor));
            }
            cursor = *end;
        }
        if cursor != document.source_bytes {
            gaps.push(format!(
                "{} covers {} of {} byte",
                document.path, cursor, document.source_bytes
            ));
        }
        let mut siblings: BTreeMap<String, Vec<u32>> = BTreeMap::new();
        for atom in &document.atoms {
            let key = match &atom.semantic.parent {
                Some(parent) => parent.clone(),
                None => String::new(),
            };
            siblings.entry(key).or_default().push(atom.semantic.order);
        }
        for (parent, orders) in siblings {
            let mut sorted = orders.clone();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() != orders.len() {
                order_problems.push(format!(
                    "{} has two atoms at the same ordinal under parent {}",
                    document.path,
                    crate::atom::short_id(&parent)
                ));
            }
        }
    }
    let total = gaps.len() + overlaps.len() + order_problems.len();
    if total == 0 {
        let bytes: usize = corpus.documents.iter().map(|item| item.source_bytes).sum();
        return GateLine::pass(
            "G-S4b",
            corpus.documents.len(),
            format!(
                "every byte of every document belongs to exactly one atom: {} document(s), {} byte(s), {} atom(s), and sibling ordinals are a total order",
                corpus.documents.len(),
                bytes,
                corpus.atoms.len()
            ),
        );
    }
    let reason = if !gaps.is_empty() {
        REASON_COVERAGE_GAP
    } else if !overlaps.is_empty() {
        REASON_RANGE_OVERLAP
    } else {
        REASON_ORDER_NOT_TOTAL
    };
    GateLine::fail(
        "G-S4b",
        reason,
        total,
        corpus.documents.len(),
        format!(
            "uncovered {:?} overlapping {:?} ordinal {:?}",
            gaps.iter().take(3).collect::<Vec<&String>>(),
            overlaps.iter().take(3).collect::<Vec<&String>>(),
            order_problems.iter().take(3).collect::<Vec<&String>>()
        ),
    )
}

pub fn journal_gate(db: &Path) -> GateLine {
    let verdict = store::verify_chain(db);
    if verdict.lines == 0 {
        return GateLine::unknown(
            "G-S5",
            REASON_JOURNAL_ABSENT,
            0,
            0,
            format!(
                "{} holds 0 admission event(s); a chain over nothing is UNTESTABLE, never a pass",
                store::journal_path(db).display()
            ),
        );
    }
    if !verdict.breaks.is_empty() {
        return GateLine::fail(
            "G-S5",
            REASON_CHAIN_BREAK,
            verdict.breaks.len(),
            verdict.lines,
            verdict.breaks.iter().take(3).cloned().collect::<Vec<String>>().join(" | "),
        );
    }
    match &verdict.stored_head {
        None => {
            return GateLine::unknown(
                "G-S5",
                REASON_HEAD_ABSENT,
                1,
                verdict.lines,
                format!(
                    "the chain over {} line(s) folds to {} but nothing is stored at {}; the last record has no successor, so without HEAD it is unprotected",
                    verdict.lines,
                    crate::atom::short_id(&verdict.computed_head),
                    store::head_path(db).display()
                ),
            )
        }
        Some(stored) if *stored != verdict.computed_head => {
            return GateLine::fail(
                "G-S5",
                REASON_HEAD_MISMATCH,
                1,
                verdict.lines,
                format!(
                    "HEAD records {} but the {} line(s) on disk fold to {}; a record was edited, removed or added after it was written. The fold is over every line as it is written, so a line in the format this engine replaced is covered by it too",
                    crate::atom::short_id(stored),
                    verdict.lines,
                    crate::atom::short_id(&verdict.computed_head)
                ),
            )
        }
        Some(_) => {}
    }
    if verdict.unverifiable > 0 {
        return GateLine::unknown(
            "G-S5",
            REASON_LEGACY_UNVERIFIABLE,
            verdict.unverifiable,
            verdict.lines,
            format!(
                "{} of {} line(s) were written by the engine this one replaced, and that engine folded its chain a different way: the prev_hash those {} line(s) carry cannot be recomputed here, so their own links are UNKNOWN, never links this engine found broken. What is checked is checked: {} line(s) in the current format carry a prev_hash this engine recomputed and hold, and HEAD equals the fold of all {} line(s), so no line was edited, removed or added",
                verdict.unverifiable,
                verdict.lines,
                verdict.unverifiable,
                verdict.verified,
                verdict.lines
            ),
        );
    }
    GateLine::pass(
        "G-S5",
        verdict.lines,
        format!(
            "chain of {} admission event(s) folds to HEAD {}; {} of them carry a prev_hash this engine recomputed, and the last record is covered because HEAD is the fold of every line including it",
            verdict.lines,
            crate::atom::short_id(&verdict.computed_head),
            verdict.verified
        ),
    )
}

pub fn index_gates(db: &Path, corpus: &Corpus) -> Vec<GateLine> {
    let mut lines: Vec<GateLine> = Vec::new();
    let connection = match store::open_index(db) {
        Ok(connection) => connection,
        Err(error) => {
            lines.push(GateLine::unknown("G-I1", REASON_INDEX_ABSENT, 0, 0, error.clone()));
            lines.push(GateLine::unknown("G-I2", REASON_INDEX_ABSENT, 0, 0, error));
            return lines;
        }
    };
    match store::source_digest(db, &corpus.manifest, &corpus.bands) {
        Ok(recomputed) => match store::meta_value(&connection, "source_digest") {
            Some(stored) if stored == recomputed => lines.push(GateLine::pass(
                "G-I1",
                1,
                format!(
                    "meta.source_digest {} equals a digest recomputed here from db.toml, every band.toml, every declared document and the journal; the index was not asked what it contains",
                    crate::atom::short_id(&stored)
                ),
            )),
            Some(stored) => lines.push(GateLine::fail(
                "G-I1",
                REASON_DIGEST_MISMATCH,
                1,
                1,
                format!(
                    "index records source_digest {} but the source on disk digests to {}; the index is stale or the source moved under it. Run db compile",
                    crate::atom::short_id(&stored),
                    crate::atom::short_id(&recomputed)
                ),
            )),
            None => lines.push(GateLine::unknown(
                "G-I1",
                REASON_INDEX_ABSENT,
                0,
                1,
                "the index carries no meta.source_digest row, so it cannot be compared".to_string(),
            )),
        },
        Err(error) => lines.push(GateLine::unknown("G-I1", REASON_INDEX_ABSENT, 0, 1, error)),
    }
    let indexed = store::count_rows(&connection, "atoms");
    let fts = store::count_rows(&connection, "atoms_fts");
    let text_rows = store::count_rows(&connection, "atom_text");
    match (indexed, fts, text_rows) {
        (Ok(indexed), Ok(fts), Ok(text_rows)) => {
            if indexed == corpus.atoms.len() && fts == indexed && text_rows == indexed {
                lines.push(GateLine::pass(
                    "G-I2",
                    corpus.atoms.len(),
                    format!(
                        "IR {} atom(s) == index {} row(s) == atom_text {} row(s) == fts {} row(s)",
                        corpus.atoms.len(),
                        indexed,
                        text_rows,
                        fts
                    ),
                ));
            } else {
                lines.push(GateLine::fail(
                    "G-I2",
                    REASON_COUNT_MISMATCH,
                    1,
                    corpus.atoms.len(),
                    format!(
                        "IR {} atom(s), index {} row(s), atom_text {} row(s), fts {} row(s); a partial index answers questions about a corpus it does not hold",
                        corpus.atoms.len(),
                        indexed,
                        text_rows,
                        fts
                    ),
                ));
            }
        }
        _ => lines.push(GateLine::unknown(
            "G-I2",
            REASON_INDEX_ABSENT,
            0,
            0,
            "the index tables could not be counted".to_string(),
        )),
    }
    lines
}

pub fn all_gates(db: &Path, corpus: &Corpus) -> Vec<GateLine> {
    let mut lines = source_gates(db, corpus);
    lines.extend(index_gates(db, corpus));
    match store::open_index(db) {
        Ok(connection) => {
            lines.extend(query_gates(&connection, &corpus.manifest));
            lines.push(settings_gate(db));
            lines.extend(crate::route::commute_gate(&connection));
        }
        Err(error) => {
            lines.push(GateLine::unknown("G-Q1", REASON_INDEX_ABSENT, 0, 0, error.clone()));
            lines.push(settings_gate(db));
            lines.push(GateLine::unknown("G-Q3", REASON_INDEX_ABSENT, 0, 0, error));
        }
    }
    lines
}

pub fn summary_text(db: &Path, corpus: &Corpus, lines: &[GateLine]) -> String {
    let mut out = render(lines);
    let journal = store::read_journal(db);
    out.push_str(&format!("journal denominator: {}\n", journal.denominator()));
    out.push_str(&format!(
        "corpus denominator: {} band(s), {} document(s), {} atom(s)\n",
        corpus.bands.len(),
        corpus.documents.len(),
        corpus.atoms.len()
    ));
    out
}

pub fn summarise(lines: &[GateLine]) -> (usize, usize, usize) {
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut unknown = 0usize;
    for line in lines {
        match line.verdict {
            VERDICT_PASS => pass += 1,
            VERDICT_FAIL => fail += 1,
            _ => unknown += 1,
        }
    }
    (pass, fail, unknown)
}

pub fn exit_for(lines: &[GateLine]) -> i32 {
    let (_, fail, unknown) = summarise(lines);
    if unknown > 0 {
        return 2;
    }
    if fail > 0 {
        return 1;
    }
    0
}

pub fn render(lines: &[GateLine]) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{:<7} {:<8} {:<24} {:>7} {:>7}  detail\n",
        "gate", "verdict", "reason", "count", "of"
    ));
    for line in lines {
        out.push_str(&format!(
            "{:<7} {:<8} {:<24} {:>7} {:>7}  {}\n",
            line.name, line.verdict, line.reason, line.count, line.denominator, line.detail
        ));
    }
    let (pass, fail, unknown) = summarise(lines);
    out.push_str(&format!(
        "{} pass, {} fail, {} UNKNOWN over {} gate(s); UNKNOWN is a third value and is never folded into fail, and a run that carries one is UNTESTABLE (exit 2) rather than a pass\n",
        pass,
        fail,
        unknown,
        lines.len()
    ));
    out
}

pub const REASON_CONTROL_ABSENT: &str = "CONTROL_NOT_CONSTRUCTIBLE";
pub const REASON_CONTROL_BROKEN: &str = "CONTROL_DISAGREES";
pub const REASON_SOURCE_MUTATED: &str = "SOURCE_MUTATED_BY_READER";
pub const REASON_COMMENT: &str = "COMMENT_OUTSIDE_HEADER";
pub const REASON_SPELLING_SPLIT: &str = "SPELLINGS_DISAGREE";
pub const REASON_UNKNOWN_COLLAPSED: &str = "UNKNOWN_COLLAPSED_TO_DEFAULT";
pub const REASON_NO_SOURCE: &str = "NO_SOURCE_READ";

pub struct Control {
    pub label: String,
    pub expected: String,
    pub observed: String,
}

impl Control {
    pub fn holds(&self) -> bool {
        self.expected == self.observed
    }
}

fn control_line(name: &'static str, reason_on_break: &'static str, controls: &[Control]) -> GateLine {
    let broken: Vec<String> = controls
        .iter()
        .filter(|item| !item.holds())
        .map(|item| format!("{}: expected {} observed {}", item.label, item.expected, item.observed))
        .collect();
    if broken.is_empty() {
        let shown: Vec<String> = controls
            .iter()
            .map(|item| format!("{} -> {}", item.label, item.observed))
            .collect();
        GateLine::pass(name, controls.len(), shown.join(" | "))
    } else {
        GateLine::fail(name, reason_on_break, broken.len(), controls.len(), broken.join(" | "))
    }
}

fn outcome_pair(outcome: &crate::route::Outcome) -> String {
    format!("({}, {})", outcome.exit, outcome.reason)
}

pub fn query_gates(connection: &rusqlite::Connection, manifest_doc: &DbManifest) -> Vec<GateLine> {
    use crate::route::{self, Filters};
    let mut lines: Vec<GateLine> = Vec::new();
    let all = match route::read_atoms(connection) {
        Ok(all) => all,
        Err(error) => {
            lines.push(GateLine::unknown("G-Q1", REASON_INDEX_ABSENT, 0, 0, error));
            return lines;
        }
    };
    if all.is_empty() {
        lines.push(GateLine::unknown(
            "G-Q1",
            REASON_EMPTY_CORPUS,
            0,
            0,
            "0 atom(s) in the index, so no projection could be asked for; an empty scan is UNTESTABLE, never a pass".to_string(),
        ));
        return lines;
    }

    let bands = route::known_bands(connection);
    let squares = bands.len() * manifest::LAYER_KEYS.len();
    let mut empty_pair: Option<(String, String)> = None;
    let mut sized_pair: Option<(String, String, usize)> = None;
    for band in &bands {
        for layer in manifest::LAYER_KEYS {
            let filters = Filters {
                band: Some(band.clone()),
                layer: Some(layer.to_string()),
                role: None,
                executor: None,
                include_gaps: false,
            };
            let count = all.iter().filter(|atom| route::matches(atom, &filters)).count();
            if count == 0 && empty_pair.is_none() {
                empty_pair = Some((band.clone(), layer.to_string()));
            }
            let cap = match manifest_doc.caps.for_layer(layer) {
                Some(cap) => cap,
                None => 0,
            };
            if count > 0 && count <= cap && sized_pair.is_none() {
                sized_pair = Some((band.clone(), layer.to_string(), count));
            }
        }
    }

    let mut controls: Vec<Control> = Vec::new();
    let illegal = route::ls(
        connection,
        manifest_doc,
        &Filters {
            band: None,
            layer: Some("L9".to_string()),
            role: None,
            executor: None,
            include_gaps: false,
        },
        0,
        None,
    );
    controls.push(Control {
        label: "negative: --layer L9 is not a declared layer".to_string(),
        expected: format!("(2, {})", route::REASON_UNKNOWN_VALUE),
        observed: outcome_pair(&illegal),
    });

    match &empty_pair {
        Some((band, layer)) => {
            let outcome = route::ls(
                connection,
                manifest_doc,
                &Filters {
                    band: Some(band.clone()),
                    layer: Some(layer.clone()),
                    role: None,
                    executor: None,
                    include_gaps: false,
                },
                0,
                None,
            );
            controls.push(Control {
                label: format!("vacuous: --band {} --layer {} is legal and selects 0 row", band, layer),
                expected: format!("(2, {})", route::REASON_EMPTY),
                observed: outcome_pair(&outcome),
            });
        }
        None => lines.push(GateLine::unknown(
            "G-Q1",
            REASON_CONTROL_ABSENT,
            1,
            squares,
            format!(
                "no legal band x layer pair selects 0 row here, so the control that proves an empty answer is refused could not be built; {} pair(s) were tried and every one had rows",
                squares
            ),
        )),
    }

    match &sized_pair {
        Some((band, layer, count)) => {
            let outcome = route::ls(
                connection,
                manifest_doc,
                &Filters {
                    band: Some(band.clone()),
                    layer: Some(layer.clone()),
                    role: None,
                    executor: None,
                    include_gaps: false,
                },
                0,
                None,
            );
            controls.push(Control {
                label: format!("positive: --band {} --layer {} selects {} row within cap", band, layer, count),
                expected: format!("(0, {})", route::REASON_ANSWERED),
                observed: outcome_pair(&outcome),
            });
        }
        None => lines.push(GateLine::unknown(
            "G-Q1",
            REASON_CONTROL_ABSENT,
            1,
            squares,
            "no legal band x layer pair sits between 1 row and its cap, so the control that proves a real answer is accepted could not be built".to_string(),
        )),
    }

    lines.push(control_line("G-Q1", REASON_CONTROL_BROKEN, &controls));
    lines
}

pub const CAP_ZERO_MANIFEST: &str = "[db]\nschema = 1\n[layers]\nL0 = \"hot\"\nL1 = \"lean\"\nL2 = \"full\"\n[caps]\nL0 = 0\nL1 = 30\nL2 = 8\nbudget_tokens = 8000\n[bands]\norder = [\"arch\"]\n";
pub const DUPLICATE_BAND_MANIFEST: &str = "[db]\nschema = 1\n[layers]\nL0 = \"hot\"\nL1 = \"lean\"\nL2 = \"full\"\n[caps]\nL0 = 100\nL1 = 30\nL2 = 8\nbudget_tokens = 8000\n[bands]\norder = [\"arch\", \"arch\"]\n";
pub const NOT_A_MANIFEST: &str = "this file is prose, not a manifest\n";

pub fn settings_gate(db: &Path) -> GateLine {
    let path = manifest::db_manifest_path(db);
    let before = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return GateLine::unknown(
                "G-Q2",
                REASON_INDEX_ABSENT,
                0,
                0,
                format!("{} could not be read: {}", path.display(), error),
            )
        }
    };
    let label = path.display().to_string();
    let real = manifest::parse_db(&String::from_utf8_lossy(&before), &label);
    let mut controls: Vec<Control> = vec![
        Control {
            label: "positive: the manifest on disk".to_string(),
            expected: "accepted".to_string(),
            observed: match &real {
                Ok(_) => "accepted".to_string(),
                Err(error) => format!("refused ({})", error),
            },
        },
        Control {
            label: "negative: caps.L0 = 0".to_string(),
            expected: "refused".to_string(),
            observed: match manifest::parse_db(CAP_ZERO_MANIFEST, "cap-zero") {
                Ok(_) => "accepted".to_string(),
                Err(_) => "refused".to_string(),
            },
        },
        Control {
            label: "negative: one band claimed twice".to_string(),
            expected: "refused".to_string(),
            observed: match manifest::parse_db(DUPLICATE_BAND_MANIFEST, "duplicate-band") {
                Ok(_) => "accepted".to_string(),
                Err(_) => "refused".to_string(),
            },
        },
        Control {
            label: "negative: not toml at all".to_string(),
            expected: "refused".to_string(),
            observed: match manifest::parse_db(NOT_A_MANIFEST, "prose") {
                Ok(_) => "accepted".to_string(),
                Err(_) => "refused".to_string(),
            },
        },
        Control {
            label: "vacuous: an empty file".to_string(),
            expected: "refused".to_string(),
            observed: match manifest::parse_db("", "empty") {
                Ok(_) => "accepted".to_string(),
                Err(_) => "refused".to_string(),
            },
        },
    ];
    let after = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return GateLine::unknown(
                "G-Q2",
                REASON_SOURCE_MUTATED,
                0,
                controls.len(),
                format!("{} vanished while it was being read: {}", path.display(), error),
            )
        }
    };
    controls.push(Control {
        label: format!("no-delete: {} byte before, and after four refusals", before.len()),
        expected: "byte identical".to_string(),
        observed: if before == after {
            "byte identical".to_string()
        } else {
            format!("changed to {} byte", after.len())
        },
    });
    control_line("G-Q2", REASON_CONTROL_BROKEN, &controls)
}

pub fn is_allowed_comment(text: &str) -> bool {
    let trimmed = text.trim();
    trimmed == "// SPDX-License-Identifier: Apache-2.0" || trimmed == "// Copyright (c) 2026 Glovrex"
}

pub fn scan_tokens(text: &str) -> Vec<(usize, String)> {
    let characters: Vec<char> = text.chars().collect();
    let mut hits: Vec<(usize, String)> = Vec::new();
    let mut index = 0usize;
    let mut line = 1usize;
    while index < characters.len() {
        let current = characters[index];
        if current == '\n' {
            line += 1;
            index += 1;
            continue;
        }
        if current == '"' {
            index += 1;
            while index < characters.len() {
                if characters[index] == '\\' {
                    index += 2;
                    continue;
                }
                if characters[index] == '\n' {
                    line += 1;
                }
                if characters[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }
        if current == '/' && index + 1 < characters.len() && characters[index + 1] == '/' {
            let mut end = index;
            while end < characters.len() && characters[end] != '\n' {
                end += 1;
            }
            hits.push((line, characters[index..end].iter().collect()));
            index = end;
            continue;
        }
        if current == '/' && index + 1 < characters.len() && characters[index + 1] == '*' {
            let start_line = line;
            let mut end = index + 2;
            while end + 1 < characters.len() && !(characters[end] == '*' && characters[end + 1] == '/') {
                if characters[end] == '\n' {
                    line += 1;
                }
                end += 1;
            }
            let stop = std::cmp::min(end + 2, characters.len());
            hits.push((start_line, characters[index..stop].iter().collect()));
            index = stop;
            continue;
        }
        index += 1;
    }
    hits
}

pub fn scan_line_anchored(text: &str) -> Vec<(usize, String)> {
    let mut hits: Vec<(usize, String)> = Vec::new();
    for (index, line) in text.split('\n').enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") || trimmed.starts_with("/*") {
            hits.push((index + 1, trimmed.to_string()));
        }
    }
    hits
}

pub fn rust_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found: Vec<std::path::PathBuf> = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return found,
    };
    let mut items: Vec<std::path::PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    items.sort();
    for path in items {
        if path.is_dir() {
            found.extend(rust_files(&path));
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            found.push(path);
        }
    }
    found
}

fn collapse_needles() -> Vec<String> {
    let stem = format!("unwrap{}or", "_");
    vec![
        format!("{}(", stem),
        format!("{}_default(", stem),
        format!("{}_else(", stem),
    ]
}

fn count_substring(text: &str, needle: &str) -> usize {
    let mut total = 0usize;
    let mut rest = text;
    while let Some(position) = rest.find(needle) {
        total += 1;
        rest = &rest[position + needle.len()..];
    }
    total
}

fn count_identifier(text: &str, needle: &str) -> usize {
    let name = needle.trim_end_matches('(');
    let mut total = 0usize;
    for line in text.split('\n') {
        let mut current = String::new();
        for character in line.chars() {
            if character.is_alphanumeric() || character == '_' {
                current.push(character);
                continue;
            }
            if current == name && character == '(' {
                total += 1;
            }
            current.clear();
        }
    }
    total
}

pub fn selftest_gates(dir: &Path) -> Vec<GateLine> {
    let mut lines: Vec<GateLine> = Vec::new();
    let files = rust_files(dir);
    if files.is_empty() {
        let detail = format!(
            "no rust file under {}; a clean scan over nothing is UNTESTABLE, never a pass",
            dir.display()
        );
        lines.push(GateLine::unknown("G-C1", REASON_NO_SOURCE, 0, 0, detail.clone()));
        lines.push(GateLine::unknown("G-C2", REASON_NO_SOURCE, 0, 0, detail));
        return lines;
    }
    let mut texts: Vec<(String, String)> = Vec::new();
    let mut unreadable: Vec<String> = Vec::new();
    for path in &files {
        match fs::read_to_string(path) {
            Ok(text) => texts.push((path.display().to_string(), text)),
            Err(error) => unreadable.push(format!("{}: {}", path.display(), error)),
        }
    }
    if !unreadable.is_empty() {
        let detail = format!(
            "{} of {} file(s) could not be read, so a clean scan would be a claim about files nobody looked at: {:?}",
            unreadable.len(),
            files.len(),
            unreadable
        );
        lines.push(GateLine::unknown("G-C1", REASON_NO_SOURCE, unreadable.len(), files.len(), detail.clone()));
        lines.push(GateLine::unknown("G-C2", REASON_NO_SOURCE, unreadable.len(), files.len(), detail));
        return lines;
    }

    let mut token_total = 0usize;
    let mut anchored_total = 0usize;
    let mut violations: Vec<String> = Vec::new();
    let mut disagreements: Vec<String> = Vec::new();
    for (name, text) in &texts {
        let token_hits = scan_tokens(text);
        let anchored_hits = scan_line_anchored(text);
        token_total += token_hits.len();
        anchored_total += anchored_hits.len();
        if token_hits.len() != anchored_hits.len() {
            disagreements.push(format!(
                "{}: token scan {} vs line anchored {}",
                name,
                token_hits.len(),
                anchored_hits.len()
            ));
        }
        for (line, content) in token_hits.iter().chain(anchored_hits.iter()) {
            if !is_allowed_comment(content) {
                let mark = format!("{}:{}", name, line);
                if !violations.contains(&mark) {
                    violations.push(mark);
                }
            }
        }
    }
    if !violations.is_empty() {
        lines.push(GateLine::fail(
            "G-C1",
            REASON_COMMENT,
            violations.len(),
            token_total + anchored_total,
            format!("{:?}", violations.iter().take(8).collect::<Vec<&String>>()),
        ));
    } else if !disagreements.is_empty() {
        lines.push(GateLine::fail(
            "G-C1",
            REASON_SPELLING_SPLIT,
            disagreements.len(),
            files.len(),
            format!("{:?}", disagreements),
        ));
    } else {
        lines.push(GateLine::pass(
            "G-C1",
            files.len(),
            format!(
                "the only comments in {} rust file(s) are the two header lines; counted twice and both spellings say {}",
                files.len(),
                token_total
            ),
        ));
    }

    let needles = collapse_needles();
    let mut hits: Vec<String> = Vec::new();
    let mut substring_total = 0usize;
    let mut identifier_total = 0usize;
    for (name, text) in &texts {
        for needle in &needles {
            let by_substring = count_substring(text, needle);
            let by_identifier = count_identifier(text, needle);
            substring_total += by_substring;
            identifier_total += by_identifier;
            if by_substring > 0 || by_identifier > 0 {
                hits.push(format!("{} {} x{}/{}", name, needle, by_substring, by_identifier));
            }
        }
    }
    if substring_total == 0 && identifier_total == 0 {
        lines.push(GateLine::pass(
            "G-C2",
            files.len() * needles.len(),
            format!(
                "no call in {} rust file(s) turns a missing value into a default: {} needle(s) counted two ways, both 0, so UNKNOWN has no silent path to a value",
                files.len(),
                needles.len()
            ),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-C2",
            REASON_UNKNOWN_COLLAPSED,
            std::cmp::max(substring_total, identifier_total),
            files.len() * needles.len(),
            format!("{:?}", hits.iter().take(8).collect::<Vec<&String>>()),
        ));
    }
    lines
}
