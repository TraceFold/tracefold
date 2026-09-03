// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::atom::{first_line, short_id, LAYER_L0, LAYER_L1, LAYER_L2};
use crate::gate::{GateLine, REASON_COMMUTE_BREAK, REASON_EMPTY_CORPUS, REASON_LOD_NOT_MONOTONE};
use crate::manifest::{Caps, DbManifest, EXECUTORS, LAYER_KEYS, ROLES};
use std::collections::BTreeMap;

pub const REASON_EMPTY: &str = "EMPTY";
pub const REASON_UNTESTABLE: &str = "UNTESTABLE";
pub const REASON_UNKNOWN_VALUE: &str = "UNKNOWN_FILTER_VALUE";
pub const REASON_UNKNOWN_LOD: &str = "UNKNOWN_LOD";
pub const REASON_OVER_CAP: &str = "OVER_CAP";
pub const REASON_OVER_BUDGET: &str = "OVER_BUDGET";
pub const REASON_BAD_CURSOR: &str = "BAD_CURSOR";
pub const REASON_AMBIGUOUS: &str = "AMBIGUOUS_ADDRESS";
pub const REASON_ANSWERED: &str = "ANSWERED";

pub struct Page {
    pub command: &'static str,
    pub rows: Vec<IndexAtom>,
    pub scores: Vec<f64>,
    pub matched: usize,
    pub unreadable: usize,
    pub cap: usize,
    pub budget_tokens: usize,
    pub lod: usize,
    pub band: Option<String>,
    pub layer: Option<String>,
    pub cursor: Option<String>,
}

impl Page {
    pub fn empty(command: &'static str) -> Self {
        Page {
            command,
            rows: Vec::new(),
            scores: Vec::new(),
            matched: 0,
            unreadable: 0,
            cap: 0,
            budget_tokens: 0,
            lod: 0,
            band: None,
            layer: None,
            cursor: None,
        }
    }
}

pub struct Outcome {
    pub exit: i32,
    pub reason: &'static str,
    pub text: String,
    pub page: Page,
}

impl Outcome {
    pub fn refused(reason: &'static str, text: String) -> Self {
        Outcome {
            exit: 2,
            reason,
            text,
            page: Page::empty("unknown"),
        }
    }
    pub fn refused_with(reason: &'static str, text: String, page: Page) -> Self {
        Outcome {
            exit: 2,
            reason,
            text,
            page,
        }
    }
    pub fn answered(text: String, page: Page) -> Self {
        Outcome {
            exit: 0,
            reason: REASON_ANSWERED,
            text,
            page,
        }
    }
    pub fn verdict(&self) -> &'static str {
        match self.reason {
            REASON_ANSWERED => "TRUE",
            REASON_EMPTY => "FALSE",
            _ => "UNKNOWN",
        }
    }
}

fn provenance_json(atom: &IndexAtom) -> serde_json::Value {
    serde_json::json!({
        "path": format!("bands/{}/{}", atom.band, atom.path),
        "anchor": atom.anchor,
        "start_line": atom.line_start,
        "end_line": atom.line_end,
        "byte_start": atom.byte_start,
        "byte_end": atom.byte_end,
    })
}

pub fn row_json(atom: &IndexAtom, lod: usize, score: Option<f64>) -> serde_json::Value {
    let mut row = serde_json::json!({
        "id": atom.id,
        "band": atom.band,
        "document": format!("{}/{}", atom.band, atom.path),
        "layer": atom.layer,
        "kind": atom.kind,
        "role": atom.role,
        "executor": atom.executor,
        "evidence": atom.evidence,
        "line": first_line(&atom.content),
    });
    if let Some(found) = score {
        row["score"] = serde_json::json!(found);
    }
    if lod >= 1 {
        row["content"] = serde_json::json!(atom.content);
    }
    if lod >= 2 {
        row["provenance"] = provenance_json(atom);
        let mut relations: Vec<serde_json::Value> = Vec::new();
        if let Some(parent) = &atom.parent {
            relations.push(serde_json::json!({ "type": "parent", "dst": parent }));
        }
        row["relations"] = serde_json::json!(relations);
    }
    row
}

