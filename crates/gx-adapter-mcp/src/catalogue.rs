//! Which tools can be undone, and by what.
//!
//! 41 §4 requires `invert` to answer 「the delta that undoes this one」 or `Ok(None)`, and DR-1(a) is why
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
//! 契約 2 forbids inventing a second: a tool this catalogue has never heard of is planned, carried and
//! (if a policy admits it) called, exactly like one it knows. What the catalogue changes is one
//! question -- 「can this be undone?」 -- and the answer flows to the gate through
//! `GateInput.invert_available` (**E-M4-5**), which is where 41 §5 already puts it.
//!
//! An unknown tool is therefore a tool whose calls are **irreversible as far as gx knows**, and
//! **E-M3-4** escalates a change with no inverse to a person. That is the conservative direction and it
//! is the one an empty catalogue takes.

use std::collections::BTreeMap;

/// The restore tools a deployment declares.
///
/// A `BTreeMap` rather than a hash map because this value is small, is read far more often than it is
/// written, and has a deterministic `Debug` -- which matters when it appears in a refusal message.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    restores: BTreeMap<String, String>,
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

    /// 「a call to `tool` is undone by a call to `restored_by`, handed the resource's prior contents」.
    ///
    /// The arguments the inverse hands over are [`crate::restore_arguments`]'s -- canonical DAG-CBOR of
    /// `{contents, uri}`, which is MCP's own `resources/read` shape (`delta.rs`, 「the restore
    /// convention」).
    #[must_use]
    pub fn with_restore(mut self, tool: impl Into<String>, restored_by: impl Into<String>) -> Self {
        self.restores.insert(tool.into(), restored_by.into());
        self
    }

    /// The tool that undoes a call to `tool`, if the deployment declared one.
    #[must_use]
    pub fn restore_for(&self, tool: &str) -> Option<&str> {
        self.restores.get(tool).map(String::as_str)
    }

    /// How many tools this catalogue can undo. Printed by `tests/mcp_conformance.rs` so that a run
    /// against an empty catalogue cannot look like a run against a full one.
    #[must_use]
    pub fn declared(&self) -> usize {
        self.restores.len()
    }
}
