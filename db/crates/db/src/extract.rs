// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use crate::atom::{
    atom_id, Range, SemanticAtom, SourceRef, StoredAtom, EVIDENCE_DERIVED, EVIDENCE_MEASURED,
    EVIDENCE_OWNER_SAID, KIND_CODE, KIND_DECISION, KIND_GAP, KIND_HEADING, KIND_LIST, KIND_PARA,
    KIND_TABLE_ROW, LAYER_L0, UNKNOWN,
};
use crate::manifest::{self, BandManifest, DocumentDecl};
use std::fs;
use std::path::Path;

pub const ANCHOR_CHARS: usize = 48;
pub const EVIDENCE_MARKERS: [(&str, &str); 4] = [
    ("[MEASURED", EVIDENCE_MEASURED),
    ("[DERIVED", EVIDENCE_DERIVED),
    ("[OWNER-SAID", EVIDENCE_OWNER_SAID),
    ("[OWNER_SAID", EVIDENCE_OWNER_SAID),
];

pub struct DocumentIr {
    pub document_id: String,
    pub band: String,
    pub path: String,
    pub role: String,
    pub executor: String,
    pub atoms: Vec<StoredAtom>,
    pub source_bytes: usize,
}

pub fn read_document(path: &Path) -> Result<String, String> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => return Err(format!("unreadable: {}", error)),
    };
    match String::from_utf8(bytes) {
        Ok(text) => Ok(text),
        Err(error) => Err(format!(
            "invalid UTF-8 at byte {}; refused rather than admitted with replacement characters",
            error.utf8_error().valid_up_to()
        )),
    }
}

struct Line {
    start: usize,
    end: usize,
    body: String,
}

fn split_lines(text: &str) -> Vec<Line> {
    let bytes = text.as_bytes();
    let mut out: Vec<Line> = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\n' {
            let end = index + 1;
            out.push(Line {
                start,
                end,
                body: text[start..index].trim_end_matches('\r').to_string(),
            });
            start = end;
        }
        index += 1;
    }
    if start < bytes.len() {
        out.push(Line {
            start,
            end: bytes.len(),
            body: text[start..].trim_end_matches('\r').to_string(),
        });
    }
    out
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn fence_marker(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    for marker in ["```", "~~~"] {
        if trimmed.starts_with(marker) {
            return Some(marker.to_string());
        }
    }
    None
}

fn heading_level(line: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|value| *value == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest: String = trimmed.chars().skip(hashes).collect();
    if rest.starts_with(' ') || rest.starts_with('\t') {
        Some(hashes)
    } else {
        None
    }
}

fn is_table_row(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if trimmed.starts_with(marker) {
            return true;
        }
    }
    if trimmed == "-" || trimmed == "*" || trimmed == "+" {
        return true;
    }
    let digits: String = trimmed.chars().take_while(|value| value.is_ascii_digit()).collect();
    if digits.is_empty() {
        return false;
    }
    let rest: String = trimmed.chars().skip(digits.len()).collect();
    rest.starts_with(". ") || rest.starts_with(") ")
}

fn is_continuation(line: &str) -> bool {
    if is_blank(line) {
        return false;
    }
    if heading_level(line).is_some() || is_table_row(line) || is_list_item(line) {
        return false;
    }
    if fence_marker(line).is_some() {
        return false;
    }
    line.starts_with(' ') || line.starts_with('\t')
}

fn is_plain(line: &str) -> bool {
    !is_blank(line)
        && heading_level(line).is_none()
        && !is_table_row(line)
        && !is_list_item(line)
        && fence_marker(line).is_none()
}

pub fn heading_text(line: &str) -> String {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|value| *value == '#').count();
    trimmed.chars().skip(hashes).collect::<String>().trim().to_string()
}

pub fn heading_layer_tag(text: &str) -> Option<&'static str> {
    let trimmed = text.trim_end();
    for tag in ["{L0}", "{L1}", "{L2}"] {
        if trimmed.ends_with(tag) {
            return match tag {
                "{L0}" => Some("L0"),
                "{L1}" => Some("L1"),
                _ => Some("L2"),
            };
        }
    }
    None
}

