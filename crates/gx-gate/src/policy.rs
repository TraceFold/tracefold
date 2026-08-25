// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Glovrex
//! The Cedar policy set, the request it is asked, and the escalation ticket (42 §3.8, FR-025).
//!
//! Spec: 32 FR-025 for the evaluation, 34 AC-025 for the two cases, 42 §3.8 for the field tables,
//! 42 §1.3 and 42 §3.8 for the IdentityView and the external resolution table, 43 T-5 for who
//! resolves a ticket. Rulings: **ASM-60-1** (the request mapping), **E-M3-3** (evaluation that
//! cannot be performed is not a verdict), **M3-04** (when a gate escalates at all),
//! **M3-11 / ASM-60-3** (the default approval requirement).
//!
//! # The mapping, in one table (ASM-60-1)
//!
//! req/38 §19 rules the shape and req/60 §5.2 asks this hand to write it down: "file the minimal
//! mapping (actor->principal, ChangeContext+order->action, pre.locator+substrate->resource, the
//! rest->context) as **ASM-60-1**; no entity schema in M3" (sem: SEM-gx-gate-106). What the primary fixes is that
//! "P, A, and R are entity references, while C is a record"
//! (<https://docs.cedarpolicy.com/auth/authorization.html>), so every row below lands in the only
//! slot its Cedar type admits:
//!
//! | 41 §3 / §4 (gx) | Cedar (PARC) | spelling | why here |
//! |---|---|---|---|
//! | `t.actor.key()` | principal | `GxActor::"<KeyId>"` | 42 §3.2 makes `KeyId` the same namespace as a DSSE `keyid`, so the principal is already the join key |
//! | `t.context` | action | `GxAction::"Time" \| … \| "Custom:<s>"` | the classification a change belongs to (P-3). gx has no read/write verb: every `Transformation` is a change |
//! | `pre.substrate` + `pre.locator()` | resource | `GxResource::"<substrate>:<locator>"` + attributes `substrate`, `locator` | the id names it; the two attributes are what a policy can compare (see below) |
//! | `t.order()` | context.`order` | Long | 0, 1 or 2 (ASM-6). AC-029's order-2 case is reachable from a policy as `context.order == 2` |
//! | `invert_available` | context.`invert_available` | Bool | FR-043 / AC-048; **E-M3-4** makes `false` the escalation trigger, in hand 4 |
//! | `evidence` | context.`evidence_count`, context.`evidence_kinds` | Long, Set\<String\> | the "evidence summary" of ASM-60-1 (sem: SEM-gx-gate-107). The bodies stay out: 42 §5 keeps evidence in its own store and gx-gate mints no CID |
//!
//! **Why the resource carries attributes.** `like` is Cedar's only string matcher and it is defined
//! over strings, not over entity references (<https://docs.cedarpolicy.com/policies/syntax-operators.html>).
//! An id alone therefore supports `resource == GxResource::"fs:/etc/passwd"` and nothing wider, and
//! AC-025 asks for `/etc/**`. The locator is put on the entity rather than into the context because
//! ASM-60-1 maps it to the **resource**: a fact reachable at two spellings is two facts to keep in
//! step. The store that carries it holds exactly the three entities the mapping names, and no
//! schema -- "no entity schema in M3" -- which the primary permits: entity data is needed
//! "when a policy references entity attributes", schema is the validator's tool. (sem: SEM-gx-gate-108)
//!
//! **Where `order` went.** The ruling reads "ChangeContext+order->action" with the note that
//! "order may also live on the context side -- settled by hand 2" (sem: SEM-gx-gate-109). It is in the context: an action id that concatenated the
//! order would make the common policy -- "this kind of change" -- unwritable without enumerating
//! three ids, while `context.order` is a Long a policy can compare and range over.
//!
//! # What this module does not build
//!
//! Resolution. 42 §3.8: "`status: Pending | Approved | Rejected`, `resolved_by`, `resolved_at` are
//! managed in a separate engine-store table keyed by `TicketId`, and are not included in
//! `EscalationTicket`'s own CID" (sem: SEM-gx-gate-110). req/60 §1 N-03 keeps that table out of M3 entirely -- FR-058's implementation crates
//! are gx-engine, gx-cli and gx-api. **This file builds the vessel and nothing that fills it.**
//!
//! The invariant registry (hand 3), the meet of the two systems (AC-027, hand 4) and the shipped
//! pack (FR-028, hand 5). What is here is the policy half: a set, a request, a decision.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use cedar_policy::{
    Authorizer, Context, Decision, Entities, Entity, EntityId, EntityTypeName, EntityUid, PolicyId,
    PolicySet, RestrictedExpression,
};
use gx_canon::cid::IdentityView;
use gx_core::{ChangeContext, Cid, KeyId, SubstrateKind, Timestamp, TransformationId};
use gx_witness::{Evidence, PolicyDecision};
use serde::{Deserialize, Serialize};

