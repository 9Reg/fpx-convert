# Specs

This is spec-driven development: we write down what fpx-convert should do and why *before* writing the Rust code for it.

Specs should describe behavior and intent, not Rust implementation details, so they'd still make sense as a reference if this were ever reimplemented in another language.

## Conventions

- Files are named `NNNN-short-slug.md` (zero-padded, sequential, e.g. `0001-fpx-conversion-pipeline.md`).
- Each spec states its `Status` up top (`Draft`, `Accepted`, `Superseded by NNNN`, ...).
- When a spec makes a judgment call rather than following an explicit instruction, it says so in the text (a "why", and a note that it's flagged for review) rather than deciding silently.