pub fn is_decision_heading(text: &str) -> bool {
    let mut token = text.trim_start();
    if let Some(rest) = token.strip_prefix("**") {
        token = rest;
    }
    let rest = match token.strip_prefix("D-") {
        Some(rest) => rest,
        None => return false,
    };
    rest.chars().take(4).filter(|value| value.is_ascii_digit()).count() == 4
}

pub fn bracket_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for character in text.chars() {
        if character == '[' {
            depth += 1;
            if depth == 1 {
                current.clear();
                continue;
            }
        }
        if character == ']' && depth > 0 {
            depth -= 1;
            if depth == 0 {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() && !tags.contains(&trimmed) {
                    tags.push(trimmed);
                }
                current.clear();
                continue;
            }
        }
        if depth > 0 {
            current.push(character);
        }
    }
    tags
}

pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for character in text.chars() {
        if character.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(character);
        } else {
            pending_dash = true;
        }
        if out.chars().count() >= ANCHOR_CHARS {
            break;
        }
    }
    if out.is_empty() {
        return "section".to_string();
    }
    out
}

pub fn evidence_marks(content: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for (needle, verdict) in EVIDENCE_MARKERS {
        if content.contains(needle) && !out.contains(&verdict) {
            out.push(verdict);
        }
    }
    out
}

fn evidence_marker(content: &str) -> Option<&'static str> {
    let mut best: Option<(usize, &'static str)> = None;
    for (needle, verdict) in EVIDENCE_MARKERS {
        if let Some(position) = content.find(needle) {
            match best {
                Some((seen, _)) if seen <= position => {}
                _ => best = Some((position, verdict)),
            }
        }
    }
    match best {
        Some((_, verdict)) => Some(verdict),
        None => None,
    }
}

struct Block {
    kind: &'static str,
    first: usize,
    last: usize,
}

fn build_blocks(lines: &[Line]) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let body = lines[index].body.as_str();
        if is_blank(body) {
            let first = index;
            while index < lines.len() && is_blank(&lines[index].body) {
                index += 1;
            }
            blocks.push(Block {
                kind: KIND_GAP,
                first,
                last: index - 1,
            });
            continue;
        }
        if let Some(marker) = fence_marker(body) {
            let first = index;
            index += 1;
            while index < lines.len() {
                let closing = fence_marker(&lines[index].body);
                index += 1;
                if let Some(found) = closing {
                    if found == marker {
                        break;
                    }
                }
            }
            blocks.push(Block {
                kind: KIND_CODE,
                first,
                last: index - 1,
            });
            continue;
        }
        if heading_level(body).is_some() {
            let kind = if is_decision_heading(&heading_text(body)) {
                KIND_DECISION
            } else {
                KIND_HEADING
            };
            blocks.push(Block {
                kind,
                first: index,
                last: index,
            });
            index += 1;
            continue;
        }
        if is_table_row(body) {
            blocks.push(Block {
                kind: KIND_TABLE_ROW,
                first: index,
                last: index,
            });
            index += 1;
            continue;
        }
        if is_list_item(body) {
            let first = index;
            index += 1;
            while index < lines.len() && is_continuation(&lines[index].body) {
                index += 1;
            }
            blocks.push(Block {
                kind: KIND_LIST,
                first,
                last: index - 1,
            });
            continue;
        }
        let first = index;
        index += 1;
        while index < lines.len() && is_plain(&lines[index].body) {
            index += 1;
        }
        blocks.push(Block {
            kind: KIND_PARA,
            first,
            last: index - 1,
        });
    }
    blocks
}