use crate::verdict::{PolicyDecisionRecord, Reason, ReasonSource};
use crate::{Error, GateInput, Result};

/// A ticket's identity (42 §3.8: `TicketId(Cid)`).
///
/// `IdentityView = {transformation, reasons, required_approval}` (42 §1.3) -- `id` itself is
/// excluded as self-referential and `created_at` by ASM-4. **Nothing in this hand computes one.**
/// Minting a CID goes through gx-canon (41 §6: "every canonical encode goes through gx-canon only") (sem: SEM-gx-gate-111), which
/// this crate does not name yet, so the field is supplied by whoever builds the ticket and a value
/// here is a container rather than a claim that the ticket hashes to it. Same sentence
/// `gx_core::Cid` carries about itself, for the same reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TicketId(pub Cid);

/// How many approvals, and whose (42 §3.8).
///
/// 42 §3.8 defers the meaning to "32 (unconfirmed)", and 32 is written and says nothing: req/60 §3
/// M3-11 measured `min_approvers` / `eligible_actors` / the approval workflow (sem: SEM-gx-gate-112) at **0 occurrences** in
/// 32, 34 and 44. The ruling fills the hole with a default rather than a design:
///
/// > "M3-11 (adopted = option a): M3 is the vessel only; default value **ASM-60-3**
/// > (min_approvers=1, eligible_actors=[] = no restriction); the satisfaction check is M5's (43
/// > T-5)" (sem: SEM-gx-gate-113)
///
/// So [`Default`] below *is* ASM-60-3, and it is the only place the numbers appear.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequirement {
    /// 42 §3.8's `min_approvers: u8`.
    pub min_approvers: u8,
    /// 42 §3.8's `eligible_actors: Vec<KeyId>`. **Empty means unrestricted, not "nobody"** (sem: SEM-gx-gate-114)
    /// (ASM-60-3). The other reading would make the default ticket unresolvable, which 43's
    /// liveness invariants (INV-L, AC-045) forbid outright -- an escalation that no actor may
    /// approve reaches `Aborted(Expired)` and nothing else.
    pub eligible_actors: Vec<KeyId>,
}

/// **ASM-60-3**, and the only spelling of it.
///
/// Who checks it: M5. 43 T-5 is where a signed human decision is required, and req/60 §1 N-03
/// keeps the check out of M3. A gate that counted approvals would be a gate holding state.
impl Default for ApprovalRequirement {
    fn default() -> Self {
        Self {
            min_approvers: 1,
            eligible_actors: Vec::new(),
        }
    }
}

/// The escalation itself (42 §3.8).
///
/// # When one is produced
///
/// Not in this hand, and the rule is nonetheless already fixed. req/60 §3 M3-04 measured that **no
/// condition anywhere in M3's FRs or ACs produces an `Escalate`**, and the ruling promotes the gate
/// half of AC-048 into an M3 requirement rather than leaving the arm unreachable:
///
/// > "M3-04 (adopted = option a): raise AC-048's gate-side half to an M3 requirement -- make
/// > **`invert_available=false -> Escalate`** the minimal generation rule for M3 (a policy firing an
/// > extra condition is M4's). Reason: option c (leave it unreachable) lets M5's AC-032 pass on a
/// > mock, hiding the defect until M5" -> **E-M3-4** (sem: SEM-gx-gate-115)
///
/// Hand 4 wrote that rule: [`crate::Gate::verify`]'s `escalate`, which is the only producer of a
/// ticket in the crate.
///
/// # No `Deserialize` (hand 4)
///
/// It derived one until the reasons inside it gained a checked constructor. `Reason` is built
/// through [`Reason::new`] so that **E-M3-6**'s vocabulary and **M3-21**'s bound hold of every
/// reason that exists; a `Deserialize` on this struct would rebuild the whole vector past both
/// checks, which is the second door hand 1 refused to open on [`crate::AdmitProof`] for E-M3-9.
/// M5 stores and resolves tickets (42 §3.8's external table, 43 T-5) and will need a wire face --
/// that face has to arrive through a checked constructor, and saying so here is cheaper than
/// discovering it after a receipt has been written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EscalationTicket {
    /// 42 §1.3 excludes this from the IdentityView (self-reference); see [`TicketId`] for who
    /// computes it, which is nobody in M3 hand 1.
    pub id: TicketId,
    /// The transformation whose decision is being raised to a human (42 §3.8) -- the join key
    /// the resolving side (M5's ticket table, 43 T-5) answers against.
    pub transformation: TransformationId,
    /// Why it could not be decided here. Non-empty for the same reason `Verdict::Deny`'s vector is
    /// (see [`crate::Verdict::deny`]) -- but unlike that one this is a plain field, because the
    /// hand that produces tickets is hand 4 and a constructor without a producer is a constructor
    /// nothing checks.
    pub reasons: Vec<Reason>,
    /// Who may resolve the ticket (42 §3.8). Stated by the gate, enforced by whoever runs the
    /// approval flow -- the gate itself never resolves anything.
    pub required_approval: ApprovalRequirement,
    /// 42 §3.8 keeps the field and ASM-4 keeps it out of the CID. gx-gate cannot read a clock
    /// (41 §6: "randomness and time are injected at the engine boundary") (sem: SEM-gx-gate-116), so the value comes in from the caller.
    pub created_at: Timestamp,
}