pub fn wire(outcome: &Outcome) -> String {
    let page = &outcome.page;
    let rows: Vec<serde_json::Value> = page
        .rows
        .iter()
        .enumerate()
        .map(|(index, atom)| {
            let score = match page.scores.get(index) {
                Some(found) => Some(*found),
                None => None,
            };
            row_json(atom, page.lod, score)
        })
        .collect();
    let body = serde_json::json!({
        "schema": 1,
        "verdict": outcome.verdict(),
        "reason": outcome.reason,
        "exit": outcome.exit,
        "query": {
            "cmd": page.command,
            "band": page.band,
            "layer": page.layer,
            "lod": page.lod,
            "cursor": page.cursor,
        },
        "cap": {
            "rows": page.cap,
            "budget_tokens": page.budget_tokens,
            "bytes_returned": outcome.text.len(),
        },
        "denominator": {
            "matched": page.matched,
            "returned": page.rows.len(),
            "unscanned": page.unreadable,
        },
        "rows": rows,
        "note": outcome.text.trim_end().to_string(),
    });
    match serde_json::to_string_pretty(&body) {
        Ok(text) => text,
        Err(error) => format!(
            "{{\"schema\":1,\"verdict\":\"UNKNOWN\",\"reason\":\"WIRE_NOT_SERIALISED\",\"exit\":2,\"note\":{:?}}}",
            error.to_string()
        ),
    }
}

#[derive(Clone)]
pub struct IndexAtom {
    pub id: String,
    pub band: String,
    pub path: String,
    pub anchor: String,
    pub layer: String,
    pub kind: String,
    pub role: String,
    pub executor: String,
    pub evidence: String,
    pub ordinal: i64,
    pub line_start: i64,
    pub line_end: i64,
    pub byte_start: i64,
    pub byte_end: i64,
    pub parent: Option<String>,
    pub content: String,
}

pub struct Filters {
    pub band: Option<String>,
    pub layer: Option<String>,
    pub role: Option<String>,
    pub executor: Option<String>,
}

pub fn read_atoms(connection: &rusqlite::Connection) -> Result<Vec<IndexAtom>, String> {
    let sql = "SELECT a.id, d.band_id, d.path, a.anchor, a.layer, a.kind, d.role, a.executor, a.evidence, a.ordinal, a.line_start, a.line_end, a.byte_start, a.byte_end, a.parent_id, t.content FROM atoms a JOIN documents d ON d.id = a.document_id JOIN atom_text t ON t.atom_id = a.id ORDER BY d.ord, a.line_start, a.ordinal";
    let mut statement = match connection.prepare(sql) {
        Ok(statement) => statement,
        Err(error) => return Err(error.to_string()),
    };
    let rows = statement.query_map([], |row| {
        Ok(IndexAtom {
            id: row.get(0)?,
            band: row.get(1)?,
            path: row.get(2)?,
            anchor: row.get(3)?,
            layer: row.get(4)?,
            kind: row.get(5)?,
            role: row.get(6)?,
            executor: row.get(7)?,
            evidence: row.get(8)?,
            ordinal: row.get(9)?,
            line_start: row.get(10)?,
            line_end: row.get(11)?,
            byte_start: row.get(12)?,
            byte_end: row.get(13)?,
            parent: row.get(14)?,
            content: row.get(15)?,
        })
    });
    let mut out: Vec<IndexAtom> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(atom) => out.push(atom),
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(out)
}

pub fn known_bands(connection: &rusqlite::Connection) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut statement = match connection.prepare("SELECT id FROM bands ORDER BY ord") {
        Ok(statement) => statement,
        Err(_) => return out,
    };
    let rows = statement.query_map([], |row| row.get::<usize, String>(0));
    if let Ok(found) = rows {
        for row in found.flatten() {
            out.push(row);
        }
    }
    out
}

