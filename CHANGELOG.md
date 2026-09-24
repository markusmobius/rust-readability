# Changelog

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