// ---------------------------------------------------------------------------
// ASM-60-1: the request, as a gx value
// ---------------------------------------------------------------------------

/// The entity type of a principal. Every actor is one of these, whatever `Actor` variant it is.
///
/// FR-006 / P-7: "accountability does not depend on whether the actor was a person, an agent or a
/// process", and encoding the variant into the type name would put that branch into the scope of (sem: SEM-gx-gate-117)
/// every policy anyone writes. What is therefore **not** visible to a policy today is the variant
/// and an agent's `model`; that gap is raised in `req/62_M3_HAND2_REPORT_2026-08-08.md` §4 rather
/// than closed by inventing rows ASM-60-1 does not have.
pub const PRINCIPAL_TYPE: &str = "GxActor";

/// The entity type of an action, whose id is the [`ChangeContext`] (41 §3, P-3).
pub const ACTION_TYPE: &str = "GxAction";

/// The entity type of a resource: the object the transformation is about.
pub const RESOURCE_TYPE: &str = "GxResource";

/// 42 §3.1's `locator`, as an attribute a policy can match with `like`.
pub const RESOURCE_ATTR_LOCATOR: &str = "locator";

/// 42 §3.1's `substrate`, as an attribute a policy can compare.
pub const RESOURCE_ATTR_SUBSTRATE: &str = "substrate";

/// 41 §3's `order`, as a context Long.
pub const CONTEXT_ORDER: &str = "order";

/// 41 §4's `invert_available`, as a context Bool.
pub const CONTEXT_INVERT_AVAILABLE: &str = "invert_available";

/// How many evidence items were shown (the count half of ASM-60-1's "evidence summary"). (sem: SEM-gx-gate-118)
pub const CONTEXT_EVIDENCE_COUNT: &str = "evidence_count";

/// Which kinds of evidence were shown (the kinds half), as a Set of 42 §3.7's four variant names.
pub const CONTEXT_EVIDENCE_KINDS: &str = "evidence_kinds";

/// The annotation a policy declares its id with.
///
/// Cedar's primary: "@id is not special in the Cedar language, it behaves like any other (sem: SEM-gx-gate-119)
/// annotation. The Cedar CLI uses the @id annotation to set policy IDs, but other interfaces, such
/// as the Cedar APIs, have other ways to set policy IDs" (sem: SEM-gx-gate-120)
/// (<https://docs.cedarpolicy.com/policies/syntax-policy.html>). gx takes the CLI's convention at
/// the API level, and [`PolicyEngine::parse`] refuses a policy without one -- see there for why a
/// positional id would reach a receipt's CID.
pub const POLICY_ID_ANNOTATION: &str = "id";

// The sentinel `cedar:default-deny` stood here until hand 4. Cedar's third rule -- "Otherwise
// (i.e., no policy is satisfied), the final result is Deny", with an empty list of determining (sem: SEM-gx-gate-121)
// policies (<https://docs.cedarpolicy.com/auth/authorization.html>) -- produced a refusal that 42
// §3.8's four `ReasonSource` variants could not name, so hand 2 spelled the absence as a policy id
// and refused any pack that claimed it. **E-M3-11** replaced the spelling with a variant
// ([`crate::ReasonSource::NoPolicyApplied`]), and the ruling required both halves in one hand:
// "the implementation is replaced by a variant in hand 4 ... and the sentinel spelling is deleted
// at the same time" (sem: SEM-gx-gate-122). With nothing to impersonate, the
// parse-time refusal went too -- a pack may now name a policy anything, and `tests/ac_025.rs`
// measures that a policy so named still reports as itself.
//
// The `Reason.code` a policy refusal carries is `crate::POLICY_DENIED`, which hand 4 moved into the
// crate root beside the rest of the vocabulary (E-M3-6): one declaration, and one place a reader
// can see the whole set.