fn reject_value(name: &str, value: &str, allowed: &[&str]) -> Outcome {
    Outcome::refused(
        REASON_UNKNOWN_VALUE,
        format!(
            "--{} \"{}\" is not one of {}; an unknown value would select nothing and print an empty page as if it were an answer\n",
            name,
            value,
            allowed.join(" ")
        ),
    )
}

pub fn validate_filters(
    connection: &rusqlite::Connection,
    filters: &Filters,
) -> Option<Outcome> {
    if let Some(band) = &filters.band {
        let bands = known_bands(connection);
        if !bands.contains(band) {
            let names: Vec<&str> = bands.iter().map(|item| item.as_str()).collect();
            return Some(reject_value("band", band, &names));
        }
    }
    if let Some(layer) = &filters.layer {
        if !LAYER_KEYS.contains(&layer.as_str()) {
            return Some(reject_value("layer", layer, &LAYER_KEYS));
        }
    }
    if let Some(role) = &filters.role {
        if !ROLES.contains(&role.as_str()) {
            return Some(reject_value("role", role, &ROLES));
        }
    }
    if let Some(executor) = &filters.executor {
        let mut allowed: Vec<&str> = EXECUTORS.to_vec();
        allowed.push(crate::atom::UNKNOWN);
        if !allowed.contains(&executor.as_str()) {
            return Some(reject_value("executor", executor, &allowed));
        }
    }
    None
}

pub fn matches(atom: &IndexAtom, filters: &Filters) -> bool {
    if let Some(band) = &filters.band {
        if atom.band != *band {
            return false;
        }
    }
    if let Some(layer) = &filters.layer {
        if atom.layer != *layer {
            return false;
        }
    }
    if let Some(role) = &filters.role {
        if atom.role != *role {
            return false;
        }
    }
    if let Some(executor) = &filters.executor {
        if atom.executor != *executor {
            return false;
        }
    }
    true
}

pub fn render_atom(atom: &IndexAtom, lod: usize) -> String {
    let headline = format!(
        "{}  {}  {}/{}#{}  {}",
        short_id(&atom.id),
        atom.layer,
        atom.band,
        atom.path,
        atom.anchor,
        first_line(&atom.content)
    );
    if lod == 0 {
        return headline;
    }
    let mut out = headline;
    out.push('\n');
    out.push_str(atom.content.trim_end_matches('\n'));
    if lod == 1 {
        return out;
    }
    out.push('\n');
    out.push_str(&format!(
        "  provenance: {}/{} anchor={} line {}..{} byte {}..{} kind={} executor={} evidence={} ordinal={}",
        atom.band,
        atom.path,
        atom.anchor,
        atom.line_start,
        atom.line_end,
        atom.byte_start,
        atom.byte_end,
        atom.kind,
        atom.executor,
        atom.evidence,
        atom.ordinal
    ));
    match &atom.parent {
        Some(parent) => out.push_str(&format!("\n  relations: parent -> {}", short_id(parent))),
        None => out.push_str("\n  relations: parent -> none (this atom sits directly under its document)"),
    }
    out
}

pub fn render_rows(rows: &[IndexAtom], lod: usize) -> Vec<(String, String)> {
    rows.iter()
        .map(|atom| (atom.layer.clone(), render_atom(atom, lod)))
        .collect()
}

fn layer_histogram(rows: &[IndexAtom]) -> BTreeMap<String, usize> {
    let mut out: BTreeMap<String, usize> = BTreeMap::new();
    for atom in rows {
        *out.entry(atom.layer.clone()).or_insert(0) += 1;
    }
    out
}

