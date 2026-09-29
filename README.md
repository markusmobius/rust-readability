# Rust-Readability

A standalone, native Rust port of [Go-ReadabilityV2](https://github.com/markusmobius/go-readabilityV2), the core-only fork of Readeck's Go Readability v2. It extracts an article's DOM, metadata, readable HTML and plain text.

The behavioral reference is `github.com/markusmobius/go-readabilityV2`, derived from Codeberg's `v2` branch at v2.1.2, commit `b18540d99ebf105cd67122585a0a41ec299b70bc`. The fork's exact source fingerprint and dependency graph are retained separately from that upstream ancestry. General Go behavior is the target, not agreement limited to the saved examples. Deterministic differences are bugs; finite differential tests are evidence, not a proof over all inputs. See [UPSTREAM.md](UPSTREAM.md) for the reference, adaptations and verification details.

The inherited algorithm follows Mozilla Readability.js 0.6.0 plus the Go forks' improvements. Going forward, we strive to mirror Mozilla's original JavaScript Readability through the Go reference. Our philosophy is **bring your own HTML**: fetching, request modifiers, CLI/server functionality and diagnostic parser logging are outside the core library.

Version **0.6.6** requires Rust 1.98.1 and a native C toolchain to build. This documentation-only patch keeps 0.6.5's runtime source and dependency pins. Existing parser defaults and standalone extraction are unchanged. Extraction needs no Go or Python runtime; optional `lab-profile` stage timings are compiled out normally. See [CHANGELOG.md](CHANGELOG.md).

**The library is single-threaded.** Each extraction runs on the calling thread, with no internal worker threads or thread pool. It is suitable for servers running many engines in parallel: give each engine its own parser and input DOM, and let the server control concurrency.

## Installation

```sh
cargo add rust-readability-v2@0.6.6
```

The package name is `rust-readability-v2`; the Rust import name is `rust_readability`.
Version 0.6.6 is available on [crates.io](https://crates.io/crates/rust-readability-v2/0.6.6)
and as a [GitHub source release](https://github.com/markusmobius/rust-readability/releases/tag/v0.6.6).

## Example

```rust
use rust_readability::{from_reader, Url};

let page_url = Url::parse("https://example.org/news/article").unwrap();
let source = format!(
    "<html lang='en'><head><title>Example article</title></head>\
     <body><article><h1>Example article</h1><p>{}</p></article></body></html>",
    "A detailed article sentence, with useful context and supporting evidence. ".repeat(15)
);
let article = from_reader(source.as_bytes(), Some(&page_url))?;

assert_eq!(article.title(), "Example article");
assert_eq!(article.language(), "en");
assert!(article.text()?.contains("supporting evidence"));
assert!(article.html()?.contains("<p>"));
# Ok::<(), rust_readability::Error>(())
```

## APIs

- `from_reader` and `Parser::parse_reader` perform Go-compatible charset detection, decoding and stream-safe Unicode normalization before parsing.
- `decode_bytes` and `parse_bytes` expose that decoding and DOM construction for bytes already in memory, allowing callers to separate file I/O from parsing.
- `from_html` and `Parser::parse` accept already decoded HTML. They do not apply the reader's normalization.
- `parse_dom` returns Readability's own `Dom`, including original tag atoms and element namespaces. `Parser::parse_dom` preserves the input; `parse_dom_and_mutate` retains Go's preparation mutations. Prefer these APIs when retaining or constructing Readability DOMs.
- `parse_html`, `from_document`, `parse_document` and `parse_and_mutate` use Readability's `Document` type. `Dom::new` assigns empty element namespaces and looks up atoms from current tag names. Supply the `Dom` metadata explicitly when importing a Go node whose atom differs from its current tag.
- `check_document` provides Go's fast readability check without running extraction. It also accepts a borrowed `Dom` through dereferencing.
- `Options` controls element limits, candidate count, character threshold, preserved classes, scored tags, JSON-LD and allowed video matching. Defaults match the Go reference. The video override accepts a native `regex::Regex`; its compiled matching behavior must correspond to the Go expression being compared.

An `Article` owns its output `Dom` in `document` and its selected `node`. DOM access preserves node order and ordered attributes. As in Go, an empty extraction can succeed with no selected node; HTML and text methods then return `Error::MissingNode`. Readability's DOM types are independent of Rust-DomDistiller.

Metadata getters include title, byline, site name, excerpt, image, favicon, language and raw timestamps. `html`, `text`, `render_html` and `render_text` expose the corresponding renderers. HTML rendering preserves Go's escaping, raw-text, plaintext-termination and malformed-node errors. Writer errors propagate.

`published_time` and `modified_time` translate the referenced `itlightning/dateparse` state machine and Go time layouts. `Timestamp` exposes Unix seconds, nanoseconds, zone name and offset in seconds. Missing timestamps return `Error::TimestampMissing`; parse failures retain the field name and source error. Dates without a zone default to UTC. Local zone lookup follows the platform's Go behavior; explicit `*_with_local_timezone` methods allow a `LocalTimeZone` loaded from TZif bytes, the bundled named-zone data, UTC or a fixed offset. The system zone is initialized once. Windows ignores `TZ`, as Go does.

Parser reuse retains Go's language-state behavior. Positive element limits are checked before mutation. Node IDs, links, and metadata vectors must remain consistent when callers edit a `Dom`; it represents parsed HTML node kinds, not Go's renderer-only `RawNode` or `ErrorNode` types.

### Parser Sink

The `parse_html_into(source, &mut sink)` API accepts an
`HtmlTreeSink` and returns its document handle. It uses the same parser and Go
node conversion as `parse_dom`, but writes into the caller's arena without
constructing a Readability `Document` or atom table. It is not a streaming HTML
tokenizer; parsing still constructs the internal HTML5 tree before emission.

`append_node` is called in document order, parents before children, with borrowed
kind/tag/namespace/data values. The sink returns its own copyable handle and
stores the parent relationship. `append_attribute` follows its node before any
children, retaining Go's namespace/key/value representation and attribute order.
Element namespaces use `""`, `"svg"`, `"math"` or the unrecognized namespace URI,
as in `Dom`. Callers must copy strings they retain. The parser sink is available
in Git releases from 0.6.1 onward.

### Shared Input

`parse_html_direct` writes directly into an `HtmlTreeStore`.
`parse_html_direct_with_scripting(source, &mut store, false)` opts into
scripting-disabled tree construction, making `noscript` contents child nodes.
All existing parser APIs keep scripting enabled. This controls HTML parsing,
not JavaScript execution. Standalone Readability should retain its default
parser so its existing noscript image recovery sees the expected raw markup.

`DomSource` is a borrowed view for integration with another document arena.
`Parser::parse_shared_document` imports it once per extraction and uses the
existing copy-on-write clones for every retry. It preserves the caller's input;
it neither reparses HTML nor caches results across calls. The default import
retains ordered attributes, topology, original tags and element namespaces.
Implementations must provide consistent, acyclic node indices in `0..node_count`.
Readability's own `Dom` implements this interface using its existing shared
storage. The public DOM types and existing extraction APIs are unchanged.

## Current Quality and Speed

The [2026-09-29 shared benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/ec719092d12f4d2a438dd29d9f4405aab6e0a321/README.md#results-2026-09-29)
compares all six implementations on **2,659 saved pages**: 983 LegoNews,
181 ScrapingHub and 1,495 WCXB. All six READMEs use this same comparison.

### Extraction Speed

| Extractor | Go Version | Rust Version | Go ms/page | Rust ms/page | Go/Rust |
| --- | --- | --- | ---: | ---: | ---: |
| Readability | 0.6.0 | 0.6.5 | 4.755 | 3.945 | 1.21x |
| DomDistiller | 1.0.0 | 1.0.1 | 6.159 | 3.400 | 1.81x |
| Trafilatura FAST | 2.2.6 | 2.2.6 | 11.329 | 6.570 | 1.72x |

Times are means of **all four measured passes after one warmup**. Go/Rust is
Go time divided by Rust time, not an old/new release speedup. Measured versions
are shown explicitly; later documentation-only releases are not new measurements.

The run used Windows 11, Ryzen AI 7 PRO 350, Go 1.27.1 and Rust 1.98.1 GNU
with ThinLTO/mimalloc. Extraction includes required working copies, metadata
and text rendering. File I/O, startup, IPC, response serialization and scoring
are excluded. Comments, pagination and Trafilatura external fallback are off;
tables are on. Power and sleep checks passed.

Parsing is separate: **Go 11.283 / Rust 6.386 ms/page**, charged once per
language/page for the shared suite. It includes decoding, DOM construction and
the separate Trafilatura noscript tree when needed. These are extraction-stage
comparisons, not complete request latencies.

### Text Quality

Go and Rust have the same text scores for each engine. Errors are listed in
LegoNews / ScrapingHub / WCXB order and remain in the scoring denominators.

| Extractor | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | ---: | ---: | ---: | --- |
| Readability | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| DomDistiller | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| Trafilatura FAST | 90.91534% | 96.15663% | 78.51703% | 4 / 0 / 10 |

The corpora use different scoring rules; their F1 scores must not be averaged.
Equal text scores do not imply identical metadata: Trafilatura differs on one
title and one author field. The [full report](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json)
contains metadata scores, differences, every pass and source/build identities.

## Verification

```text
cargo fmt --all -- --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --release
cargo package --locked --allow-dirty
```

The checked-in independent Go expectations run without Go. To regenerate the reference in isolation or run the larger comparisons, install Python and Go and provide the fingerprinted Go fork checkout as the sibling `go-readabilityV2` directory (or pass `--source /path/to/go-readabilityV2`). The generator selects Go 1.27.1 and checks module checksums and source identity:

```text
python tools/go_reference.py
python tools/go_reference.py --corpus
python tools/go_reference.py --html-corpus
python tools/go_reference.py --native-timezone
cargo test --locked --release -- --include-ignored
```

The first command verifies exact artifact bytes. Only `--write` changes portable expectations. Corpus and platform-specific outputs go under ignored `target/`; they are not packaged. `RUST_READABILITY_CASE` selects one exact corpus case ID for diagnosis. Reader-source receipts can be verified with `--reader-archive` and the published Rust-DomDistiller 1.0.0 crate archive.

## License

The library source and data use MIT, BSD-3-Clause and ICU terms. The optional benchmark tooling also retains the upstream benchmark's Apache-2.0 notice. See [LICENSE](LICENSE), [NOTICE](NOTICE) and the retained [licenses](licenses). Dependencies retain their own licenses.