/// A value a policy can read out of `context` (the C of PARC, which the primary types as a record).
///
/// Four shapes, named after Cedar's own types rather than after Rust's, because the table in the
/// module doc has to be checkable against the primary word for word.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextValue {
    /// Cedar `String`.
    Str(String),
    /// Cedar `Long`, which is an `i64`.
    Long(i64),
    /// Cedar `Bool`.
    Bool(bool),
    /// Cedar `Set` of `String`. Ordered, so that the request built from a view is a function of
    /// the view and not of a hash seed.
    Set(BTreeSet<String>),
}

impl ContextValue {
    fn to_cedar(&self) -> RestrictedExpression {
        match self {
            ContextValue::Str(s) => RestrictedExpression::new_string(s.clone()),
            ContextValue::Long(n) => RestrictedExpression::new_long(*n),
            ContextValue::Bool(b) => RestrictedExpression::new_bool(*b),
            ContextValue::Set(values) => RestrictedExpression::new_set(
                values
                    .iter()
                    .map(|s| RestrictedExpression::new_string(s.clone())),
            ),
        }
    }
}

/// **ASM-60-1** as a value: what a [`GateInput`] becomes before Cedar is named.
///
/// The mapping is a projection and it is written down twice on purpose -- as the table in this
/// module's doc, and as this struct. A doc table alone drifts silently; a `cedar_policy::Request`
/// alone cannot be compared, because Cedar's `Request` exposes its parts as `Option<&EntityUid>`
/// and its context not at all. So the view is the thing tests assert about
/// (`crates/gx-gate/tests/policy_mapping.rs`), and `RequestView::to_cedar` (private) is the only road from
/// it to Cedar.
///
/// Total and infallible: every field of `GateInput` that ASM-60-1 names has a slot, and none of the
/// values can fail to project. What *can* fail is turning the view into a Cedar request, which is
/// why that is a separate step with a `Result`.
///
/// The `planned` field of `GateInput` is deliberately absent from every row. 42 §3.4 makes a
/// delta's payload opaque below the adapter (P-6) and req/38 §19's M3-10 fixes the consequence in
/// words: "the v0.1 pack's effective reach is **stated explicitly as the locator/actor/context/order
/// class** (no overclaiming)" (sem: SEM-gx-gate-123). A policy
/// cannot see what the change does; it sees what the change is about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestView {
    principal_id: String,
    action_id: String,
    resource_id: String,
    resource_attrs: BTreeMap<String, String>,
    context: BTreeMap<String, ContextValue>,
}

impl RequestView {
    /// Project a gate input onto PARC.
    #[must_use]
    pub fn of(input: &GateInput<'_>) -> Self {
        let locator = input.pre.locator().to_string();
        let substrate = substrate_tag(input.pre.substrate());

        let mut resource_attrs = BTreeMap::new();
        resource_attrs.insert(RESOURCE_ATTR_LOCATOR.to_string(), locator.clone());
        resource_attrs.insert(RESOURCE_ATTR_SUBSTRATE.to_string(), substrate.clone());

        let mut kinds = BTreeSet::new();
        for item in input.evidence {
            kinds.insert(evidence_kind(item).to_string());
        }

        let mut context = BTreeMap::new();
        context.insert(
            CONTEXT_ORDER.to_string(),
            ContextValue::Long(i64::from(input.t.order())),
        );
        context.insert(
            CONTEXT_INVERT_AVAILABLE.to_string(),
            ContextValue::Bool(input.invert_available),
        );
        context.insert(
            CONTEXT_EVIDENCE_COUNT.to_string(),
            // `as` is not in reach here: a slice length above i64::MAX cannot exist on any
            // machine with an address space, and `try_into` would add an error path nothing can
            // reach. The conversion is written out so the claim is visible rather than implied.
            ContextValue::Long(i64::try_from(input.evidence.len()).unwrap_or(i64::MAX)),
        );
        context.insert(CONTEXT_EVIDENCE_KINDS.to_string(), ContextValue::Set(kinds));

        Self {
            principal_id: input.t.actor.key().clone(),
            action_id: action_id(&input.t.context),
            // The id is opaque and is never parsed back apart: a locator containing a colon makes
            // the halves ambiguous to a reader and identical to Cedar, which compares whole ids.
            // What the halves are separately is what the two attributes carry.
            resource_id: format!("{substrate}:{locator}"),
            resource_attrs,
            context,
        }
    }