pub fn cap_for(rows: &[IndexAtom], filters: &Filters, caps: &Caps) -> (usize, String) {
    if let Some(layer) = &filters.layer {
        if let Some(cap) = caps.for_layer(layer) {
            return (cap, format!("caps.{} (you declared --layer {})", layer, layer));
        }
    }
    let mut lowest = caps.l0;
    let mut which = LAYER_L0.to_string();
    for layer in layer_histogram(rows).keys() {
        if let Some(cap) = caps.for_layer(layer) {
            if cap < lowest {
                lowest = cap;
                which = layer.clone();
            }
        }
    }
    (
        lowest,
        format!(
            "caps.{} (no --layer was declared, so the strictest cap among the layers present applies)",
            which
        ),
    )
}

fn narrowing_hint(rows: &[IndexAtom]) -> String {
    let mut bands: BTreeMap<&str, usize> = BTreeMap::new();
    let mut roles: BTreeMap<&str, usize> = BTreeMap::new();
    for atom in rows {
        *bands.entry(atom.band.as_str()).or_insert(0) += 1;
        *roles.entry(atom.role.as_str()).or_insert(0) += 1;
    }
    let layers = layer_histogram(rows);
    let mut out = String::new();
    out.push_str(&format!("  narrow by --layer: {:?}\n", layers));
    out.push_str(&format!("  narrow by --band: {:?}\n", bands));
    out.push_str(&format!("  narrow by --role: {:?}\n", roles));
    out.push_str("  or page with --cursor <the exact id printed on the previous page>\n");
    out
}

pub fn ls(
    connection: &rusqlite::Connection,
    manifest: &DbManifest,
    filters: &Filters,
    lod: usize,
    cursor: Option<&str>,
) -> Outcome {
    if lod > 2 {
        return Outcome::refused(
            REASON_UNKNOWN_LOD,
            format!("--lod {} is not one of 0 1 2\n", lod),
        );
    }
    if let Some(refusal) = validate_filters(connection, filters) {
        return refusal;
    }
    let all = match read_atoms(connection) {
        Ok(all) => all,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if all.is_empty() {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s), so this question could not be asked. Run db compile\n".to_string(),
        );
    }
    let rows: Vec<IndexAtom> = all.iter().filter(|atom| matches(atom, filters)).cloned().collect();
    let describe = |cap: usize, page: Vec<IndexAtom>, matched: usize| Page {
        command: "ls",
        rows: page,
        scores: Vec::new(),
        matched,
        unreadable: 0,
        cap,
        budget_tokens: manifest.caps.budget_tokens,
        lod,
        band: filters.band.clone(),
        layer: filters.layer.clone(),
        cursor: cursor.map(|value| value.to_string()),
    };
    if rows.is_empty() {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: 0 of {} atom(s) match this projection. The corpus is there and the filters are legal, so this is an empty answer, not an unanswerable question; it is still not a pass\n",
                all.len()
            ),
            describe(0, Vec::new(), 0),
        );
    }
    let (cap, cap_source) = cap_for(&rows, filters, &manifest.caps);
    let total = rows.len();
    let mut header = String::new();
    let page: Vec<&IndexAtom>;
    let mut start = 0usize;
    match cursor {
        Some(key) => {
            if key != "begin" {
                match rows.iter().position(|atom| atom.id == key) {
                    Some(position) => start = position + 1,
                    None => {
                        return Outcome::refused(
                            REASON_BAD_CURSOR,
                            format!(
                                "cursor \"{}\" is not the id of a row in this projection ({} row(s)); the cursor is the exact id printed on the previous page, never a prefix of it\n",
                                key, total
                            ),
                        )
                    }
                }
            }
            page = rows.iter().skip(start).take(cap).collect();
            header.push_str(&format!(
                "rows {}..{} of {} (cap {} from {}), lod {}\n",
                start + 1,
                start + page.len(),
                total,
                cap,
                cap_source,
                lod
            ));
        }
        None => {
            if total > cap {
                let mut text = format!(
                    "{} row(s) exceed cap {} from {}; nothing is silently truncated\n",
                    total, cap, cap_source
                );
                text.push_str(&narrowing_hint(&rows));
                return Outcome::refused(REASON_OVER_CAP, text);
            }
            page = rows.iter().collect();
            header.push_str(&format!(
                "rows {} of {} (cap {} from {}), lod {}\n",
                page.len(),
                total,
                cap,
                cap_source,
                lod
            ));
        }
    }
    let mut body = String::new();
    for atom in &page {
        body.push_str(&render_atom(atom, lod));
        body.push('\n');
    }
    let budget = manifest.caps.budget_tokens * 4;
    if body.len() > budget {
        let mut text = format!(
            "the projection is {} byte, over the budget of {} byte ({} token); nothing is silently truncated\n",
            body.len(),
            budget,
            manifest.caps.budget_tokens
        );
        text.push_str(&narrowing_hint(&rows));
        return Outcome::refused(REASON_OVER_BUDGET, text);
    }
    let mut text = header;
    text.push_str(&body);
    if start + page.len() < total {
        if let Some(last) = page.last() {
            text.push_str(&format!("next: db ls --cursor {}\n", last.id));
        }
    }
    let carried: Vec<IndexAtom> = page.iter().map(|atom| (*atom).clone()).collect();
    Outcome::answered(text, describe(cap, carried, total))
}

