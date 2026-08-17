# Contributing

Short version: open an issue before a large change, and bring a measurement.

## Before code

This project decides things in writing before implementing them. If your change alters a
surface — a wire format, an exit code, a CLI verb — say so in an issue first. Surfaces are
frozen deliberately, and changing one is a decision rather than a patch.

## The floor

Every change must leave the test floor green. A pull request that lowers a count, skips a
suite, or narrows an assertion has to say so in its own description. Silently bounded is
the failure this project guards against hardest.

## What a good report looks like

State what you measured, under what conditions, how many runs, and what you did not look
at. "It works on my machine" is a measurement with an undeclared denominator.

## Style

Rust: `cargo fmt` and `cargo clippy -D warnings` both clean. Identifiers and public
documentation in English.

## What will not be merged

Code copied from another project, however permissive its licence. Other work is observed,
then written fresh.