    /// `GxActor::"<this>"`.
    #[must_use]
    pub fn principal_id(&self) -> &str {
        &self.principal_id
    }

    /// `GxAction::"<this>"`.
    #[must_use]
    pub fn action_id(&self) -> &str {
        &self.action_id
    }

    /// `GxResource::"<this>"`.
    #[must_use]
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }

    /// The attributes the resource entity carries, by name.
    #[must_use]
    pub fn resource_attrs(&self) -> &BTreeMap<String, String> {
        &self.resource_attrs
    }

    /// The C of PARC, by name.
    #[must_use]
    pub fn context(&self) -> &BTreeMap<String, ContextValue> {
        &self.context
    }

    /// The one road from a view to Cedar: a request and the entity store the mapping names.
    ///
    /// `t` is carried only so that a failure can say which transformation could not be mapped;
    /// nothing about it reaches the request except through the view.
    ///
    /// # Errors
    /// [`Error::Unevaluable`] when Cedar refuses any part of the construction -- a type name that
    /// does not parse, a context that is not a record, an attribute expression it cannot evaluate.
    /// None of these is a decision about the transformation (**E-M3-3**).
    fn to_cedar(&self, t: TransformationId) -> Result<(cedar_policy::Request, Entities)> {
        let unevaluable = |detail: String| Error::Unevaluable {
            transformation: t,
            detail,
        };

        let principal = uid(PRINCIPAL_TYPE, &self.principal_id)
            .map_err(|e| unevaluable(format!("principal: {e}")))?;
        let action =
            uid(ACTION_TYPE, &self.action_id).map_err(|e| unevaluable(format!("action: {e}")))?;
        let resource = uid(RESOURCE_TYPE, &self.resource_id)
            .map_err(|e| unevaluable(format!("resource: {e}")))?;

        let context = Context::from_pairs(
            self.context
                .iter()
                .map(|(key, value)| (key.clone(), value.to_cedar())),
        )
        .map_err(|e| unevaluable(format!("context: {e}")))?;

        let request = cedar_policy::Request::new(
            principal.clone(),
            action.clone(),
            resource.clone(),
            context,
            // No schema, in either place it could go. req/38 §19: "no entity schema in M3". (sem: SEM-gx-gate-124)
            None,
        )
        .map_err(|e| unevaluable(format!("request: {e}")))?;

        let attrs: HashMap<String, RestrictedExpression> = self
            .resource_attrs
            .iter()
            .map(|(key, value)| (key.clone(), RestrictedExpression::new_string(value.clone())))
            .collect();
        let resource_entity = Entity::new(resource, attrs, HashSet::new())
            .map_err(|e| unevaluable(format!("resource entity: {e}")))?;

        // Exactly the three entities the mapping names. The principal and the action carry no
        // attributes because gx knows nothing else about them that a policy may read -- an empty
        // entity is the truthful shape, and it is not the same as an absent one.
        let entities = Entities::from_entities(
            [
                Entity::with_uid(principal),
                Entity::with_uid(action),
                resource_entity,
            ],
            None,
        )
        .map_err(|e| unevaluable(format!("entities: {e}")))?;

        Ok((request, entities))
    }
}

fn uid(type_name: &str, id: &str) -> core::result::Result<EntityUid, String> {
    let parsed: EntityTypeName = type_name
        .parse()
        .map_err(|e| format!("{type_name:?} is not an entity type name: {e}"))?;
    // `EntityId::new` takes the string as it is. Building the uid by formatting
    // `GxResource::"<id>"` and parsing that back would make a locator containing a quote or a
    // backslash able to name a different entity, which is an injection with a policy engine on
    // the other end of it.
    Ok(EntityUid::from_type_name_and_id(parsed, EntityId::new(id)))
}

fn substrate_tag(substrate: &SubstrateKind) -> String {
    match substrate {
        SubstrateKind::Fs => "fs".to_string(),
        SubstrateKind::Git => "git".to_string(),
        SubstrateKind::Mcp => "mcp".to_string(),
        SubstrateKind::Custom(name) => format!("custom:{name}"),
    }
}