pub fn show(connection: &rusqlite::Connection, wanted: &str, lod: usize) -> Outcome {
    if lod > 2 {
        return Outcome::refused(
            REASON_UNKNOWN_LOD,
            format!("--lod {} is not one of 0 1 2\n", lod),
        );
    }
    let all = match read_atoms(connection) {
        Ok(all) => all,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if all.is_empty() {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s). Run db compile\n".to_string(),
        );
    }
    let found: Vec<&IndexAtom> = all
        .iter()
        .filter(|atom| {
            atom.id == wanted
                || atom.anchor == wanted
                || format!("{}/{}#{}", atom.band, atom.path, atom.anchor) == wanted
                || format!("{}#{}", atom.path, atom.anchor) == wanted
        })
        .collect();
    if found.is_empty() {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: no atom has the id or address \"{}\" among {} atom(s). show takes a full id or an exact address, never a prefix\n",
                wanted,
                all.len()
            ),
            Page {
                command: "show",
                lod,
                ..Page::empty("show")
            },
        );
    }
    if found.len() > 1 {
        let mut text = format!(
            "\"{}\" names {} atoms; an address that selects more than one is ambiguous and is refused rather than answered with the first\n",
            wanted,
            found.len()
        );
        for atom in found.iter().take(10) {
            text.push_str(&format!(
                "  {}  {}/{}#{}\n",
                short_id(&atom.id),
                atom.band,
                atom.path,
                atom.anchor
            ));
        }
        return Outcome::refused(REASON_AMBIGUOUS, text);
    }
    match found.first() {
        Some(atom) => Outcome::answered(
            format!("{}\n", render_atom(atom, lod)),
            Page {
                command: "show",
                rows: vec![(*atom).clone()],
                matched: 1,
                cap: 1,
                lod,
                band: Some(atom.band.clone()),
                layer: Some(atom.layer.clone()),
                ..Page::empty("show")
            },
        ),
        None => Outcome::refused(REASON_UNTESTABLE, "no atom\n".to_string()),
    }
}

pub fn escape_fts(needle: &str) -> String {
    format!("\"{}\"", needle.replace('"', "\"\""))
}

