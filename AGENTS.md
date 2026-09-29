# rust-readability-v2 Agent Instructions

Read these instructions before changing README.md, UPSTREAM.md, CHANGELOG.md,
benchmark claims or releases. Inspect current files and git status; preserve
unrelated edits and follow explicit user constraints.

## Library Identity

The repository is `rust-readability`; the crate is `rust-readability-v2` and the
Rust import name is `rust_readability`. Preserve these distinct names in examples
and publication checks. The behavioral reference is `go-readabilityV2`, derived
from `readeck/go-readability`, `go-shiori/go-readability` and `mozilla/readability`.
Preserve the package's own parser defaults and noscript image recovery. The
library does not fetch pages or schedule background extraction. Tests are
evidence, not universal parity proof. Keep other packages' implementation and
integration policies out of this guide.

## Required Creator Acknowledgments

The README's License and Credits section must explicitly name:

- Arc90 Inc, whose original JavaScript algorithm is credited in the
	`mozilla/readability` 0.6.0 source header.
- Mozilla and its contributors, creators and maintainers of `mozilla/readability`.
- Radhi Fadlillah, original author of `go-shiori/go-readability` and the
	copyright holder named in LICENSE.
- Felipe Martin and the `go-shiori/go-readability` contributors, maintainers
	of that Go port.
- The Readeck contributors, responsible for the `readeck/go-readability` v2 fork.
- Markus Mobius, maintainer of `go-readabilityV2` and the `rust-readability-v2`
	translation; do not replace upstream credits with the current maintainer.

Verify the reference chain in UPSTREAM.md and preserve LICENSE, NOTICE and
licenses/. Additional Go Authors, html5ever, dateparse, chardet and IBM/ICU
attributions remain governed by their original retained notices. The combined
MIT, BSD-3-Clause and ICU terms are not a blanket MIT license. Do not imply
endorsement by the original creators.

## Document Roles

- README.md: purpose, scope, usage and concise current quality/speed. Preserve
  useful API/examples and clearly label historical comparisons and limitations.
- UPSTREAM.md: source/dependency pins, deliberate deviations, controlled evidence,
  reproduction commands, hashes, coverage and unresolved compatibility issues.
- CHANGELOG.md: dated/versioned user-visible changes and reasons, not worker
  incidents or an audit transcript. Label documentation-only changes explicitly.
- Release notes: the same release-facing changes, benchmark definitions and
  limitations as the README/changelog, with links to detailed evidence.
- AGENTS.md: durable instructions, not current results or work-in-progress.

## Package Naming

Always identify the implementation by its full package name, including in
titles, headings, prose, tables, captions, changelogs and release notes:

- `go-domdistiller`
- `rust-domdistiller`
- `go-readabilityV2`
- `rust-readability-v2` (repository: `rust-readability`; Rust import: `rust_readability`)
- `go-trafilatura` (Go module: `github.com/markusmobius/go-trafilatura/v2`)
- `rust-trafilatura`

Never replace a package identifier with a bare algorithm name or a generic
language/algorithm label. Do not drop the language prefix or the versioned
package suffix. Give measured versions beside package names in benchmarks.
When discussing upstream projects, use their owner-qualified repository names,
not names that could be mistaken for one of these packages. Keep actual code identifiers
and import aliases unchanged; naming prose precisely is not an API rename.

## README Format

The rules below define the approved structure and required content for all six
library READMEs. Each repository's README demonstrates these rules; it is not a
substitute for this specification. Keep package-specific guidance local to its
own repository. The benchmark repository retains its methodology/results layout.

Consistency means the whole README, not just an identical benchmark table.
Use these exact level-two headings in this exact order. Every point in the
Required Content column is mandatory, not a suggestion:

| Order | Heading | Required Content |
| --- | --- | --- |
| 1 | `## Philosophy` | State all three principles in order: bring your own HTML; stay as close as possible to upstream; provide very fast native Go and Rust packages. Explain each using the required points below. |
| 2 | `## Overview` | Inputs, outputs, scope, current release and source/reference relationship. Identify which entry points fetch pages, if any. |
| 3 | `## Installation` | One command pinned to the current published version. Explain package/import names where they differ and relevant compiler requirements. Never recommend `@main`, `-u` or an unpinned branch as the default. |
| 4 | `## Usage` | One complete runnable example using local HTML, without network access or external fixtures. Show useful output. Follow with a short entry-point/result summary; link generated API docs instead of copying full type definitions. |
| 5 | `## Options` | A compact table of this package's important controls, their actual defaults and effects. Put explanations of its own behavior under level-three headings here. |
| 6 | `## Current Quality and Speed` | The shared six-package comparison with `### Extraction Speed` and `### Text Quality`. Use full package names, measured versions, common units, timing boundaries and evidence links. |
| 7 | `## Compatibility and Limitations` | Current behavioral target, important omissions, input/output safety and known differences. Link UPSTREAM for evidence rather than copying investigation logs. |
| 8 | `## Development` | Existing commands for relevant tests and checks, with prerequisites. Listing commands does not mean they were run successfully. |
| 9 | `## License and Credits` | Name the original upstream creators and the relevant port authors explicitly, using this repository's verified attribution. Link actual licenses/notices and upstream projects; links alone do not replace creator acknowledgments. Do not reduce multiple inherited licenses to a blanket MIT claim. |

