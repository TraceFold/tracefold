// The github/github-mcp-server **A2 example pack** (req/38 §92, ruling 1, req/153's E2E) -- the same (sem: SEM-gx-adapter-mcp-001)
// shape, reasons and placement as `github-target.cedar` beside it (whose header carries the full
// argument: why a target example lives in tests/fixtures/ rather than policies/, and why "only a catalogue (sem: SEM-gx-adapter-mcp-002)
// tool is permitted" is not cedar-expressible in v0.1 -- the tool's name is opaque to a policy, 42 §3.4). (sem: SEM-gx-adapter-mcp-003)
//
// One narrow permit over Cedar's own default-deny, scoped to the **A2** throwaway repository
// (`mahirhir/glovrex-a2-throwaway`, created for req/153's real-server undo E2E and archived after
// it). A separate file rather than a second statement in the A1-1 pack because each lane's E2E is
// scoped to exactly the one repository it may write (the task brief's "0 writes outside the new (sem: SEM-gx-adapter-mcp-004)
// throwaway repo" is a per-lane HARD, and a pack carrying both repos would permit the retired one again). (sem: SEM-gx-adapter-mcp-005)
//
// 🔴 This permit pins no server *endpoint* (the leading `*` before `#` admits any endpoint naming
// this repository resource) -- deliberate for a resource-scoped example: the footprint (which
// server a call may reach) is a different mechanism's responsibility (commutation/deployment
// wiring), not this pack's silence (req/38 §94, ruling 3, req/154 F4, gotcha78's family). (sem: SEM-gx-adapter-mcp-006)

@id("github-target-a2-permit-throwaway-repo")
permit (
    principal,
    action,
    resource
)
when {
    resource.substrate == "mcp"
    && resource.locator like "*#repo://mahirhir/glovrex-a2-throwaway/*"
};