fn action_id(context: &ChangeContext) -> String {
    match context {
        ChangeContext::Time => "Time".to_string(),
        ChangeContext::Evidence => "Evidence".to_string(),
        ChangeContext::Policy => "Policy".to_string(),
        ChangeContext::Model => "Model".to_string(),
        ChangeContext::Representation => "Representation".to_string(),
        ChangeContext::Substrate => "Substrate".to_string(),
        ChangeContext::Custom(name) => format!("Custom:{name}"),
    }
}

fn evidence_kind(evidence: &Evidence) -> &'static str {
    match evidence {
        Evidence::TestResult { .. } => "TestResult",
        Evidence::Measurement { .. } => "Measurement",
        Evidence::ExternalAttestation { .. } => "ExternalAttestation",
        Evidence::PolicyEvaluation { .. } => "PolicyEvaluation",
    }
}

// ---------------------------------------------------------------------------
// FR-025: the policy set, and what it answers
// ---------------------------------------------------------------------------

/// What Cedar answered, in gx's vocabulary.
///
/// 42 §3.7 already fixed the two words: `PolicyDecision { Allow, Deny }`, "the same vocabulary as
/// `cedar_policy::Decision`" (sem: SEM-gx-gate-125). This type is the decision together with the policies that produced it, which is
/// what 42 §3.8's `PolicyDecisionRecord` records and what an `AdmitProof` carries into a receipt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PolicyOutcome {
    decision: PolicyDecision,
    determining: Vec<PolicyDecisionRecord>,
}

impl PolicyOutcome {
    /// `Allow` or `Deny`, and nothing else -- an evaluation that could not be performed left
    /// through `Err` before this value existed (**E-M3-3**).
    #[must_use]
    pub fn decision(&self) -> PolicyDecision {
        self.decision
    }

    /// The policies that decided, by id, in id order.
    ///
    /// The primary defines the set: "If the decision is Allow, the determining policies are the (sem: SEM-gx-gate-126)
    /// permit policies that satisfy the request. Otherwise, the determining policies are the forbid
    /// policies, if any, that satisfy the request. If the decision is Deny because no policies were
    /// satisfied (rule 3), then the list of determining policies is empty". So every record here (sem: SEM-gx-gate-127)
    /// carries the same decision the whole response did, and an empty slice on a `Deny` is Cedar's
    /// third rule rather than a lost record.
    #[must_use]
    pub fn determining(&self) -> &[PolicyDecisionRecord] {
        &self.determining
    }

    /// The reasons a refusal is written down with (42 §3.8).
    ///
    /// One per determining policy, or one carrying [`ReasonSource::NoPolicyApplied`] for Cedar's
    /// third rule (**E-M3-11**). Empty when the decision was `Allow`, which is why
    /// [`crate::Gate::verify`] only asks for it on the other branch: 42 §3.8 gives reasons to
    /// `Deny`, not to `Admit`.
    ///
    /// # Errors
    /// [`Error::UnknownReasonCode`] cannot happen here -- the code is [`crate::POLICY_DENIED`],
    /// which is in the vocabulary by construction -- but the constructor is the only door into a
    /// [`Reason`] (E-M3-6) and a door that some callers may walk around is not a door.
    pub fn deny_reasons(&self) -> Result<Vec<Reason>> {
        if self.decision == PolicyDecision::Allow {
            return Ok(Vec::new());
        }
        if self.determining.is_empty() {
            return Ok(vec![Reason::new(
                crate::POLICY_DENIED,
                "no policy in the set permitted this transformation".to_string(),
                ReasonSource::NoPolicyApplied,
            )?]);
        }
        self.determining
            .iter()
            .map(|record| {
                Reason::new(
                    crate::POLICY_DENIED,
                    format!("policy {} forbids this transformation", record.policy_id),
                    ReasonSource::Policy {
                        policy_id: record.policy_id.clone(),
                    },
                )
            })
            .collect()
    }
}

/// `EscalationTicket`'s projection (42 §1.3: `{transformation, reasons, required_approval}`).
///
/// `id` is out because it is what the projection computes, and `created_at` is out by ASM-4 -- 42
/// §3.8 says so in the table itself: "`created_at` is excluded from the IdentityView by ASM-4 --
/// the field itself remains but does not contribute to the CID computation" (sem: SEM-gx-gate-128). Two escalations of the same change, for the same reasons,
/// requiring the same approval, are therefore one ticket whenever they were raised.
#[derive(Debug, Serialize)]
pub struct EscalationTicketView<'a> {
    /// [`EscalationTicket::transformation`], identity-bearing.
    pub transformation: TransformationId,
    /// [`EscalationTicket::reasons`], identity-bearing: the same change escalated for different
    /// reasons is a different ticket.
    pub reasons: &'a [Reason],
    /// [`EscalationTicket::required_approval`], identity-bearing.
    pub required_approval: &'a ApprovalRequirement,
}