pub fn find(
    connection: &rusqlite::Connection,
    manifest: &DbManifest,
    needle: &str,
    filters: &Filters,
    limit: usize,
) -> Outcome {
    if needle.trim().is_empty() {
        return Outcome::refused(
            REASON_UNKNOWN_VALUE,
            "an empty needle asks nothing; db find needs a string to look for\n".to_string(),
        );
    }
    if let Some(refusal) = validate_filters(connection, filters) {
        return refusal;
    }
    if limit < 1 {
        return Outcome::refused(
            REASON_UNKNOWN_VALUE,
            "--limit below 1 can never return a row\n".to_string(),
        );
    }
    let all = match read_atoms(connection) {
        Ok(all) => all,
        Err(error) => return Outcome::refused(REASON_UNTESTABLE, format!("{}\n", error)),
    };
    if all.is_empty() {
        return Outcome::refused(
            REASON_UNTESTABLE,
            "UNTESTABLE: the index holds 0 atom(s), so the search could not run. Run db compile\n".to_string(),
        );
    }
    let cap = match &filters.layer {
        Some(layer) => match manifest.caps.for_layer(layer) {
            Some(cap) => cap,
            None => manifest.caps.l0,
        },
        None => manifest.caps.l0,
    };
    if limit > cap {
        return Outcome::refused(
            REASON_OVER_CAP,
            format!("--limit {} is over the cap {} that applies to this layer\n", limit, cap),
        );
    }
    let mut sql = String::from(
        "SELECT atom_id, bm25(atoms_fts) FROM atoms_fts WHERE atoms_fts MATCH ?1",
    );
    if filters.band.is_some() {
        sql.push_str(" AND band = ?2");
    }
    if filters.layer.is_some() {
        sql.push_str(if filters.band.is_some() { " AND layer = ?3" } else { " AND layer = ?2" });
    }
    sql.push_str(" ORDER BY bm25(atoms_fts) ASC");
    let query = escape_fts(needle);
    let mut binds: Vec<String> = vec![query];
    if let Some(band) = &filters.band {
        binds.push(band.clone());
    }
    if let Some(layer) = &filters.layer {
        binds.push(layer.clone());
    }
    let mut statement = match connection.prepare(&sql) {
        Ok(statement) => statement,
        Err(error) => {
            return Outcome::refused(
                REASON_UNTESTABLE,
                format!("the search could not be prepared: {}\n", error),
            )
        }
    };
    let parameters: Vec<&dyn rusqlite::ToSql> =
        binds.iter().map(|item| item as &dyn rusqlite::ToSql).collect();
    let rows = statement.query_map(parameters.as_slice(), |row| {
        Ok((row.get::<usize, String>(0)?, row.get::<usize, f64>(1)?))
    });
    let mut hits: Vec<(String, f64)> = Vec::new();
    match rows {
        Ok(found) => {
            for row in found {
                match row {
                    Ok(hit) => hits.push(hit),
                    Err(error) => {
                        return Outcome::refused(
                            REASON_UNTESTABLE,
                            format!("the search failed while reading rows: {}\n", error),
                        )
                    }
                }
            }
        }
        Err(error) => {
            return Outcome::refused(
                REASON_UNTESTABLE,
                format!("the search failed: {}\n", error),
            )
        }
    }
    let filtered: Vec<(String, f64)> = hits
        .into_iter()
        .filter(|(id, _)| match all.iter().find(|atom| atom.id == *id) {
            Some(atom) => matches(atom, filters),
            None => false,
        })
        .collect();
    if filtered.is_empty() {
        return Outcome::refused_with(
            REASON_EMPTY,
            format!(
                "EMPTY: \"{}\" matched 0 of {} atom(s) in the full text index. The index is there and the query is legal, so this is an empty answer, not an unanswerable question\n",
                needle,
                all.len()
            ),
            Page {
                command: "find",
                cap,
                budget_tokens: manifest.caps.budget_tokens,
                band: filters.band.clone(),
                layer: filters.layer.clone(),
                ..Page::empty("find")
            },
        );
    }
    let total = filtered.len();
    let mut text = format!(
        "find \"{}\": {} hit(s), showing {} (bm25, lower is a better match; find returns the address and one line, never the body - use db show for that)\n",
        needle,
        total,
        std::cmp::min(total, limit)
    );
    let mut shown: Vec<IndexAtom> = Vec::new();
    let mut scores: Vec<f64> = Vec::new();
    for (id, score) in filtered.iter().take(limit) {
        if let Some(atom) = all.iter().find(|atom| atom.id == *id) {
            text.push_str(&format!("{:>9.3}  {}\n", score, render_atom(atom, 0)));
            shown.push(atom.clone());
            scores.push(*score);
        }
    }
    if total > limit {
        text.push_str(&format!(
            "{} further hit(s) are not shown; raise --limit (cap {}) to see them\n",
            total - limit,
            cap
        ));
    }
    Outcome::answered(
        text,
        Page {
            command: "find",
            rows: shown,
            scores,
            matched: total,
            cap,
            budget_tokens: manifest.caps.budget_tokens,
            band: filters.band.clone(),
            layer: filters.layer.clone(),
            ..Page::empty("find")
        },
    )
}

