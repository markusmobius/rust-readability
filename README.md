# Rust-Readability

A standalone, native Rust port of [Go-ReadabilityV2](https://github.com/markusmobius/go-readabilityV2), the core-only fork of Readeck's Go Readability v2. It extracts an article's DOM, metadata, readable HTML and plain text.

The behavioral reference is `github.com/markusmobius/go-readabilityV2`, derived from Codeberg's `v2` branch at v2.1.2, commit `b18540d99ebf105cd67122585a0a41ec299b70bc`. The fork's exact source fingerprint and dependency graph are retained separately from that upstream ancestry. General Go behavior is the target, not agreement limited to the saved examples. Deterministic differences are bugs; finite differential tests are evidence, not a proof over all inputs. See [UPSTREAM.md](UPSTREAM.md) for the reference, adaptations and verification details.

The inherited algorithm follows Mozilla Readability.js 0.6.0 plus the Go forks' improvements. Going forward, we strive to mirror Mozilla's original JavaScript Readability through the Go reference. Our philosophy is **bring your own HTML**: fetching, request modifiers, CLI/server functionality and diagnostic parser logging are outside the core library.

Version **0.6.3** requires Rust 1.98.1 and a native C toolchain to build. Extraction does not require a Go or Python runtime. See [CHANGELOG.md](CHANGELOG.md) for the parser improvements in this release.

**The library is single-threaded.** Each extraction runs on the calling thread, with no internal worker threads or thread pool. It is suitable for servers running many engines in parallel: give each engine its own parser and input DOM, and let the server control concurrency.

## Installation

```sh
cargo add rust-readability-v2 --git https://github.com/markusmobius/rust-readability --tag v0.6.3
```

The package name is `rust-readability-v2`; the Rust import name is `rust_readability`.
Version 0.6.3 is a GitHub source release, not a new crates.io publication.

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

`DomSource` is a borrowed view for integration with another document arena.
`Parser::parse_shared_document` imports it once per extraction and uses the
existing copy-on-write clones for every retry. It preserves the caller's input;
it neither reparses HTML nor caches results across calls. The default import
retains ordered attributes, topology, original tags and element namespaces.
Implementations must provide consistent, acyclic node indices in `0..node_count`.
Readability's own `Dom` implements this interface using its existing shared
storage. The public DOM types and existing extraction APIs are unchanged.

## Current Quality and Speed

The [2026-09-23 benchmark JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json)
is the source for these tables. All six released engines use the same 2,659
development pages: 983 LegoNews, 181 ScrapingHub and 1,495 WCXB. The three F1
scores use different scoring rules and must not be averaged. Errors are shown
in that corpus order and remain in the denominators.

| Implementation | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors |
| --- | ---: | ---: | ---: | --- |
| go-readabilityV2-0.6.0 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| rust-readability-0.6.3 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 |
| go-domdistiller-1.0.0 | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| rust-domdistiller-1.0.1 | 86.74080% | 92.74280% | 74.39696% | 0 / 0 / 0 |
| go-trafilatura-2.2.2 | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 |
| rust-trafilatura-2.2.4 | 90.88412% | 96.15663% | 78.49352% | 3 / 0 / 10 |

| Implementation | Shared Parse ms/page | Extraction ms/page | Extraction ms/page, All Four Passes |
| --- | ---: | ---: | ---: |
| go-readabilityV2-0.6.0 | 5.638 | 2.669 | 2.705 |
| rust-readability-0.6.3 | 2.525 | 2.441 | 2.451 |
| go-domdistiller-1.0.0 | 5.638 | 3.618 | 3.628 |
| rust-domdistiller-1.0.1 | 2.525 | 1.965 | 1.973 |
| go-trafilatura-2.2.2 | 5.638 | 6.815 | 6.839 |
| rust-trafilatura-2.2.4 | 2.525 | 4.000 | 4.026 |

One full warmup precedes four measured passes. The first two timing columns
use the common best two complete passes (1 and 3), an optimistic estimate;
the final column retains the all-four mean. Go/Rust extraction ratios from
unrounded means are **1.09x Readability, 1.84x DomDistiller and 1.70x Trafilatura**.
Parsing is charged once per language/page, not once per engine. All-four parse
means are Go 5.667 and Rust 2.531 ms/page.

These Windows 11 / Ryzen AI 7 PRO 350 measurements use Go 1.27.1 and Rust
1.98.1 GNU, the released Trafilatura dependency graphs, and Rust ThinLTO/mimalloc.
Parsing includes eager decoding, normalization and DOM construction after the
file read; extraction includes private working copies, native metadata and text
rendering. Rust temporary trees are destroyed inside the timer; Go uses normal
GC, which can cross stage boundaries. File I/O, startup, IPC and scoring are
excluded. Fallbacks, comments and pagination are off; tables are on.

Every repeated scored output was stable. Go/Rust Readability and DomDistiller
match all scored outputs; Trafilatura retains two metadata-only differences.
Separate metadata scores, exact source pins and protocol limits are in
[UPSTREAM.md](UPSTREAM.md#released-suite-benchmark). These are shared-input
suite timings, not standalone end-to-end latency or an isolated parser-speedup
measurement.

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