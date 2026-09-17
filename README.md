# Rust-Readability

A standalone, native Rust port of [Go-ReadabilityV2](https://github.com/markusmobius/go-readabilityV2), the core-only fork of Readeck's Go Readability v2. It extracts an article's DOM, metadata, readable HTML and plain text.

The behavioral reference is `github.com/markusmobius/go-readabilityV2`, derived from Codeberg's `v2` branch at v2.1.2, commit `b18540d99ebf105cd67122585a0a41ec299b70bc`. The fork's exact source fingerprint and dependency graph are retained separately from that upstream ancestry. General Go behavior is the target, not agreement limited to the saved examples. Deterministic differences are bugs; finite differential tests are evidence, not a proof over all inputs. See [UPSTREAM.md](UPSTREAM.md) for the reference, adaptations and verification details.

The inherited algorithm follows Mozilla Readability.js 0.6.0 plus the Go forks' improvements. Going forward, we strive to mirror Mozilla's original JavaScript Readability through the Go reference. Our philosophy is **bring your own HTML**: fetching, request modifiers, CLI/server functionality and diagnostic parser logging are outside the core library.

Version **0.6.0** requires Rust 1.98.1 and a native C toolchain to build. Extraction does not require a Go or Python runtime.

**The library is single-threaded.** Each extraction runs on the calling thread, with no internal worker threads or thread pool. It is suitable for servers running many engines in parallel: give each engine its own parser and input DOM, and let the server control concurrency.

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
- `from_html` and `Parser::parse` accept already decoded HTML. They do not apply the reader's normalization.
- `parse_dom` returns Readability's own `Dom`, including original tag atoms and element namespaces. `Parser::parse_dom` preserves the input; `parse_dom_and_mutate` retains Go's preparation mutations. Prefer these APIs when retaining or constructing Readability DOMs.
- `parse_html`, `from_document`, `parse_document` and `parse_and_mutate` use Readability's `Document` type. `Dom::new` assigns empty element namespaces and looks up atoms from current tag names. Supply the `Dom` metadata explicitly when importing a Go node whose atom differs from its current tag.
- `check_document` provides Go's fast readability check without running extraction. It also accepts a borrowed `Dom` through dereferencing.
- `Options` controls element limits, candidate count, character threshold, preserved classes, scored tags, JSON-LD and allowed video matching. Defaults match the Go reference. The video override accepts a native `regex::Regex`; its compiled matching behavior must correspond to the Go expression being compared.

An `Article` owns its output `Dom` in `document` and its selected `node`. DOM access preserves node order and ordered attributes. As in Go, an empty extraction can succeed with no selected node; HTML and text methods then return `Error::MissingNode`. Readability's DOM types are independent of Rust-DomDistiller.

Metadata getters include title, byline, site name, excerpt, image, favicon, language and raw timestamps. `html`, `text`, `render_html` and `render_text` expose the corresponding renderers. HTML rendering preserves Go's escaping, raw-text, plaintext-termination and malformed-node errors. Writer errors propagate.

`published_time` and `modified_time` translate the referenced `itlightning/dateparse` state machine and Go time layouts. `Timestamp` exposes Unix seconds, nanoseconds, zone name and offset in seconds. Missing timestamps return `Error::TimestampMissing`; parse failures retain the field name and source error. Dates without a zone default to UTC. Local zone lookup follows the platform's Go behavior; explicit `*_with_local_timezone` methods allow a `LocalTimeZone` loaded from TZif bytes, the bundled named-zone data, UTC or a fixed offset. The system zone is initialized once. Windows ignores `TZ`, as Go does.

Parser reuse retains Go's language-state behavior. Positive element limits are checked before mutation. Node IDs, links, and metadata vectors must remain consistent when callers edit a `Dom`; it represents parsed HTML node kinds, not Go's renderer-only `RawNode` or `ErrorNode` types.

## Quality and Performance

Measured on 2026-09-16 using all **983 labeled pages** from
[content-extractor-benchmark](https://github.com/markusmobius/content-extractor-benchmark/tree/466fdbee8a504441eb78ed11d71c1da220681cab),
the same pinned corpus used for Go-DomDistiller and Rust-DomDistiller.
The Codeberg row uses the unmodified v2.1.2 source at the commit above and its
original dependencies. The Go fork and Rust rows use version 0.6.0.

### Extraction Quality

| Extractor | Precision | Recall | F1 | Accuracy |
| --- | ---: | ---: | ---: | ---: |
| Codeberg Go-Readability v2.1.2 | 0.8705 | 0.8862 | 0.8783 | 0.8774 |
| Go-ReadabilityV2 | 0.8705 | 0.8862 | 0.8783 | 0.8774 |
| Rust-Readability | 0.8705 | 0.8862 | 0.8783 | 0.8774 |

All three engines have **exactly the same quality counts**: TP 2,601, FN 334,
FP 387 and TN 2,561. The same seven empty extractions are scored as empty text,
not skipped. Scores use the benchmark's case-sensitive snippet matching and
globally aggregated counts, not token-level scoring or metadata-quality scores.

The Go fork and Rust match exactly on all 983 pages for text, HTML, title, byline,
excerpt, site name, image URL, favicon, language and errors. Codeberg's original
dependencies use x/net 0.41.0 and x/text 0.26.0; the fork uses 0.59.0 and 0.42.0.
With the original dependencies, 387 pages differ only in the HTML field; all
other compared fields and quality counts agree. A separate Codeberg control
using the fork's dependency versions matches all ten fields exactly on 983/983
pages. Thus equal quality is not a claim of byte-identical HTML across different
parser versions.

### Extraction Time

Median time per complete 983-page pass; ranges show the measured minimum and
maximum across all **102 measured passes per engine**.
Speedup is Go-ReadabilityV2's median time divided by each engine's median time.

| Extractor | Median | Range | Speedup vs Go-ReadabilityV2 |
| --- | ---: | ---: | ---: |
| Codeberg Go-Readability v2.1.2 | 2,024 ms | 1,916-2,919 ms | 0.96x |
| Go-ReadabilityV2 | 1,946 ms | 1,836-2,759 ms | 1.00x |
| Rust-Readability | 952 ms | 878-1,374 ms | 2.04x |

Rust's median speedup was **2.04x over Go-ReadabilityV2** and **2.13x over
Codeberg**.

Measured on an AMD Ryzen AI 7 PRO 350 under Linux/WSL2, pinned to one logical CPU,
with Go 1.27.1 and Rust 1.98.1 release builds. Each engine received two warmup
passes followed by 102 measured passes, cycling all six engine orders 17 times.
The dependency-aligned Codeberg control is for correctness only;
the timed Codeberg row retains its original dependency versions.

Timing includes extraction from pre-parsed DOMs, plain-text rendering and snippet
scoring. It excludes file I/O, decoding, initial HTML/URL parsing, startup, IPC
and the extra HTML/metadata collection used for exact-output checks. Timing varies
between runs: these are single-machine measurements, not a guaranteed speedup or an
end-to-end reader/network benchmark.

Raw samples, dependency graphs and source/binary fingerprints are retained in
[testdata/benchmark-results-0.6.0.json](testdata/benchmark-results-0.6.0.json).
See [UPSTREAM.md](UPSTREAM.md#shared-benchmark) for the runner and reproduction
instructions. These tables are identical to the Go fork's README tables.

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