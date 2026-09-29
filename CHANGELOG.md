# Changelog

## 0.6.6 - 2026-09-29

- Documentation-only release; runtime source and dependency pins are unchanged
  from 0.6.5.
- Use the same September 29 six-engine speed and quality comparison in all six
  library READMEs, with consistent units, measured versions and timing boundaries.
- Include AGENTS.md with instructions for keeping README, UPSTREAM, CHANGELOG,
  release notes and crate documentation consistent.
- Package the revised documentation on crates.io. Benchmark rows retain the
  versions actually measured; this release introduces no new measurements.

## 0.6.5 - 2026-09-29

- Add `parse_html_direct_with_scripting` for callers that need scripting-disabled
  HTML tree construction, including parsed `noscript` children.
- Keep every existing parsing API scripting-enabled and preserve standalone
  Mozilla/Go-ReadabilityV2 extraction, metadata, and noscript image recovery.
- This is an additive parser API release, not a new extraction algorithm or a
  standalone readability-lxml implementation.
- Pass all 60 release tests, including 133 saved extraction pages and 1,793 HTML
  cases, plus strict Clippy and package checks. Verify unchanged standalone
  Mozilla outputs on all 6,554 application inputs in each language.
- Record [fresh released-suite measurements](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/README.md#results-2026-09-29):
  F1 remains 87.82711% / 95.20557% / 78.47603% on LegoNews / ScrapingHub / WCXB.
  Go/Rust extraction is 4.755 / 3.945 ms/page across all four measured passes.
  This is a within-run language comparison, not an isolated version speedup.
  Trafilatura's 0% FAST / 3.082% non-FAST external fallback is independent of
  standalone Readability. Fresh docs follow publication; crate/tag are unchanged.

## 0.6.4 - 2026-09-28

- Prepare the input once per extraction and clone the prepared state for retry
  passes, preserving caller input and existing retry behavior.
- Cache ordinary four-decimal candidate scores using exact integer rounding;
  large and nonfinite values retain the original formatter/parser path.
- Preserve canonical score text, ties-to-even, signed zero, mutation invalidation
  and cloned-state behavior. Regression tests include 200,000 score comparisons.
- Add opt-in `lab-profile` stage timings; normal builds contain no profiling work.
- These are the retained Readability improvements from the rustHTML lab. The
  earlier 1.196x worker result used a different fallback policy and is not a
  speed claim for this release; fresh released-suite measurements follow in the
  [shared benchmark](https://github.com/markusmobius/content-extractor-benchmark).

## crates.io Publication - 2026-09-23

- Publish `rust-readability-v2` 0.6.3 on crates.io with the same runtime sources
  as the existing GitHub release. Update registry installation instructions;
  the original release tag and benchmark results are unchanged.

## Documentation - 2026-09-23

- Refresh README quality and six-engine speed comparisons from the published
  [benchmark JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json),
  with separate metadata scores and exact provenance in UPSTREAM.
- Readability text F1 remains 87.82711% / 95.20557% / 78.47603% on LegoNews /
  ScrapingHub / WCXB. Selected Go/Rust extraction is 2.669 / 2.441 ms/page
  (1.09x); all-four means are 2.705 / 2.451 ms/page. Shared parsing is separate.
- This is a documentation follow-up to 0.6.3, not another release or a moved tag.

## 0.6.3 - 2026-09-23

- Accelerate ordinary lowercase ASCII tag and attribute names with bulk copying,
  and common raw-text/attribute delimiter scans with jetscii.
- Integrate a private html5ever 0.39.0 tokenizer while retaining the released
  tree builder, token interfaces, entity data and exceptional-input behavior.
  Decoding, normalization, DOM finalization and cleanup remain eager; no work
  is deferred into extraction and no extraction rules or public APIs change.
- Retain upstream license notices and document the import in [UPSTREAM.md](UPSTREAM.md).
- Verify all 58 release library tests, including the 1,793 Go HTML cases and
  133 saved extraction pages, tokenizer differential tests, strict Clippy,
  documentation and local package compilation on Windows/GNU with Rust 1.98.1.
  The integrated three-engine suite also matched all 2,659 development-page
  DOMs and benchmark outputs. These checks do not claim new platform coverage.

Version 0.6.3 is available as both a GitHub source release and a crates.io package.