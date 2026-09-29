# Rust-Readability Maintenance

Read these instructions before changing README.md, UPSTREAM.md, CHANGELOG.md,
benchmark claims or releases. Inspect current files and git status; preserve
unrelated edits and follow explicit user constraints.

## Library Identity

The repository is rust-readability; the crate is rust-readability-v2 and the
Rust import name is rust_readability. Preserve these distinct names in examples
and publication checks. The behavioral reference is Go-ReadabilityV2, derived
from Readeck/Mozilla Readability. It is not Trafilatura's bundled readability-lxml.
Standalone extraction and its default noscript image recovery remain independent
of Trafilatura's parser/fallback choices. The library does not fetch pages or
schedule background extraction. Tests are evidence, not universal parity proof.

## Document Roles

- README.md: readable purpose, scope, usage and current quality/speed. Keep
  examples useful; avoid application-worker stories, source hashes and audit logs.
- UPSTREAM.md: ancestry, exact source/dependency pins, compatibility boundaries,
  reproductions, hashes, coverage and remaining differences.
- CHANGELOG.md: dated/versioned user-visible changes and their reasons. Explicitly
  label documentation-only releases; keep historical entries historical.
- Release notes: the same changes, benchmark definitions and limitations as the
  README/changelog, with links to detailed evidence instead of copied logs.
- AGENTS.md: durable instructions, not current results or work-in-progress.

## Six-Repository Benchmark Contract

Follow the complete
[benchmark maintenance guide](https://github.com/markusmobius/content-extractor-benchmark/blob/master/AGENTS.md).
Coordinate go-domdistiller, rust-domdistiller, go-readabilityV2, rust-readability,
go-trafilatura and rust-trafilatura, not only this library's language pair.

1. Every README has `## Current Quality and Speed` with the same six-engine
	comparison from one completed shared-suite report. Use measured version labels.
2. Columns: `Extractor`, `Go Version`, `Rust Version`, `Go ms/page`,
	`Rust ms/page`, `Go/Rust`. Rows: Readability, DomDistiller, Trafilatura FAST.
	Display milliseconds/page to three decimals and Go/Rust ratios to two decimals.
3. Read structured JSON and compute ratios from unrounded means. The current
	shared protocol uses all four measured passes after one warmup. Never mix
	dates, environments, modes, means/medians or selected/all-pass aggregates.
4. Report parsing separately, charged once per language/page. Include decoding,
	normalization, DOM construction and any separately required Trafilatura tree.
	State corpus/counts, options, hardware, toolchains and included/excluded work.
5. Keep non-FAST Trafilatura as a separate comparison with its own report. A
	paired language ratio is not an isolated version speedup or request latency.
6. Report named-corpus F1 percentages to five decimals, retaining errors in the
	denominators. Do not average different scoring definitions. Matching text
	scores do not prove byte-identical HTML or metadata; preserve known differences.
7. Link immutable source reports and commits. Keep historical results clearly
	dated and old artifacts unchanged. Older 102-pass standalone measurements
	are not directly comparable to the newer shared-input timing boundary.
8. Documentation-only patches retain actual measured versions; do not relabel
	old measurements or rerun benchmarks just to change wording. Do not insert
	Trafilatura worker incidents or its fallback rates as Readability results.

## Editing and Crate Publication

1. Identify the scope/evidence before editing. Documentation does not authorize
	runtime, parser or dependency changes. Coordinate the three document roles
	and all six common benchmark sections; use plain library-facing prose.
2. Check numbers, version labels, API names and links. Compare common sections
	and run `git diff --check`. Run appropriate existing format/doc-test/test/lint
	gates on the pinned toolchain and report optional/unrun coverage accurately.
3. Obtain authorization before commits, pushes, versions, tags or publication.
	Never move a published tag or replace an immutable crate archive.
4. Finalize README, UPSTREAM, CHANGELOG and AGENTS before packaging. A
	crates.io README update requires a new version; changing GitHub release text
	cannot update the registry archive. Include intended documents in Cargo's
	explicit package file list and verify that the README is actually packaged.
5. For documentation-only patches, change package identity/its lockfile entry
	and current installation links without changing runtime source or dependency
	pins. Keep benchmark rows labeled with the versions that were timed.
6. Inspect the clean-commit crate archive, excluding temporary data/secrets.
	Verify runtime sources against the preceding published archive and verify
	intended docs against the new commit. Check package ownership/version
	availability before publishing; a similarly named repository is not ownership.
7. Verify the published checksum, source/doc bytes, normal registry installation
	and GitHub release page. A source tag is not crates.io publication.
8. Hash exact Git/published bytes, not assumed Windows checkout bytes. Preserve
	historical artifacts and unrelated edits. Never print credentials or claim
	hosted CI success without observing it.
9. Report what was checked and published, remaining local work and limitations.
	Keep detailed evidence in UPSTREAM/reports rather than the README/changelog.