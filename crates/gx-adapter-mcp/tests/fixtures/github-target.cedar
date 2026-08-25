// The github/github-mcp-server **target-scoped example pack** (req/150 §A1-1, req/151's target
// pick, req/152's A1-1 implementation report).
//
// # Why it is not under `policies/mcp/`
//
// `crates/gx-gate/tests/pack_embedding.rs::nothing_ships_in_policies_that_no_build_loads` asserts
// that every `.cedar` under `policies/` is the one file `crates/gx-gate/src/packs.rs` embeds as the
// **shipped default**. This is not that: it is one target's example, read at run time by
// `gx wrap --policy <this file>` / `gx undo --policy <this file>`, the same road
// `crates/gx-cli/tests/fixtures/permit-postgres.cedar` already takes for the postgres target and for
// the same reason its own header states.
//
// # 🔴 What this pack cannot say, and why: the tool's name is invisible to a policy
//
// `policies/mcp/deny-etc-resources.cedar`'s own header states it first: 42 §3.4 makes an MCP delta's
// payload opaque (P-6), and the tool name (`create_or_update_file`, `issue_write`, ...) lives inside
// that payload. A Cedar statement reads `resource.locator` (`<server endpoint>#<resource URI>`,
// `crates/gx-adapter-mcp/src/locator.rs`) and never the tool a call names. So a rule of the shape the
// the task brief for req/152 asked for -- "only a catalogue tool is permitted", i.e. "permit only the tool names a (sem: SEM-gx-adapter-mcp-013)
// deployment's do/undo `Catalogue` declares" -- **cannot be written in v0.1**, not "is not written (sem: SEM-gx-adapter-mcp-014)
// yet": there is no attribute on `resource` a policy could compare a tool name against. What actually (sem: SEM-gx-adapter-mcp-015)
// gates a tool a `Catalogue` has never heard of is a different mechanism entirely, at the gate layer
// and not the policy layer -- `crates/gx-gate/src/lib.rs`'s `GateInput.invert_available`: **E-M3-4**
// makes `invert_available == false` escalate to a person (`PackExpectation::Escalate`,
// `crates/gx-gate/src/packs.rs`), universally, regardless of what any pack permits. A tool this
// catalogue does not know is therefore never silently admitted by a permissive pack; it is asked of a
// person instead. That is the real answer to "only a catalogue tool is permitted" in this architecture, and it is (sem: SEM-gx-adapter-mcp-016)
// enforced by the gate's fixed fold rather than by policy text.
//
// What a pack **can** say, and what this one says instead, is **where a call may land**: the resource.
// This target's operator scopes calls to the one throwaway repository req/152's E2E used
// (`mahirhir/glovrex-a11-throwaway`) -- the same shape `policies/mcp/deny-etc-resources.cedar` takes
// for `/etc` on the fs-shaped case, but inverted: that pack permits broadly and carves out a forbid;
// this one is **deny-default with nothing but a narrow permit**, because Cedar itself is
// default-deny ("no request is authorized ... unless there is a specific permit policy that grants
// it", https://docs.cedarpolicy.com/auth/authorization.html) and a pack that names no rule at all
// already refuses everything -- the same fact `permit-postgres.cedar`'s header states for why one
// statement is enough.
//
// # The anchor, and why it is a repository prefix rather than a single resource
//
// `resource.locator like "*#repo://mahirhir/glovrex-a11-throwaway/*"` matches on the resource half of
// the locator, anchored past the `#` for `policies/mcp/deny-etc-resources.cedar`'s own reason (a
// server endpoint that happened to spell the same text in a query string is not this resource). The
// wildcard after the repository name is deliberate: `create_or_update_file`, `delete_file` and
// `issue_write`/`create_pull_request` (req/152 §2's catalogue) each name a different resource path
// under the same repository (`repo://{owner}/{repo}/refs/heads/{branch}/contents/{path}` for file
// content, a synthetic `repo://{owner}/{repo}/issues/{n}` / `repo://{owner}/{repo}/pulls/{n}` for the
// two tools that carry no MCP resource of their own -- github-mcp-server exposes `resources/read`
// only for repository file content, `pkg/github/repository_resource.go`, GitRepo/REFERENCES.md
// 2026-08-14), so one deployment-scoping rule has to reach all of them.
//
// 🔴 This permit pins no server *endpoint* (the leading `*` before `#` admits any endpoint naming
// this repository resource) -- deliberate for a resource-scoped example: the footprint (which
// server a call may reach) is a different mechanism's responsibility (commutation/deployment
// wiring), not this pack's silence (req/38 §94, ruling 3, req/154 F4, gotcha78's family). (sem: SEM-gx-adapter-mcp-017)

@id("github-target-permit-throwaway-repo")
permit (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*#repo://mahirhir/glovrex-a11-throwaway/*"
};
