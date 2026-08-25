import GxSpec.Core
import GxSpec.Admissible
import GxSpec.Invariant
import GxSpec.Canon
import GxSpec.Receipt
import GxSpec.Minimality
import GxSpec.Injection
import GxSpec.InjectionRng
import GxSpec.MinimalityF0
import GxSpec.EffectAlgebra
import GxSpec.Attribution

/-!
# GxSpec — root import (all modules gathered) (sem: SEM-lean-001)

Identity: the `GxSpec.lean` named in `req/spec/40-architecture/46-verification-plan.md` §2 (the
root import gathering every module). That T1-T5 are import-reachable from here is part of AC-061's
machine gate -- import-reachable from `GxSpec.lean` and type-checks. (sem: SEM-lean-002)

M8-b stage: T1-T5 (+ the non-vacuity witnesses `orderBounded_admissible`/`RecoverableChainWitness`,
the executable model `CanonModel`, after applying fix directives 1-5 of `req/38` §85's ruling)
proved with zero unproven placeholders (meets the `req/142` §1 M8-b-3 gate). (sem: SEM-lean-003)

M8-d stage: `GxSpec.Minimality` (membrane minimality = three counterexample constructions for
Rule 1's three powers, `req/142` §1 M8-d item 3) joins here -- the same gate shape, "import-reachable
from this root and type-checks". (sem: SEM-lean-004)

v0.3-c stage: `GxSpec.Injection` (Rule 2's minimality = the pair of a breakage theorem for the
clock-injection variant and a projection-recovery theorem, `DR-46-4` / `req/160` §3-1's five
conditions / `req/38` §98 ruling 3) joins here -- same gate shape. Scope is the clock instance only
(rng is a denominator, declared in that file's header doc). (sem: SEM-lean-005)

v0.4-d stage: `GxSpec.InjectionRng` (Rule 2's minimality = the pair of a breakage theorem for the
rng-injection variant and a projection-recovery theorem -- the second instance of the same
`DR-46-4` schema, `req/38` §106 ruling 1's candidate-box item / `req/174`) joins here -- same gate
shape. Both the clock and rng instances are now in place (the declaration that "Rule 2's
counterexample is complete" is Fable's acceptance call under `req/38` §NN; this root carries only
import reachability). (sem: SEM-lean-006)

U4 stage (`req/188` §2 U4, `req/38` §124 parallel lane 5, `req/191`): `GxSpec.MinimalityF0` joins
here (the F0 field-irreducibility audit: for each field of each structure of the frozen five modules,
a one-field-removed variant plus either a breakage counterexample or a survival proof; four verdicts
[NEEDED (breaks) / NEEDED (unstatable) / NEEDED (trivialised) / REDUNDANT-CANDIDATE (theory),
DESIGN-REQUIRED]; frozen modules by `import` only; axiom set unchanged) -- same gate shape. The
per-field verdict table and the denominator (fields handled by argument only, argument arities) are
owned by `req/191` §1; this root only carries import reachability.

A1 stage (`req/38` §125 ruling 1, consuming `DR-46-6` / `req/186`, reported in `req/203`):
`GxSpec.EffectAlgebra` joins here -- the first stage of the adjudicated foundation A' (effect algebra
+ lens laws + Merkle ledger + linearizable log): the three inverse-semigroup laws for a *partial*
inverse (the catalogue-declared escrow, `x*x^-1*x ~ x` / `x^-1*x*x^-1 ~ x^-1` / idempotents commute)
stated **on observations** (`ObsEq` = the pre/post pair a receipt records -- never on id or order,
which `composeId`'s opacity makes impossible and which `req/186` §3 rules out as overclaim anyway),
the GetPut/PutGet lens laws for the (effect, escrow) pair, three theorems tying the two together,
and the minimality counterexample for an undeclared tool (`invert()` returning `None`, i.e.
`InverseStatus::Unavailable`) whose single undeclared step removes a whole composable chain from the
domain of the laws -- same gate shape as above (import reachability plus type checking). Scope is
stage **A1 only**: A2 (ledger prefix monotonicity) and A3 (crash-prefix recovery) are not delivered,
and the denominators (observation-only, declared-part-only, two of `InverseStatus`'s **seven**
values -- five when this paragraph was written, six at R8, seven in DR-46-24(A)'s erratum batch;
corrected in DR-46-26's window, `req/38` §258) are owned by that file's header and `req/203`; this
root carries import reachability only.

`DR-R34-1` stage (`req/38` §257 ruling 4 item 1, unblocked by §291 ruling 5's Phase R exit, reported
in `req/523`): `GxSpec.Attribution` joins here -- the attribution invariant, i.e. whether a resume
after a crash in the commit window can separate "my own apply landed" from "a third party moved the
world". Delivered as the house pair: a breakage theorem quantified over *every* classifier of the
pre-`DR-R34-1` observation (planned fingerprint + current fingerprint), and a recovery theorem
showing one classifier of the post-`DR-R34-1` observation is sound on *every* faithful scenario --
plus the residual crash window landing fail-closed (`undetermined` if and only if the record is
missing), the catch-all-arm and rank-fold counterexample/control pairs for the append gate, and the
bridge showing that the pre/post pair `GxSpec.EffectAlgebra`'s laws are stated modulo is
reconstructible from a journal exactly when the post value is recorded. The proved-versus-not
inventory and the five denominators (no-collision hypothesis, two hypotheses not the full lifecycle,
three modelled kinds and formats, no window size, the Rust bridge is prose) are owned by that file's
header; this root carries import reachability only. (sem: SEM-lean-204) -/
