// The github/github-mcp-server **A-3 example pack** (req/38 §98, ruling 2, product settle (sem: SEM-gx-adapter-mcp-010)
// pre-flight; req/163's E2E) -- the same shape, reasons and placement as
// `github-target-a2.cedar` / `github-target-a4.cedar` beside it (whose headers carry the
// standing arguments: why a target example lives in tests/fixtures/ rather than policies/, and
// why "only a catalogue tool is permitted" is not cedar-expressible -- the tool's name is opaque (sem: SEM-gx-adapter-mcp-011)
// to a policy, 42 §3.4).
//
// One narrow permit over Cedar's own default-deny, scoped to the **A-3** throwaway repository
// (`mahirhir/glovrex-a5-throwaway`, created for req/163's write-then-immediately-undo E2E and
// archived after it). A separate file rather than a second statement in an earlier pack for the
// same per-lane reason those packs give: each lane's E2E is scoped to exactly the one repository
// it may write, and a pack carrying a retired repo would permit it again.
//
// 🔴 This permit pins no server *endpoint* (the leading `*` before `#` admits any endpoint naming
// this repository resource) -- deliberate for a resource-scoped example: the footprint (which
// server a call may reach) is a different mechanism's responsibility (commutation/deployment
// wiring), not this pack's silence (req/38 §94, ruling 3, req/154 F4, gotcha78's family). (sem: SEM-gx-adapter-mcp-012)

@id("github-target-a5-permit-throwaway-repo")
permit (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*#repo://mahirhir/glovrex-a5-throwaway/*"
};