pub fn extract_document(
    band: &BandManifest,
    document: &DocumentDecl,
    text: &str,
) -> Result<DocumentIr, String> {
    let document_id = format!("{}/{}", band.id, document.path);
    let executor = manifest::document_executor(band, document);
    let role_layer = manifest::layer_for_role(&document.role);
    let role_evidence = manifest::evidence_for_role(&document.role);
    let lines = split_lines(text);
    let blocks = build_blocks(&lines);

    let mut atoms: Vec<StoredAtom> = Vec::new();
    let mut heading_stack: Vec<(usize, usize)> = Vec::new();
    let mut layer_stack: Vec<(usize, &'static str)> = Vec::new();
    let mut anchors: Vec<String> = Vec::new();
    let mut child_count: Vec<u32> = Vec::new();
    let mut root_children = 0u32;

    for block in &blocks {
        let start = lines[block.first].start;
        let end = lines[block.last].end;
        let content = text[start..end].to_string();
        let body = lines[block.first].body.as_str();
        let is_heading = block.kind == KIND_HEADING || block.kind == KIND_DECISION;
        let level = match heading_level(body) {
            Some(level) => level,
            None => 0,
        };

        if is_heading {
            while let Some((top, _)) = heading_stack.last() {
                if *top >= level {
                    heading_stack.pop();
                } else {
                    break;
                }
            }
            while let Some((top, _)) = layer_stack.last() {
                if *top >= level {
                    layer_stack.pop();
                } else {
                    break;
                }
            }
        }

        let parent_index = match heading_stack.last() {
            Some((_, index)) => Some(*index),
            None => None,
        };
        let parent = match parent_index {
            Some(index) => Some(atoms[index].semantic.id.clone()),
            None => None,
        };
        let order = match parent_index {
            Some(index) => {
                child_count[index] += 1;
                child_count[index]
            }
            None => {
                root_children += 1;
                root_children
            }
        };

        let title = if is_heading { heading_text(body) } else { String::new() };
        let own_tag = if is_heading { heading_layer_tag(&title) } else { None };
        let title_clean = match own_tag {
            Some(_) => {
                let trimmed = title.trim_end();
                let cut = trimmed.len().saturating_sub(4);
                trimmed[..cut].trim_end().to_string()
            }
            None => title.clone(),
        };
        if let Some(tag) = own_tag {
            layer_stack.push((level, tag));
        }
        let layer = match own_tag {
            Some(tag) => tag.to_string(),
            None => match layer_stack.last() {
                Some((_, tag)) => tag.to_string(),
                None => {
                    if block.kind == KIND_HEADING {
                        LAYER_L0.to_string()
                    } else {
                        match role_layer {
                            Some(found) => found.to_string(),
                            None => UNKNOWN.to_string(),
                        }
                    }
                }
            },
        };

        let evidence = if block.kind == KIND_GAP {
            UNKNOWN.to_string()
        } else {
            match evidence_marker(&content) {
                Some(found) => found.to_string(),
                None => match role_evidence {
                    Some(found) => found.to_string(),
                    None => UNKNOWN.to_string(),
                },
            }
        };

        let base_anchor = if is_heading {
            slug(&title_clean)
        } else {
            match parent_index {
                Some(index) => format!("{}.{}", atoms[index].semantic.source.anchor, order),
                None => format!("~.{}", order),
            }
        };
        let mut anchor = base_anchor.clone();
        let mut attempt = 2usize;
        while anchors.contains(&anchor) {
            anchor = format!("{}-{}", base_anchor, attempt);
            attempt += 1;
        }
        anchors.push(anchor.clone());

        let tags = if is_heading { bracket_tags(&title_clean) } else { Vec::new() };
        let id = atom_id(block.kind, &document.path, &anchor, &content);

        atoms.push(StoredAtom {
            semantic: SemanticAtom {
                id,
                kind: block.kind.to_string(),
                content,
                layer,
                parent,
                order,
                tags,
                source: SourceRef {
                    document: document_id.clone(),
                    path: document.path.clone(),
                    anchor,
                    range: Range {
                        start_line: block.first + 1,
                        end_line: block.last + 1,
                        byte_start: start,
                        byte_end: end,
                    },
                },
            },
            executor: executor.clone(),
            evidence,
            band: band.id.clone(),
        });
        child_count.push(0);
        if is_heading {
            heading_stack.push((level, atoms.len() - 1));
        }
    }

    Ok(DocumentIr {
        document_id,
        band: band.id.clone(),
        path: document.path.clone(),
        role: document.role.clone(),
        executor,
        atoms,
        source_bytes: text.len(),
    })
}