### Required Philosophy Points

1. **Bring your own HTML.** Present supplied HTML as the primary workflow.
	The caller controls fetching, caching, rendering, retries and scheduling;
	extraction is a separate concern. Do not claim existing URL convenience
	helpers are absent: identify them in Overview without making them the default
	installation example or integration path.
2. **Stay close to upstream.** Preserve the declared upstream algorithms and
	behavior as closely as possible rather than inventing a separate extractor.
	Name the actual reference in Overview, including when a Rust package follows
	its Go counterpart. Document deliberate differences and compatibility limits
	in UPSTREAM; do not turn this principle into a claim of universal parity.
3. **Provide very fast Go and Rust packages.** State native execution and high
	throughput as design goals. Improve performance without silently changing the
	intended extraction behavior. Support concrete speed claims with reproducible
	benchmarks and report quality alongside speed; do not claim every package is
	always faster than its reference or that Rust is always faster than Go.

Before those sections, use `# <Full Package Name>` and one short purpose
paragraph. Keep section headings language-neutral; real API and language
differences belong in their contents. Do not force identical functionality
onto different packages or retain a competing legacy outline below the new one.

Keep historical results in dated technical records linked from the current
comparison. Do not retain old ranking essays, old timing tables, project origin
stories or full copied API structs as extra README sections. Preserve evidence
in UPSTREAM or immutable reports rather than deleting or relabeling it. Claims
such as "fastest", "best accuracy", "stable enough" or "minimal effects" require
specific current evidence and stated limits, not an old anecdote or citation.

Before finalizing README changes:

1. Check the entire level-two heading sequence against the table above, not
	just the benchmark section. Check every required content point, including
	all three philosophy principles in their specified order.
2. Verify installation, API names and option defaults against the actual
	release. Run the exact example against that version without network input.
3. Check benchmark arithmetic and labels against the saved structured report.
	Keep the measured versions when a later release changes documentation only.
4. Check every package reference and local link throughout the draft. Remove
	stale recommendations and unsupported rankings, not merely stale tables.
5. Preserve this approved structure and its named creator acknowledgments.
	Obtain approval before redesigning it; passing checks or previous release
	authorization is not approval of a new README design.

README.md is included as crate-level Rust documentation. Even for README-only
changes, run `cargo clippy --locked --all-targets -- -D warnings` as well as
formatting and doctests before pushing or publishing. Passing doctests does not
check Clippy's Markdown lints. Use three spaces for numbered-list continuation
lines and two for bullet-list continuation lines, not tabs; do not suppress the
documentation lints to make CI pass.

## Six-Repository Benchmark Contract

Follow the complete
[benchmark maintenance guide](https://github.com/markusmobius/content-extractor-benchmark/blob/master/AGENTS.md).
Coordinate go-domdistiller, rust-domdistiller, go-readabilityV2, rust-readability,
go-trafilatura and rust-trafilatura, not only this library's language pair.

1. Every README has `## Current Quality and Speed` with the same six-package
	comparison from one completed shared-suite report. Use measured version labels.
2. Speed columns: `Go Package (Measured Version)`, `Rust Package (Measured Version)`,
	`Go ms/page`, `Rust ms/page`, `Go/Rust`. Pair `go-readabilityV2` with
	`rust-readability-v2`, `go-domdistiller` with `rust-domdistiller`, and
	`go-trafilatura` with `rust-trafilatura`, in that order. Mark FAST on both
	packages in the last pair. Display milliseconds/page to three decimals and
	ratios to two decimals. Quality rows also name both packages explicitly.
3. Read structured JSON and compute ratios from unrounded means. The current
	shared protocol uses all four measured passes after one warmup. Never mix
	dates, environments, modes, means/medians or selected/all-pass aggregates.
4. Report parsing separately, charged once per language/page. Include decoding,
	normalization, DOM construction and any additional parse trees recorded in
	the shared benchmark's timing boundary.
	State corpus/counts, options, hardware, toolchains and included/excluded work.
	`go-domdistiller` and `rust-domdistiller` pagination must be identified as
	on/off and by algorithm.
5. Keep separate measurement runs and configurations separate. A paired
	language ratio is not an isolated version speedup or request latency.
6. Report named-corpus F1 percentages to five decimals, retaining errors in the
	denominators. Do not average different scoring definitions. Matching text
	scores do not prove byte-identical HTML or metadata; preserve known differences.
7. Link immutable reports/commits and preserve old artifacts unchanged. Clearly
	date old standalone timing/allocation tables; do not compare them directly
	to the current shared-input extraction boundary.
8. Documentation-only patches retain actual measured versions; do not relabel
	old measurements or rerun benchmarks just to change wording.

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