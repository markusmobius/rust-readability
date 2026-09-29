# rust-readability-v2

`rust-readability-v2` extracts article HTML, plain text and metadata from supplied
HTML. It is a native Rust port of
[go-readabilityV2](https://github.com/markusmobius/go-readabilityV2), descended
from `readeck/go-readability`, `go-shiori/go-readability` and `mozilla/readability`.

## Philosophy

Our extractor packages share three principles:

1. **Bring your own HTML.** Keep page acquisition separate from extraction.
   The primary workflow uses HTML supplied by the caller, who controls fetching,
   caching, rendering, retries and scheduling.
2. **Stay close to upstream.** Preserve the algorithms and behavior of each
   package's declared upstream reference as closely as possible. Document
   deliberate differences and compatibility limits in [UPSTREAM.md](UPSTREAM.md)
   rather than claiming exact equivalence on every page.
3. **Provide very fast Go and Rust packages.** Run extraction natively, without
   a Python or Java runtime. Improve throughput and allocation efficiency while
   preserving intended behavior, and substantiate performance with reproducible
   benchmarks that report quality alongside speed.

## Overview

The current `rust-readability-v2` release is **0.6.7**. It accepts HTML readers,
decoded strings or parsed trees and returns an owned article DOM with HTML/text
renderers and metadata getters for title, byline, excerpt, site, image, language
and dates. It has no page fetcher, CLI or HTTP server; a URL supplies context only.

The behavioral reference is `go-readabilityV2` 0.6.0, whose ancestry follows
`readeck/go-readability` v2.1.2 and `mozilla/readability` 0.6.0. Extraction runs
on the calling thread without an internal worker pool. This documentation-only
release preserves the preceding release's runtime source and dependency pins.

## Installation

```sh
cargo add rust-readability-v2@=0.6.7
```

Use Rust 1.98.1 or newer and a native build toolchain. The crate is
`rust-readability-v2`, the repository is `rust-readability`, and the Rust import
is `rust_readability`. See [Cargo.toml](Cargo.toml) for dependencies and
[CHANGELOG.md](CHANGELOG.md) for release changes.

## Usage

Extract text from HTML already held in memory:

```rust
use rust_readability::{from_reader, Url};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let page_url = Url::parse("https://example.org/research").ok_or("invalid example URL")?;
    let source = r#"<html><head><title>Research results</title></head><body><article>
<h1>Research results</h1>
<p>The research team compared several methods for extracting articles from saved
web pages. Every method received the same original HTML, and the evaluation
kept the reference text separate from the input supplied to each extractor.</p>
<p>The report records the complete experiment, including errors and repeated
measurements. Its results describe this collection of pages and do not promise
the same quality or execution time for every website.</p>
<p>The archived pages and analysis make it possible to repeat the comparison
and inspect the evidence behind each result.</p>
</article></body></html>"#;
    let article = from_reader(source.as_bytes(), Some(&page_url))?;

    println!("{}", article.title());
    println!("{}", article.text()?);
    Ok(())
}
```

| Entry Point | Input |
| --- | --- |
| `from_reader` | HTML bytes from a reader, with decoding and normalization |
| `from_html` | Already decoded HTML, without reader normalization |
| `from_document` | An existing parsed document |
| `Parser::parse_dom` | A DOM retaining original tag and namespace metadata |
| `Parser::parse_shared_document` | A caller-owned arena exposed through `DomSource` |
| `check_document` | A parsed document for a quick article-likelihood check |

Use `Article::text`, `html`, `render_text` or `render_html` for output. An article
owns its `document` and optional selected `node`; metadata getters expose title,
byline, language and dates. Complete APIs are in the
[rust-readability-v2 reference](https://docs.rs/rust-readability-v2/0.6.7/rust_readability/).

## Options

Use `Parser::with_options(Options { ..Options::default() })` for customization:

| Option | Default | Effect |
| --- | --- | --- |
| `max_elems_to_parse` | `0` | No element-count limit; positive values bound accepted trees. |
| `n_top_candidates` | `5` | Number of top candidates considered during selection. |
| `char_thresholds` | `500` | Article-length threshold used by extraction retries. |
| `keep_classes` | `false` | Remove classes except those explicitly preserved. |
| `classes_to_preserve` | `page` | Classes retained when general class preservation is off. |
| `disable_json_ld` | `false` | Use JSON-LD metadata unless disabled. |
| `allowed_video_regex` | `None` | Use the default video matcher unless overridden. |

### Parsed Input

`parse_html_into` supports a caller's `HtmlTreeSink`; `parse_html_direct` writes
into an `HtmlTreeStore`. The scripting flag in `parse_html_direct_with_scripting`
controls HTML tree construction, not JavaScript execution. Existing APIs keep
scripting enabled, as expected by this package's noscript image recovery.
Caller-owned DOM adapters must preserve ordered attributes, original tags,
namespaces and valid acyclic indices. Shared extraction preserves its input.

## Current Quality and Speed

The [2026-09-29 shared benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/ec719092d12f4d2a438dd29d9f4405aab6e0a321/README.md#results-2026-09-29)
compares the six packages below on **2,659 saved pages**: 983 LegoNews,
181 ScrapingHub and 1,495 WCXB.

### Extraction Speed

| Go Package (Measured Version) | Rust Package (Measured Version) | Go ms/page | Rust ms/page | Go/Rust |
| --- | --- | ---: | ---: | ---: |
| `go-readabilityV2` 0.6.0 | `rust-readability-v2` 0.6.5 | 4.755 | 3.945 | 1.21x |
| `go-domdistiller` 1.0.0 | `rust-domdistiller` 1.0.1 | 6.159 | 3.400 | 1.81x |
| `go-trafilatura` 2.2.6 (FAST) | `rust-trafilatura` 2.2.6 (FAST) | 11.329 | 6.570 | 1.72x |

Times are means of **all four measured passes after one warmup**. Go/Rust is
the named Go package's time divided by the named Rust package's time, not an
old/new release speedup. Later documentation-only releases do not change the
versions actually measured.

The run used Windows 11, Ryzen AI 7 PRO 350, Go 1.27.1 and Rust 1.98.1 GNU
with ThinLTO/mimalloc. Extraction includes required working copies, metadata
and text rendering. File I/O, startup, IPC, response serialization and scoring
are excluded. Comments and pagination are off; tables are on.
`go-trafilatura` and `rust-trafilatura` use FAST with external fallback disabled.
Power and sleep checks passed.

Parsing is separate: **Go 11.283 / Rust 6.386 ms/page**, charged once per
language/page for the shared suite. It includes decoding, DOM construction and
the separate `go-trafilatura` / `rust-trafilatura` noscript tree when needed.
These are extraction-stage comparisons, not complete request latencies.

### Text Quality

Each named pair has equal text scores. Errors are listed in LegoNews /
ScrapingHub / WCXB order and remain in the scoring denominators.

| Go Package | Rust Package | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | --- | ---: | ---: | ---: | --- |
| `go-readabilityV2` | `rust-readability-v2` | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| `go-domdistiller` | `rust-domdistiller` | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| `go-trafilatura` (FAST) | `rust-trafilatura` (FAST) | 90.91534% | 96.15663% | 78.51703% | 4 / 0 / 10 |

The corpora use different scoring rules; their F1 scores must not be averaged.
Equal text scores do not imply identical metadata: `go-trafilatura` and
`rust-trafilatura` differ on one title and one author field. The
[full report](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json)
contains metadata scores, differences, every pass and source/build identities.

## Compatibility and Limitations

- **Declared reference.** General `go-readabilityV2` behavior is the target;
  deterministic differences are compatibility bugs. Finite tests are evidence,
  not a proof of identical results on every possible input.
- **No browser rendering.** No page fetching, JavaScript execution or computed
  layout is provided. HTML absent from the input cannot be recovered.
- **Empty articles.** Extraction can succeed without a selected node. Text and
  HTML methods then return `Error::MissingNode`; rendering errors propagate.
- **Caller-owned concurrency.** Give concurrent extractions separate parsers
  and working trees. Parser reuse retains the reference's language-state behavior.
- **Date context.** Zone-less dates default to UTC. System local-zone behavior
  follows the Go reference; explicit local-timezone methods allow caller control.
- **Not a sanitizer.** Sanitize extracted HTML before displaying untrusted input.

See [UPSTREAM.md](UPSTREAM.md) for source pins, parser adaptations, known
boundaries, optional comparisons and historical measurements.

## Development

Use the pinned Rust toolchain and a native build toolchain:

```sh
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo package --locked
```

Checked-in Go expectations run without Go. Optional reference generation and
corpus comparisons require Python, Go 1.27.1 and the pinned `go-readabilityV2`
source; see [UPSTREAM.md](UPSTREAM.md). Package verification requires a clean
release checkout. Documentation and release rules are in [AGENTS.md](AGENTS.md).

## License and Credits

The source and adapted data retain MIT, BSD-3-Clause and ICU terms. Optional
benchmark tooling retains its Apache-2.0 notice. See [LICENSE](LICENSE),
[NOTICE](NOTICE) and [licenses](licenses); dependencies retain their own licenses.

Arc90 Inc created the original JavaScript algorithm credited by
[mozilla/readability](https://github.com/mozilla/readability), developed and
maintained by Mozilla and its contributors. Radhi Fadlillah
created [go-shiori/go-readability](https://github.com/go-shiori/go-readability),
subsequently maintained by Felipe Martin and its contributors. The Readeck
contributors developed [readeck/go-readability](https://codeberg.org/readeck/go-readability)
v2. Markus Mobius maintains `go-readabilityV2` and the `rust-readability-v2`
translation. The additional parser, reader and date adaptations retain their
original creator and copyright notices in the linked attribution files.