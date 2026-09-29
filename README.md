# Rust-Readability

A standalone, native Rust port of [Go-ReadabilityV2](https://github.com/markusmobius/go-readabilityV2), the core-only fork of Readeck's Go Readability v2. It extracts an article's DOM, metadata, readable HTML and plain text.

The behavioral reference is `github.com/markusmobius/go-readabilityV2`, derived from Codeberg's `v2` branch at v2.1.2, commit `b18540d99ebf105cd67122585a0a41ec299b70bc`. The fork's exact source fingerprint and dependency graph are retained separately from that upstream ancestry. General Go behavior is the target, not agreement limited to the saved examples. Deterministic differences are bugs; finite differential tests are evidence, not a proof over all inputs. See [UPSTREAM.md](UPSTREAM.md) for the reference, adaptations and verification details.

The inherited algorithm follows Mozilla Readability.js 0.6.0 plus the Go forks' improvements. Going forward, we strive to mirror Mozilla's original JavaScript Readability through the Go reference. Our philosophy is **bring your own HTML**: fetching, request modifiers, CLI/server functionality and diagnostic parser logging are outside the core library.

Version **0.6.5** requires Rust 1.98.1 and a native C toolchain to build. It adds an explicit scripting-mode option for direct parser integrations; existing parser defaults and standalone extraction are unchanged. Extraction needs no Go or Python runtime; optional `lab-profile` stage timings are compiled out normally. See [CHANGELOG.md](CHANGELOG.md).

**The library is single-threaded.** Each extraction runs on the calling thread, with no internal worker threads or thread pool. It is suitable for servers running many engines in parallel: give each engine its own parser and input DOM, and let the server control concurrency.

## Installation

```sh
cargo add rust-readability-v2@0.6.5
```

The package name is `rust-readability-v2`; the Rust import name is `rust_readability`.
Version 0.6.5 is available on [crates.io](https://crates.io/crates/rust-readability-v2/0.6.5)
and as a [GitHub source release](https://github.com/markusmobius/rust-readability/releases/tag/v0.6.5).

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

The [2026-09-29 benchmark](https://github.com/markusmobius/content-extractor-benchmark/blob/d5e8c6402430b4e8a36ff364df991ba74e3ace67/README.md#results-2026-09-29) uses 2,659 saved
development pages: 983 LegoNews, 181 ScrapingHub and 1,495 WCXB. Their F1
scores use different rules and must not be averaged. Errors are listed in
that order and remain in the denominators.

| Implementation | LegoNews F1 | ScrapingHub F1 | WCXB F1 | Errors | Extraction ms/page |
| --- | ---: | ---: | ---: | --- | ---: |
| go-readabilityV2-0.6.0 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 | 4.755 |
| rust-readability-0.6.5 | 87.82711% | 95.20557% | 78.47603% | 7 / 0 / 28 | 3.945 |

Timings are means of **all four measured passes** after one warmup, not best-of
selection. Windows 11 / Ryzen AI 7 PRO 350; Go 1.27.1 and Rust 1.98.1 GNU with
ThinLTO/mimalloc. Native extraction includes working copies, metadata and
text rendering; file I/O, startup, IPC and scoring are excluded.
Parsing is one charge per worker/page: Go 11.283 and Rust 6.386 ms, including
the separate Trafilatura noscript tree when required. Standalone Mozilla keeps
its default parser and algorithm; Trafilatura fallback, comments and pagination
are off. Scored Go/Rust Readability outputs match on all 2,659 inputs and are
unchanged from the previous release. The within-run extraction ratio is 1.21x;
this is not an isolated 0.6.4-to-0.6.5 speedup or a request-latency measurement.
All 26,590 responses were audited, with AC power and no sleep events.

On the separate unannotated application corpus, all 6,554 standalone Mozilla
outputs remain unchanged in each language, including noscript image recovery.
Readability is not Trafilatura's fallback. Trafilatura workers always use FAST
(0% external fallback); non-FAST library probes use only bundled readability-lxml
(202/6,554 final outputs, 3.082%). Neither rate describes standalone Readability.

Both application workers also retain independent standalone DomDistiller,
including pagination. Its initial removal was an integration error, corrected
without altering either library. The
[correction record](https://github.com/markusmobius/content-extractor-benchmark/blob/d5e8c6402430b4e8a36ff364df991ba74e3ace67/worker_correction_2026_09_29.json)
verifies the frozen pre-removal DomDistiller result on all 6,554 pages, with
6,169 nonempty outputs per language, complete Go/Rust equality and unchanged
other sections. The standalone results never supply Trafilatura candidates.

[FAST-suite JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_shared_performance_2026_09_29.json),
[non-FAST-suite JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/go_rust_lxml_performance_2026_09_29.json), and
[release validation](https://github.com/markusmobius/content-extractor-benchmark/blob/49c426d6135df81b7d492bea7e6aec8e6d77d80c/release_validation_2026_09_29.json)
retain separate metadata scores, output differences, exact source/build pins,
all pass totals and verification limits. Historical results use other protocols.
The immutable 0.6.5 crate retains its release-time README; these fresh tables are
repository and GitHub release-note follow-ups, not a republished archive.

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