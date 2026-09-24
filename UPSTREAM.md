# Upstream Reference

## Released Suite Benchmark

The [2026-09-23 JSON](https://github.com/markusmobius/content-extractor-benchmark/blob/d433ab637f0a56c0926aa3698f470794a553472f/go_rust_shared_performance_2026_09_23.json)
is authoritative for the current [README tables](README.md#current-quality-and-speed).
Its SHA-256 is `24db96d7858adea1f345e7e3096e7a62bc24fa2f218a3f19f85d1fecb727e4b9`.
Read text scores at `quality[worker][engine].evaluations[corpus].overall.f1`,
selected timings at `overall`, and all-four timings at `all_passes`.

Rust pins are Readability 0.6.3 (`52ec5ae744fb132e011ad9153ad3071e1227bdeb`),
DomDistiller 1.0.1 (`e95bff0cea7f7b9639abe04a8531b220b3ee4a6e`) and Trafilatura
2.2.4 (`fd57552f181c59fbb0b232250529ef68e967181b`). Go stays at Readability
0.6.0, DomDistiller 1.0.0 and Trafilatura 2.2.2; full commits and unchanged
dependency graphs are in the embedded build receipts. Rust-Trafilatura remains
private; reproducing that suite requires authorized access.

| Implementation | Author Sets Exact / 1,290 | Author-Unit F1 | Titles Exact / 2,364 | Dates Exact / 1,530 |
| --- | ---: | ---: | ---: | ---: |
| go-readabilityV2-0.6.0 | 640 | 56.38767% | 1,247 | 763 |
| rust-readability-0.6.3 | 640 | 56.38767% | 1,247 | 763 |
| go-domdistiller-1.0.0 | 0 | 0.00000% | 1,106 | 0 |
| rust-domdistiller-1.0.1 | 0 | 0.00000% | 1,106 | 0 |
| go-trafilatura-2.2.2 | 695 | 58.80923% | 1,228 | 1,227 |
| rust-trafilatura-2.2.4 | 696 | 58.86640% | 1,227 | 1,227 |

Metadata uses only nonempty supplied annotations; unannotated is not negative,
and missing output is not filled by another engine. All six scored-output
digests match the preceding September 22 report. Trafilatura's two Go/Rust
differences concern one title and one author, not extracted text.

The full 2,659-page development run used seed 20260922, one warmup and four
measured passes. Passes 1 and 3 were selected by combined extraction time for
every row (5,318 observations each); all-four means retain 10,636 observations.
Worker order is balanced per page; three-engine order is a partial six-pass
block. Go uses `GOMAXPROCS=1`, `GOGC=100`, without forced collection. Native
timers exclude file reads and IPC; parsing and extraction stay separate.
The 26,590-response audit passed with no recorded sleep and AC power throughout.
This compares released suites, not isolated parser changes or unseen holdout
quality. It does not establish extraction-time neutrality versus older Rust.
Historical standalone results and independent oracle fixtures below are unchanged.

## Source and Dependency Graph

The authoritative extraction implementation is the core-only Go-ReadabilityV2 fork, derived from Readeck's v2 branch at v2.1.2, commit `b18540d99ebf105cd67122585a0a41ec299b70bc`. The upstream branch head and tag matched when imported. This is a native port, not a wrapper around a Go command, Mozilla Readability or another Rust extractor.

- Fork module: `github.com/markusmobius/go-readabilityV2`, version 0.6.0, without a `/v2` suffix.
- Rust package: `rust-readability-v2`, Git release 0.6.3 and crates.io release 0.6.0, with library import `rust_readability`. The registry package name differs from the repository because `rust_readability` is already owned by another maintainer; crates.io treats hyphens and underscores as equivalent for name uniqueness.
- Fork source tree SHA-256: `7b4ab06ed130e3ff5778dfd69dedc87ce12f63e8c8521e0789e855b19070b064`; per-file hashes and normalization rules are in [testdata/go-source.json](testdata/go-source.json). The content digest identifies the exact core source independently of Git history.
- Original upstream module: `codeberg.org/readeck/go-readability/v2@v2.1.2`.
- Original upstream module checksum: `h1:JBrdyYJBRPMBbodLM1b5KxCSDH+JqCkGcuVRD7ICBAw=`.
- Original upstream manifest checksum: `h1:Ut31sW4osSrJPR3T8eQslMh4+jbwimXqn0w0ReCT+PU=`.
- Oracle toolchain: Go 1.27.1, with portable timestamp comparisons explicitly setting `time.Local = time.UTC`.
- Integration dependencies: `golang.org/x/net v0.59.0`, `golang.org/x/text v0.42.0`, `github.com/go-shiori/dom v0.0.0-20230515143342-73569d674e1c`, and `github.com/itlightning/dateparse v0.2.1`.

[tools/go_reference.py](tools/go_reference.py) fingerprints the supplied fork checkout, copies its core sources and test resources into a temporary main module, and overlays [tools/go-reference/export_test.go](tools/go-reference/export_test.go). It never edits the checkout or module cache. The selected graph is retained in [testdata/go-modules.json](testdata/go-modules.json); actually compiled module versions are checked separately. Replacements and unapproved version changes fail verification. The larger selected graph includes modules that do not compile into the library oracle. The default source is the sibling `go-readabilityV2` checkout; use `--source` for another location.

The fork removes URL fetching/request modifiers, the CLI/server, network helper scripts and diagnostic logging. Reader/DOM extraction, parser options, metadata, timestamps and rendering remain the compatibility target. Regenerating all five regex sources with upstream re2go 4.2 produced byte-identical Go files.

## Translation

The port follows the extraction state machine, all retry flags, scoring and tie ordering, metadata precedence, cleanup, relative-URI handling, reader behavior and result methods. Go's pdqsort and 64-bit pattern-breaking PRNG are translated locally because unstable tie order can change selected candidates. Reader byte lengths, rune counts, signed/wrapping integer behavior and the original-atom versus retagged-name distinction are retained where the Go algorithm uses them.

Readability owns its `Document`, `Dom` and node types; there is no runtime dependency on Rust-DomDistiller. An article owns its result DOM. Clones share immutable strings, node payloads and child lists with copy-on-write mutation, while keeping topology and lazy caches independent. Go-Shiori clones retain atoms and attributes but clear element namespaces; Readability's clone paths reproduce that behavior. `Document` imports cannot recover absent namespaces or original atoms; the metadata-aware `Dom` APIs preserve and accept that information explicitly. Rust ownership and compiled-regex interfaces replace their language-specific Go counterparts. Renderer-only non-HTML node kinds are not represented by the native arena API; this is an API-shape difference, not an exception to extraction comparisons.

Extraction is synchronous and single-threaded. Each server engine owns its parser and input DOM; the library does not schedule work or create a thread pool. Per-node `Cell` caches store exact text statistics and the float bits parsed from canonical score attributes. Cloning starts those caches empty, and mutation invalidates them, so extraction does not warm caches in the caller's DOM. Packed text statistics fall back to exact computation when a value does not fit; this is not an input limit. Score writes retain Go's four-decimal formatting and reparsing, including signed zero, infinities and malformed attributes. Batched tag queries and removals preserve the original tag-group, descendant and detached-node processing order. Focused tests compare these operations against the scalar or sequential implementations.

Read-only DOM parsing consumes its private prepared clone for the first extraction attempt. A failed attempt reconstructs the same preparation from the unchanged input, retaining the original retry flags, atoms, cleared namespaces and node layout. Metadata is extracted once. Mutating DOM APIs still retain their preparation mutations, including during unwinding. Forced-retry tests compare the complete result DOM and metadata against the mutating path. No scoring, retry, cleanup or page-selection shortcuts are used.

The HTML adapter uses pinned html5ever 0.39.0 and markup5ever_rcdom 0.39.0. The older parser used by DomDistiller remains unchanged. The adapter preserves Go's formatting-element attribute order, duplicate-attribute handling, namespace adjustments, exact atom dictionary and template-content layout. Comments use Go's entity decoding. Raw leading doctypes are located with html5gum's tokenizer and interpreted in Go's order, including empty identifiers, decoded quotes and quirks decisions. A token/tree sink enforces Go's foreign-content/template termination rule during construction. Its trace hook detects foreign nodes retained by the builder; foreign nodes do not enter HTML's active-formatting list.

The private [tokenizer](src/html/tokenizer/mod.rs) is adapted from html5ever
0.39.0 at commit `ce64836c685025a5fef0860fa2e9c80b2683e8d0`, specifically
`html5ever/src/tokenizer/mod.rs`, `html5ever/src/tokenizer/char_ref/mod.rs`
and `html5ever/src/macros.rs`. It retains the released token types, entity
tables, states and tree builder. Local changes bulk-copy ordinary lowercase
ASCII tag and attribute names and use jetscii for delimiter scanning in raw
text and attribute values. Exceptional characters retain the original state
transitions and preprocessing. The decoder, normalization, final DOM and
parser cleanup remain eager; no work is deferred into extraction. No global
Cargo dependency patch or benchmark-control switch is required. The optional
`trace_tokenizer` feature retains upstream tracing support.

The new fast paths use safe buffer APIs. Unsafe-code exceptions are restricted
to the unchanged upstream SSE2/NEON data-state routines and their
CPU-feature-checked call site; the rest of the tokenizer remains covered by
the crate's unsafe-code denial. Differential tests compare tokens, errors and
line numbers against the independent released tokenizer at every UTF-8 split
point. Upstream notices are retained under the
[MIT option](licenses/LICENSE-html5ever-MIT.txt), alongside its
[alternative Apache-2.0 text](licenses/LICENSE-html5ever-APACHE.txt).
The registry archive does not contain the historical `COPYRIGHT` file named
in its source headers; the original copyright statements and supplied
license texts are preserved.

The `parse_html_into` / `HtmlTreeSink` API emits the same
converted nodes into a caller-owned arena. `parse_dom` uses that same conversion
with its own sink, so attribute, namespace, doctype and comment handling are not
duplicated in downstream extractors. Emission is parent-first in document order;
callers supply copyable handles and receive ordered attributes before children.
This removes the intermediate Readability arena and atom lookup for callers
that do not need them, but retains the internal HTML5 tree and is not a streaming
tokenizer. The API is available from Git release 0.6.1 and changes no extraction rules.

`DomSource` and `Parser::parse_shared_document` accept a borrowed shared input
without changing Readability's internal node types. The source is imported once
per call, before entering the existing copy-on-write retry path. Forced-retry
coverage checks one import, complete output equality and input preservation.
`decode_bytes` and `parse_bytes` expose the existing reader normalization for
in-memory input; they introduce no new decoding rules.

HTML serialization is a local translation of x/net v0.59.0, including comment escaping, doctype identifiers, void-element failures, literal text in HTML integration contexts, and plaintext aborts. Text serialization follows Readeck's renderer, including queued whitespace, block separators, preformatted text, math annotations and hidden content.

Timestamps use a direct translation of `itlightning/dateparse v0.2.1` and Go's layout parser, not Markus DateParser or a heuristic substitute. Chrono supplies calendar arithmetic. TZif decoding uses tz-rs with Go-compatible transition ordering, pre-first-transition choice and abbreviation lookup. Named fallback data and Windows abbreviations come from the pinned Go toolchain. Unix uses the system zone files and Go's fallback order. Windows uses native timezone information, registry/MUI name lookup, Go's abbreviation table and its current-rule projection across a 200-year window. Unsafe code is limited to the Windows FFI module and the upstream tokenizer SIMD routines described above.

## Reader Provenance

The reader implementation and retained notices originate from the published Rust-DomDistiller 1.0.0 archive, SHA-256 `86ee78455228a72f9133dbd11b51c85fd13fc9b3cb4b708bd239671f71d489b6`.

[testdata/reader-provenance.json](testdata/reader-provenance.json) records both original and adapted file hashes. The intentional adaptation adds stream-safe processing before and after soft-hyphen removal, matching Go's normalization around combining-character runs. The import command checks the archive checksum before extracting anything and reproduces that adaptation explicitly. Formatting must not silently invalidate these receipts.

## Differential Coverage

Checked-in tests compare independent outputs produced by the Go library:

| Surface | Records |
| --- | ---: |
| Synthetic extraction and preparation, across nine option profiles | 171 |
| Reader encodings and normalization | 273 |
| Scalar matching and whitespace helpers | 490 |
| Additional entity-decoding boundaries | 7 |
| Exact unstable sort index order | 60 |
| JSON-LD decoding boundaries | 10 |
| Reused parsers, subtrees and caller mutation | 29 |
| Caller-built HTML rendering nodes | 154 |
| Parsed HTML topology, namespaces and atoms | 44 |
| Go URL parsing, resolution, fields and errors | 110 |
| Timestamp inputs under default parsing | 817 |
| Explicit Go time layouts | 140 |
| Timestamp inputs in four non-UTC zones | 3,268 |

Both direct TZif and bundled named-zone loading are checked for the four-zone records. Timestamp getter assertions cover both published and modified fields. Local tests additionally cover JSON's 10,000-level depth boundary, normalization, option defaults and unwind-safe ownership.

Optional reference runs compare all 133 saved upstream extraction pages, 1,793 x/net HTML test inputs and 821 native-platform timestamp inputs. The HTML corpus deliberately runs every source through the default full-document parser, including inputs whose original upstream tests used fragment contexts or different scripting options. Its expectations are newly produced by pinned Go; it does not assert those unrelated original test modes.

The requirement is general agreement with the pinned algorithm. These counts describe verification, not a restriction of the behavioral target. No deterministic extraction difference is accepted by the comparisons. These regression runs are correctness checks; the separate labeled-content benchmark below supplies the quality and timing measurements.

## Shared Benchmark

The [README](README.md#quality-and-performance) and Go fork README contain identical quality and timing tables from the full 983-page `content-extractor-benchmark` at commit `466fdbee8a504441eb78ed11d71c1da220681cab`. Its 2,935 positive and 2,948 negative snippets use case-sensitive containment, counting duplicates independently and failed/empty extractions as empty text. Metadata/date accuracy and token-level F1 are not measured by those scores.

The shared normalized corpus has SHA-256 `0e8b21bc8c28a88a90d891d91020a21aab95a2cfa83f761dd2b1642d98c757ea`, identical to the corpus used for the published DomDistiller comparison. Raw source pages are decoded once by that benchmark's pinned Go reader preparation, and every engine receives the same HTML and original URL. Parsing and URL setup precede the timed work. The default parser is freshly instantiated for each page; each input DOM is preserved between passes.

[tools/benchmark.py](tools/benchmark.py) builds the original Codeberg source from its exact Git archive, the fingerprinted local Go fork, and the native [Rust worker](examples/benchmark.rs). Both Go builds use the same [worker source](tools/benchmark-go/main.go), changing only its library import. It records selected and compiled dependency graphs, rejects module replacements, verifies source/binary hashes, and requires all 983 pages to be scored. The Codeberg timing baseline retains x/net 0.41.0 and x/text 0.26.0; the fork retains its qualified 0.59.0/0.42.0 graph.

A fourth, untimed control builds unchanged Codeberg extraction code with the fork's dependency versions. It matches the fork exactly across all ten compared fields and 983 pages; Rust matches the fork exactly as well. With its original dependency versions, Codeberg differs in the HTML field on 387 pages, while text, title, byline, excerpt, site name, image URL, favicon, language, errors and quality counts agree. These original-parser differences are retained in the record, not normalized away or attributed to the core-only fork. The runner refuses timing if the dependency-aligned or Go/Rust exact-output checks fail, quality changes, or an original-Codeberg difference extends beyond HTML.

The 2026-09-16 measurement used an AMD Ryzen AI 7 PRO 350, Linux/WSL2 x86_64, Go 1.27.1 and Rust 1.98.1. All measured workers were pinned to logical CPU 2; Go used `GOMAXPROCS=1` and `CGO_ENABLED=0`. Rust used the checked-in release profile with Thin LTO and one codegen unit. There was no custom allocator, native-CPU tuning, PGO or internal extraction parallelism; `LD_PRELOAD`, `RUSTFLAGS` and `CARGO_ENCODED_RUSTFLAGS` were unset. Cargo release profiles are selected by the build root, so downstream applications choose their own profile.

Workers stayed alive for two warmups and **102 measured full passes per engine**, with all six engine orders repeated 17 times in the measured phase. The aligned control was closed before timing. Counts and errors were checked after every pass. Every sample is retained, including outliers. Medians were 2,044.237251 ms for original Codeberg, 1,954.974109 ms for the Go fork and 960.028647 ms for Rust. Rust's median speedup was 2.036370597x over the fork and 2.129350262x over Codeberg, with 50.9% less time than the fork.

Timers cover extraction, plain-text rendering and snippet scoring. Input I/O, decoding, initial parsing, startup, IPC and extra quality-pass HTML/metadata collection are excluded. Normal allocations, Go garbage collection and Rust result destruction remain part of the native lifecycle. No extraction outputs are cached between passes. The median ratios are single-machine measured results, not statistical guarantees or end-to-end throughput claims.

The complete 0.6.0 numeric record for the final registry package, including all samples, source/binary hashes and the seven identical empty-extraction cases, is [testdata/benchmark-results-0.6.0-crate.json](testdata/benchmark-results-0.6.0-crate.json), SHA-256 `e275c8bf3597c3f18737f6a224e187718eb0c9d9f22829f7a5c5c6b949bcbf01`. Its measured source bytes match the repository's LF checkout. The Go repository retains a byte-identical copy. Full article outputs and build copies stay under ignored `target/benchmark/`; source pages are not repackaged or relicensed. Scorer/tooling attribution is retained in [licenses/LICENSE-benchmark.txt](licenses/LICENSE-benchmark.txt).

Historical evidence remains unchanged: [testdata/benchmark-results.json](testdata/benchmark-results.json), SHA-256 `54b53024048a6fe3be5f8bd07a0177fb18118ec01b31227ea54cf2337297cb55`, and [testdata/benchmark-results-optimized.json](testdata/benchmark-results-optimized.json), SHA-256 `9f30e23b1e7c3603d4af756c6d1a0c2372d610fa059f4323e39455d258e59544`. The latter contains the earlier clone-only comparison and profiling evidence, not the current implementation or timing result. Both records also have byte-identical Go copies. Absolute medians from separate runs should not be used as a controlled comparison.

The 102-pass LF-source run before the registry rename remains in [testdata/benchmark-results-0.6.0.json](testdata/benchmark-results-0.6.0.json), SHA-256 `7204a53f119d2d94c523d5706236304a2a5a6c2326687d0c7df018ab4b51cd04`. The final run repeated the same protocol after changing only the root package name and explicitly retaining the library name. Runtime code and dependencies were unchanged, and samples are not combined.

An earlier 102-pass run is retained in [testdata/benchmark-results-0.6.0-crlf.json](testdata/benchmark-results-0.6.0-crlf.json), SHA-256 `b245fc3c91ace7d39f91e883054bf468750e8693c8800985b9dca7490cc3c312`. Four build inputs had local CRLF bytes that Git normalized to LF; the next run repeated the protocol after normalization so its fingerprints reproduce from a checkout. Both historical 102-pass records have byte-identical Go copies. No extraction logic changed between these runs.

### Reproduction

Use Linux or WSL2 with Git, Python 3.11+, `lscpu`, Go 1.27.1 and Rust 1.98.1. The Python runner uses only the standard library. Prepare the shared normalized corpus following [Rust-DomDistiller's pinned benchmark instructions](https://github.com/markusmobius/rust-domdistiller/blob/v1.0.0/UPSTREAM.md#benchmark-reproduction); this runner verifies its exact digest and inventory rather than accepting a different corpus silently.

From a Rust-Readability source checkout with the qualified Go fork as its sibling:

```sh
git -C ../go-readabilityV2 fetch --no-tags https://codeberg.org/readeck/go-readability.git b18540d99ebf105cd67122585a0a41ec299b70bc
env -u LD_PRELOAD -u RUSTFLAGS -u CARGO_ENCODED_RUSTFLAGS python3 -B tools/benchmark.py --prepare --corpus ../rust-domdistiller/target/benchmark/corpus.json --cpu 2 --warmups 2 --samples 102 --output target/benchmark/reproduction/results.json
```

Choose a CPU allowed by the local affinity mask. `--prepare` tests and builds fresh native workers in isolated copies without editing either Go source checkout or module cache. `--go-source`, `--go`, `--git` and `--cargo` select explicit source/tool paths. Later runs reuse the verified workers unless sources change. `--warmups 0 --samples 0` runs only full-corpus quality/agreement checks; `--limit` is for first-N protocol smoke checks, never for reported quality or timing. The benchmark adapters and runner are source-checkout tooling, not runtime library APIs.

## Artifact Digests

All digests below are SHA-256. The generator verifies exact bytes by default.

| Artifact | Digest |
| --- | --- |
| `testdata/go-reference.jsonl` | `a20ee9a597821651243009df4a1fc3aeb7000b9f451845fc836d12b8d09330de` |
| `testdata/go-helpers.json` | `4d4a8d28abfb54604495aa562f3333828d3f2424685e2264441a76dd82471c92` |
| `testdata/go-modules.json` | `06e63a6862a767c4907e2dabfbcff568629aacebe38b0d164e4b62c7502a2fa2` |
| `testdata/go-source.json` | `5ff9682debf66816960d8dbb21bbcf289dd7ad6b81db7c9870abf8b8cfc046b5` |
| `src/html-atoms.json` | `79e847d04d9b4e072928fa82003efcbae0a1aee6d5b0587baa3bb56396f56122` |
| `src/timestamp/windows-abbreviations.json` | `87c1294ff30c475aac91666191ba225b1fc3670318521d27358ac88e06e2cfa3` |
| `src/timestamp/zoneinfo.zip` | `b2d18a7c8fa8142097a48c99609fb3c92db5ee98bc740294e57eab8ae9f94779` |
| `LICENSE` | `d6dc6e3f1d3793e428ad2ddfb16cdf9cf2c3f4e23084e73e8ac37d7899183311` |
| `licenses/LICENSE-dateparse.txt` | `44d79006f0fc8f4b3e063646ac5553e820468afb1b6b218e3b7f96c3c3fe65df` |

Generated, ignored corpus digests: extraction `babf373615499928e38569c9428efdad738ec4b6876b40c97cde7afd920fc4ae`; HTML parser `8fe4e65bd2f0e99b1257585c49861f069b53e2ed38b6825bf26028f1f778ba2e`. Native timezone records are machine- and year-specific and use separate platform filenames.

## Verification Status

The unpublished parser-output API was checked on 2026-09-20 with Rust 1.98.1
Windows/GNU: the 45 previously active library tests and all 1,793 independent Go
HTML corpus cases passed, followed by a new custom-handle sink contract test.
No reference fixture was regenerated. Downstream Trafilatura's 13,281 current-Go
extraction combinations also passed. These local checks do not extend the
published 0.6.0 timing record below or establish new hosted/platform coverage.

Local verification of version 0.6.0 passed on native Windows x86_64 (GNU toolchain) and Linux/WSL2 x86_64:

- All 48 Rust release tests, including the 133 saved extraction pages, 1,793 HTML-parser inputs and native-platform timezone comparisons, plus the README example.
- Exact portable Go artifact reproduction, the qualified source/dependency graph and all eight imported reader-source/notice digests.
- Strict all-target Clippy and the repository formatting gate.
- Go tests, module checksum verification, `go mod tidy -diff` and `go vet` with both Go 1.26.0 and 1.27.1 on Windows and Linux.
- The full 983-page benchmark agreement checks and 102 measured passes per engine described above.
- Local Cargo package creation and compilation on Linux, with benchmark executables intentionally excluded from the crate.

GitHub workflows perform verification only. Hosted CI is separate from these local results; macOS, ARM and native Windows MSVC have not been qualified locally. General agreement with Go remains the requirement beyond the exercised cases.