// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex

use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

pub const DB_MANIFEST: &str = "db.toml";
pub const BAND_MANIFEST: &str = "band.toml";
pub const BANDS_DIR: &str = "bands";
pub const JOURNAL_DIR: &str = "journal";
pub const BUILD_DIR: &str = "build";

pub const ROLES: [&str; 6] = [
    "overview",
    "principles",
    "decisions",
    "evidence",
    "raw",
    "reference",
];
pub const EXECUTORS: [&str; 4] = ["owner", "cc", "lane", "external"];
pub const LAYER_KEYS: [&str; 3] = ["L0", "L1", "L2"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DbFile {
    db: DbSection,
    layers: LayerSection,
    caps: CapSection,
    bands: BandsSection,
    granularity: Option<GranularitySection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DbSection {
    schema: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LayerSection {
    #[serde(rename = "L0")]
    l0: String,
    #[serde(rename = "L1")]
    l1: String,
    #[serde(rename = "L2")]
    l2: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapSection {
    #[serde(rename = "L0")]
    l0: i64,
    #[serde(rename = "L1")]
    l1: i64,
    #[serde(rename = "L2")]
    l2: i64,
    budget_tokens: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandsSection {
    order: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GranularitySection {
    min_mean_bytes: i64,
    max_gap_ratio: f64,
}

#[derive(Clone, Debug)]
pub struct Granularity {
    pub min_mean_bytes: usize,
    pub max_gap_ratio: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandFile {
    band: BandSection,
    #[serde(default)]
    documents: Vec<DocumentDecl>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BandSection {
    id: String,
    title: String,
    #[serde(rename = "abstract")]
    summary: String,
    executor: Option<String>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct DocumentDecl {
    pub path: String,
    pub order: i64,
    pub role: String,
    pub executor: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Caps {
    pub l0: usize,
    pub l1: usize,
    pub l2: usize,
    pub budget_tokens: usize,
}

impl Caps {
    pub fn for_layer(&self, layer: &str) -> Option<usize> {
        match layer {
            "L0" => Some(self.l0),
            "L1" => Some(self.l1),
            "L2" => Some(self.l2),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DbManifest {
    pub schema: i64,
    pub layer_names: Vec<(String, String)>,
    pub caps: Caps,
    pub band_order: Vec<String>,
    pub granularity: Option<Granularity>,
}

#[derive(Clone, Debug)]
pub struct BandManifest {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub executor: Option<String>,
    pub documents: Vec<DocumentDecl>,
    pub dir: PathBuf,
    pub ord: usize,
}

pub struct BandLoad {
    pub id: String,
    pub outcome: Result<BandManifest, String>,
}

pub fn db_manifest_path(db: &Path) -> PathBuf {
    db.join(DB_MANIFEST)
}

pub fn bands_dir(db: &Path) -> PathBuf {
    db.join(BANDS_DIR)
}

pub fn band_dir(db: &Path, band: &str) -> PathBuf {
    bands_dir(db).join(band)
}

pub struct DbSearch {
    pub found: Option<PathBuf>,
    pub searched: Vec<PathBuf>,
}

pub fn search_db(explicit: Option<&str>) -> DbSearch {
    let mut searched: Vec<PathBuf> = Vec::new();
    if let Some(given) = explicit {
        let dir = PathBuf::from(given);
        let candidate = db_manifest_path(&dir);
        searched.push(candidate.clone());
        let found = if candidate.is_file() { Some(dir) } else { None };
        return DbSearch { found, searched };
    }
    if let Ok(from_env) = std::env::var("DB_DIR") {
        let dir = PathBuf::from(from_env);
        let candidate = db_manifest_path(&dir);
        searched.push(candidate.clone());
        if candidate.is_file() {
            return DbSearch { found: Some(dir), searched };
        }
    }
    let mut here = match std::env::current_dir() {
        Ok(dir) => dir,
        Err(_) => return DbSearch { found: None, searched },
    };
    loop {
        let direct = db_manifest_path(&here);
        searched.push(direct.clone());
        if direct.is_file() {
            return DbSearch { found: Some(here), searched };
        }
        let sibling = here.join("DB");
        let sibling_manifest = db_manifest_path(&sibling);
        searched.push(sibling_manifest.clone());
        if sibling_manifest.is_file() {
            return DbSearch { found: Some(sibling), searched };
        }
        if !here.pop() {
            return DbSearch { found: None, searched };
        }
    }
}

pub const DECISIONS_BAND: &str = "decisions";
pub const DECISIONS_DOCUMENT: &str = "01_DECISIONS.md";

const INIT_DB_TOML: &str = "[db]\nschema = 1\n\n[layers]\nL0 = \"hot\"\nL1 = \"lean\"\nL2 = \"full\"\n\n[caps]\nL0 = 100\nL1 = 30\nL2 = 8\nbudget_tokens = 8000\n\n[granularity]\nmin_mean_bytes = 40\nmax_gap_ratio = 0.55\n\n[bands]\norder = [\"decisions\"]\n";
const INIT_BAND_TOML: &str = "[band]\nid = \"decisions\"\ntitle = \"Decisions\"\nabstract = \"Decisions admitted into this DB, one entry per change.\"\nexecutor = \"cc\"\n\n[[documents]]\npath = \"01_DECISIONS.md\"\norder = 0\nrole = \"decisions\"\n";
const INIT_DECISIONS_MD: &str = "# Decisions\n";
const INIT_GITIGNORE: &str = "build/\n";
const INIT_HEAD: &str = "\n";

pub enum InitError {
    Exists(Vec<PathBuf>),
    Failed(String),
}

pub fn init_targets(dir: &Path) -> Vec<PathBuf> {
    let decisions_dir = band_dir(dir, DECISIONS_BAND);
    vec![
        db_manifest_path(dir),
        decisions_dir.join(BAND_MANIFEST),
        decisions_dir.join(DECISIONS_DOCUMENT),
        crate::store::journal_path(dir),
        crate::store::head_path(dir),
        crate::store::build_dir(dir),
        dir.join(".gitignore"),
    ]
}

pub fn init(dir: &Path) -> Result<Vec<PathBuf>, InitError> {
    let targets = init_targets(dir);
    let existing: Vec<PathBuf> = targets.iter().filter(|path| path.exists()).cloned().collect();
    if !existing.is_empty() {
        return Err(InitError::Exists(existing));
    }
    let decisions_dir = band_dir(dir, DECISIONS_BAND);
    let files: [(PathBuf, &[u8]); 6] = [
        (db_manifest_path(dir), INIT_DB_TOML.as_bytes()),
        (decisions_dir.join(BAND_MANIFEST), INIT_BAND_TOML.as_bytes()),
        (decisions_dir.join(DECISIONS_DOCUMENT), INIT_DECISIONS_MD.as_bytes()),
        (crate::store::journal_path(dir), b""),
        (crate::store::head_path(dir), INIT_HEAD.as_bytes()),
        (dir.join(".gitignore"), INIT_GITIGNORE.as_bytes()),
    ];
    let mut written: Vec<PathBuf> = Vec::new();
    for (path, bytes) in files {
        if let Err(error) = crate::store::write_atomic(&path, bytes) {
            return Err(InitError::Failed(error));
        }
        written.push(path);
    }
    let build = crate::store::build_dir(dir);
    if let Err(error) = fs::create_dir_all(&build) {
        return Err(InitError::Failed(format!("create dir {} failed: {}", build.display(), error)));
    }
    written.push(build);
    Ok(written)
}

pub fn load_db(db: &Path) -> Result<DbManifest, String> {
    let path = db_manifest_path(db);
    let label = path.display().to_string();
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return Err(format!(
                "{} could not be read: {}. Nothing was written and no default was substituted; a manifest nobody wrote is not a manifest.",
                label, error
            ))
        }
    };
    parse_db(&text, &label)
}

pub fn parse_db(text: &str, label: &str) -> Result<DbManifest, String> {
    let parsed: DbFile = match toml::from_str(text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Err(format!(
                "{} is present ({} byte) but does not parse as the root manifest: {}. The file on disk is untouched; no default was substituted.",
                label,
                text.len(),
                error
            ))
        }
    };
    if parsed.db.schema != 1 {
        return Err(format!(
            "{} declares schema {}; this engine reads schema 1 only, and reading a schema it does not know would be a guess.",
            label, parsed.db.schema
        ));
    }
    for (name, value) in [
        ("caps.L0", parsed.caps.l0),
        ("caps.L1", parsed.caps.l1),
        ("caps.L2", parsed.caps.l2),
        ("caps.budget_tokens", parsed.caps.budget_tokens),
    ] {
        if value < 1 {
            return Err(format!(
                "{} sets {} to {}; a cap below 1 can never return a row and paging could never advance, so it is refused rather than silently raised.",
                label, name, value
            ));
        }
    }
    let mut seen: Vec<&String> = Vec::new();
    for band in &parsed.bands.order {
        if seen.contains(&band) {
            return Err(format!(
                "{} lists band \"{}\" twice in bands.order; the band list is a partition, so a repeated claim is refused.",
                label, band
            ));
        }
        seen.push(band);
    }
    if parsed.bands.order.is_empty() {
        return Err(format!(
            "{} lists 0 band(s) in bands.order; an empty corpus is UNTESTABLE, never a pass.",
            label
        ));
    }
    let granularity = match &parsed.granularity {
        None => None,
        Some(section) => {
            if section.min_mean_bytes < 1 {
                return Err(format!(
                    "{} sets granularity.min_mean_bytes to {}; a floor below one byte can never be crossed, so it is refused rather than silently raised.",
                    label, section.min_mean_bytes
                ));
            }
            if !(section.max_gap_ratio > 0.0 && section.max_gap_ratio <= 1.0) {
                return Err(format!(
                    "{} sets granularity.max_gap_ratio to {}; a ratio outside the range above 0 and up to 1 measures nothing about a corpus.",
                    label, section.max_gap_ratio
                ));
            }
            Some(Granularity {
                min_mean_bytes: section.min_mean_bytes as usize,
                max_gap_ratio: section.max_gap_ratio,
            })
        }
    };
    Ok(DbManifest {
        schema: parsed.db.schema,
        layer_names: vec![
            ("L0".to_string(), parsed.layers.l0),
            ("L1".to_string(), parsed.layers.l1),
            ("L2".to_string(), parsed.layers.l2),
        ],
        caps: Caps {
            l0: parsed.caps.l0 as usize,
            l1: parsed.caps.l1 as usize,
            l2: parsed.caps.l2 as usize,
            budget_tokens: parsed.caps.budget_tokens as usize,
        },
        band_order: parsed.bands.order,
        granularity,
    })
}

pub fn load_band(db: &Path, band: &str, ord: usize) -> Result<BandManifest, String> {
    let dir = band_dir(db, band);
    let path = dir.join(BAND_MANIFEST);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            return Err(format!(
                "{} could not be read: {}. A band listed in bands.order must carry its own contract.",
                path.display(),
                error
            ))
        }
    };
    let parsed: BandFile = match toml::from_str(&text) {
        Ok(parsed) => parsed,
        Err(error) => {
            return Err(format!(
                "{} is present ({} byte) but does not parse as a band manifest: {}",
                path.display(),
                text.len(),
                error
            ))
        }
    };
    if parsed.band.id != band {
        return Err(format!(
            "{} declares id \"{}\" but sits in bands/{}/; the directory and the declaration must name the same band.",
            path.display(),
            parsed.band.id,
            band
        ));
    }
    if let Some(executor) = &parsed.band.executor {
        if !EXECUTORS.contains(&executor.as_str()) {
            return Err(format!(
                "{} declares band executor \"{}\", which is outside the closed set {}.",
                path.display(),
                executor,
                EXECUTORS.join(" ")
            ));
        }
    }
    let mut seen: Vec<&str> = Vec::new();
    for document in &parsed.documents {
        if !ROLES.contains(&document.role.as_str()) {
            return Err(format!(
                "{} gives {} the role \"{}\", which is outside the closed set {}.",
                path.display(),
                document.path,
                document.role,
                ROLES.join(" ")
            ));
        }
        if let Some(executor) = &document.executor {
            if !EXECUTORS.contains(&executor.as_str()) {
                return Err(format!(
                    "{} gives {} the executor \"{}\", which is outside the closed set {}.",
                    path.display(),
                    document.path,
                    executor,
                    EXECUTORS.join(" ")
                ));
            }
        }
        if seen.contains(&document.path.as_str()) {
            return Err(format!(
                "{} declares {} twice; two claims on one file is not a partition.",
                path.display(),
                document.path
            ));
        }
        seen.push(document.path.as_str());
    }
    let mut documents = parsed.documents;
    documents.sort_by_key(|item| (item.order, item.path.clone()));
    Ok(BandManifest {
        id: parsed.band.id,
        title: parsed.band.title,
        summary: parsed.band.summary,
        executor: parsed.band.executor,
        documents,
        dir,
        ord,
    })
}

pub fn load_bands(db: &Path, manifest: &DbManifest) -> Vec<BandLoad> {
    let mut out: Vec<BandLoad> = Vec::new();
    for (ord, band) in manifest.band_order.iter().enumerate() {
        out.push(BandLoad {
            id: band.clone(),
            outcome: load_band(db, band, ord),
        });
    }
    out
}

pub fn unclaimed_band_dirs(db: &Path, manifest: &DbManifest) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let entries = match fs::read_dir(bands_dir(db)) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !manifest.band_order.contains(&name) {
            out.push(name);
        }
    }
    out.sort();
    out
}

pub fn markdown_files(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) == Some("md") {
            out.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    out.sort();
    out
}

pub fn document_executor(band: &BandManifest, document: &DocumentDecl) -> String {
    match &document.executor {
        Some(value) => value.clone(),
        None => match &band.executor {
            Some(value) => value.clone(),
            None => crate::atom::UNKNOWN.to_string(),
        },
    }
}

pub fn layer_for_role(role: &str) -> Option<&'static str> {
    match role {
        "overview" => Some("L0"),
        "principles" => Some("L1"),
        "decisions" => Some("L1"),
        "evidence" => Some("L2"),
        "raw" => Some("L2"),
        "reference" => Some("L2"),
        _ => None,
    }
}

pub fn evidence_for_role(role: &str) -> Option<&'static str> {
    match role {
        "evidence" => Some(crate::atom::EVIDENCE_MEASURED),
        "decisions" => Some(crate::atom::EVIDENCE_DERIVED),
        _ => None,
    }
}