pub fn commute_gate(connection: &rusqlite::Connection) -> Vec<GateLine> {
    let mut lines: Vec<GateLine> = Vec::new();
    let all = match read_atoms(connection) {
        Ok(all) => all,
        Err(error) => {
            lines.push(GateLine::unknown("G-Q3", REASON_EMPTY_CORPUS, 0, 0, error));
            return lines;
        }
    };
    if all.is_empty() {
        lines.push(GateLine::unknown(
            "G-Q3",
            REASON_EMPTY_CORPUS,
            0,
            0,
            "0 atom(s) to project; a commuting square over nothing is UNTESTABLE, never a pass".to_string(),
        ));
        return lines;
    }
    let mut checks = 0usize;
    let mut breaks: Vec<String> = Vec::new();
    for layer in [LAYER_L0, LAYER_L1, LAYER_L2] {
        for lod in 0..3usize {
            let filters = Filters {
                band: None,
                layer: Some(layer.to_string()),
                role: None,
                executor: None,
            };
            let filtered: Vec<IndexAtom> =
                all.iter().filter(|atom| matches(atom, &filters)).cloned().collect();
            let render_then: Vec<String> = render_rows(&filtered, lod)
                .into_iter()
                .map(|(_, text)| text)
                .collect();
            let filter_then: Vec<String> = render_rows(&all, lod)
                .into_iter()
                .filter(|(found, _)| found == layer)
                .map(|(_, text)| text)
                .collect();
            checks += 1;
            if render_then != filter_then {
                breaks.push(format!(
                    "layer {} lod {}: render(filter) has {} row(s), filter(render) has {} row(s)",
                    layer,
                    lod,
                    render_then.len(),
                    filter_then.len()
                ));
            }
        }
    }
    if breaks.is_empty() {
        lines.push(GateLine::pass(
            "G-Q3",
            checks,
            format!(
                "layer and LOD commute over {} square(s): filtering by meaning then rendering gives the same rows as rendering then filtering, which is what keeps the cap a function of layer alone",
                checks
            ),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-Q3",
            REASON_COMMUTE_BREAK,
            breaks.len(),
            checks,
            breaks.join(" | "),
        ));
    }

    let mut monotone_breaks: Vec<String> = Vec::new();
    let sample: Vec<&IndexAtom> = all.iter().take(64).collect();
    for atom in &sample {
        let zero = render_atom(atom, 0);
        let one = render_atom(atom, 1);
        let two = render_atom(atom, 2);
        if !one.contains(&zero) || !two.contains(&one) {
            monotone_breaks.push(format!("{}/{}#{}", atom.band, atom.path, atom.anchor));
        }
    }
    if monotone_breaks.is_empty() {
        lines.push(GateLine::pass(
            "G-Q3b",
            sample.len(),
            format!(
                "LOD is monotone over {} sampled atom(s): everything a shallower level says, a deeper level says too",
                sample.len()
            ),
        ));
    } else {
        lines.push(GateLine::fail(
            "G-Q3b",
            REASON_LOD_NOT_MONOTONE,
            monotone_breaks.len(),
            sample.len(),
            format!("{:?}", monotone_breaks.iter().take(5).collect::<Vec<&String>>()),
        ));
    }
    lines
}
