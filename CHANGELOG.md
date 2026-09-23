# Changelog

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

This is a GitHub source release; the crates.io version remains 0.6.0.