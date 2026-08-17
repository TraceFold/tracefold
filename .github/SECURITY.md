# Security

## Reporting

Report a vulnerability privately through GitHub's
[security advisory form](https://github.com/TraceFold/tracefold/security/advisories/new).
Do not open a public issue for a vulnerability.

We aim to acknowledge within 72 hours. This is a small project; that is a target, not a
contract.

## Scope

Tracefold is pre-release, and its threat model is narrow and written down. Several classes
of attack are **out of scope by declaration, not by oversight**:

- An actor with root or kernel privilege writing around the tool.
- An actor holding write access to the tool's own state directory. A local detector cannot
  answer that case; the defence is an artifact held outside the machine.
- A policy that faithfully enforces the wrong intent. The tool checks that a change
  satisfies the rule, not that the rule was the right one to write.

A report that lands inside a declared limit is still worth sending — it tells us the
declaration is not clear enough — but it will be closed as documented rather than fixed.

## What we ask

Bring a reproduction. A claim we cannot check is a claim we cannot act on.