impl IdentityView for EscalationTicket {
    type View<'a>
        = EscalationTicketView<'a>
    where
        Self: 'a;

    fn identity_view(&self) -> EscalationTicketView<'_> {
        EscalationTicketView {
            transformation: self.transformation,
            reasons: &self.reasons,
            required_approval: &self.required_approval,
        }
    }
}

/// A parsed `PolicySet` and the authorizer that asks it questions (41 §4: "Cedar PolicySet"). (sem: SEM-gx-gate-129)
///
/// # What is not re-implemented here
///
/// Cedar's decision. req/60 §7.2: "**a gx test does not re-implement Cedar's decision**. What is
/// measured is not 'Cedar decided this' but '**gx's mapping built this request, and its decision
/// mapped onto this branch of the Verdict**'" (sem: SEM-gx-gate-130), and 46 §1 puts Cedar's own formalisation outside the project's scope. What this type owns
/// is the mapping in, the two words out, and the one judgement gx makes for itself: an evaluation
/// that produced errors is not a decision.
pub struct PolicyEngine {
    policies: PolicySet,
    authorizer: Authorizer,
}

impl core::fmt::Debug for PolicyEngine {
    /// By the ids it holds. `Authorizer` is a strategy object with nothing to show, and printing a
    /// whole policy set into a panic message would bury the id that matters.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PolicyEngine")
            .field("policy_ids", &self.policy_ids())
            .finish()
    }
}

impl PolicyEngine {
    /// Parse a policy set from Cedar source, giving every policy the id its `@id` says.
    ///
    /// # Why the annotation is required
    ///
    /// `PolicySet::from_str` names policies by position -- "Policy ids will default to 'policy*'
    /// with numbers from 0" (docs.rs, cedar-policy 4.12.0) (sem: SEM-gx-gate-131). gx puts the deciding policy's id into
    /// `PolicyDecisionRecord`, 42 §1.3 puts that record inside `AdmitProof`'s IdentityView, and an
    /// `AdmitProof` reaches a receipt's CID (42 §3.10). A positional id would therefore make the
    /// identity of a proof depend on the order statements happen to be written in, and moving two
    /// statements in a pack would change what a receipt says without changing what was decided.
    ///
    /// # Errors
    /// [`Error::PolicySetUnreadable`] when the source does not parse, when a policy carries no
    /// `@id` (or an empty one), when two policies claim the same id, or when the set contains a
    /// template (M3 links none, and `PolicySet::from_policies` would drop them silently).
    ///
    /// A fourth refusal stood here until hand 4: a policy claiming the id `cedar:default-deny`,
    /// the sentinel hand 2 used for a `Deny` no policy decided. **E-M3-11** replaced the sentinel
    /// with [`ReasonSource::NoPolicyApplied`], so there is no longer a name to impersonate and the
    /// refusal has nothing left to protect.
    pub fn parse(src: &str) -> Result<Self> {
        let unreadable = |detail: String| Error::PolicySetUnreadable { detail };

        let parsed: PolicySet = src
            .parse()
            .map_err(|e| unreadable(format!("cedar refused the policy source: {e}")))?;

        if parsed.num_of_templates() != 0 {
            return Err(unreadable(format!(
                "the set holds {} template(s); M3 links none, and rebuilding a set from its \
                 policies would drop them without saying so",
                parsed.num_of_templates()
            )));
        }

        let mut named = Vec::new();
        for policy in parsed.policies() {
            let Some(id) = policy.annotation(POLICY_ID_ANNOTATION) else {
                return Err(unreadable(format!(
                    "the policy cedar numbered {:?} carries no @{POLICY_ID_ANNOTATION} annotation; \
                     a positional id would reach a receipt's CID",
                    policy.id().to_string()
                )));
            };
            if id.is_empty() {
                return Err(unreadable(format!(
                    "the policy cedar numbered {:?} carries an empty @{POLICY_ID_ANNOTATION}",
                    policy.id().to_string()
                )));
            }
            named.push(policy.new_id(PolicyId::new(id)));
        }

        let policies = PolicySet::from_policies(named)
            .map_err(|e| unreadable(format!("two policies claim one id, or cedar refused: {e}")))?;

        Ok(Self {
            policies,
            authorizer: Authorizer::new(),
        })
    }

    /// The ids this set answers under, in order.
    #[must_use]
    pub fn policy_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .policies
            .policies()
            .map(|p| p.id().to_string())
            .collect();
        ids.sort();
        ids
    }

    /// How many policies. Zero is a set that permits nothing, not a set that decides nothing.
    #[must_use]
    pub fn len(&self) -> usize {
        self.policies.num_of_policies()
    }

    /// 🔴 **R36 / `req/476` M-01** — how many of them are `permit`, as **Cedar** counts them.
    ///
    /// [`crate::packs::declares_its_default`] is the caller, and the reason this is here rather
    /// than there is that "how many permits does this pack ship" is a question about the parsed
    /// policy set and only looks like a question about text. Answering it from the text is what
    /// F3 did until R36, and a `permit` written on the same line as its `@id` annotation was
    /// invisible to it: the pack shipped a permit, declared no permit default, and passed the
    /// clause whose entire subject is that a pack must declare its default out loud.
    #[must_use]
    pub fn permit_count(&self) -> usize {
        self.policies
            .policies()
            .filter(|p| p.effect() == cedar_policy::Effect::Permit)
            .count()
    }

    /// Whether the set is empty. Cedar's default-deny answers every request against it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.policies.is_empty()
    }

    /// Ask the set about one gate input (FR-025).
    ///
    /// # Errors
    /// [`Error::Unevaluable`] when the mapping cannot be turned into a Cedar request, and -- the
    /// judgement gx makes for itself -- when Cedar's diagnostics carry any evaluation error.
    ///
    /// Cedar's own rule is to skip a policy that errored: "skip on error: if a policy's evaluation
    /// returns error, the policy does not factor into the authorization response; it is skipped",
    /// with the escape hatch named in the same paragraph -- "applications can always choose to look (sem: SEM-gx-gate-132)
    /// at the authorization response's diagnostics and take a different decision if an evaluated
    /// policy produces errors" (<https://docs.cedarpolicy.com/auth/authorization.html>) (sem: SEM-gx-gate-133). For an
    /// authorization service, skipping is the safer default; for a gate whose forbid policy is the
    /// thing standing between an agent and `/etc/passwd`, a skipped forbid is a fail-open, and one
    /// that leaves the response looking exactly like a clean Allow. `Decision::Deny` carries the
    /// mirror image of the same problem -- docs.rs, 4.12.0: "This is also returned if sufficiently (sem: SEM-gx-gate-134)
    /// fatal errors are encountered such that no decision could be safely reached; for example,
    /// errors parsing the policies" -- so a Deny read without its diagnostics can mean "refused"
    /// or "broken" (sem: SEM-gx-gate-135). **E-M3-3** says those two must not be one verdict, so both fold into `Err`.
    pub fn evaluate(&self, input: &GateInput<'_>) -> Result<PolicyOutcome> {
        let view = RequestView::of(input);
        let (request, entities) = view.to_cedar(input.t.id)?;

        let response = self
            .authorizer
            .is_authorized(&request, &self.policies, &entities);

        let mut errors: Vec<String> = response
            .diagnostics()
            .errors()
            .map(std::string::ToString::to_string)
            .collect();
        if !errors.is_empty() {
            errors.sort();
            return Err(Error::Unevaluable {
                transformation: input.t.id,
                detail: format!(
                    "cedar reported {} policy evaluation error(s), so the decision it did reach \
                     was reached with policies skipped: {}",
                    errors.len(),
                    errors.join("; ")
                ),
            });
        }

        let decision = match response.decision() {
            Decision::Allow => PolicyDecision::Allow,
            Decision::Deny => PolicyDecision::Deny,
        };

        // Sorted, because the iterator's order is not part of Cedar's contract and this vector
        // reaches a CID through `AdmitProof` (E-M3-9 sorts it again there; sorting here is what
        // makes the *outcome* a function of the input, which is what the determinism property
        // measures). Duplicates are not folded -- H4-3: "multiplicity is not folded; only order is canonicalized". (sem: SEM-gx-gate-136)
        let mut ids: Vec<String> = response
            .diagnostics()
            .reason()
            .map(std::string::ToString::to_string)
            .collect();
        ids.sort();

        let determining = ids
            .into_iter()
            .map(|policy_id| PolicyDecisionRecord {
                policy_id,
                decision,
                // 42 §3.8 keeps diagnostics by digest, and minting one goes through gx-canon
                // (41 §6). gx-gate does not name gx-canon: "the hand that mints one is the hand
                // that adds the edge" (sem: SEM-gx-gate-137), and this hand mints nothing.
                diagnostics_digest: None,
            })
            .collect();

        Ok(PolicyOutcome {
            decision,
            determining,
        })
    }
}
