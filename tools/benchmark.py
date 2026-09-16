import argparse
from datetime import datetime, timezone
import hashlib
import io
import itertools
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tarfile
import tempfile

from go_reference import MODULE, UPSTREAM_COMMIT, read_modules, source_snapshot


ROOT = Path(__file__).resolve().parents[1]
BUILD = ROOT / "target/benchmark"
CORPUS_COMMIT = "466fdbee8a504441eb78ed11d71c1da220681cab"
CORPUS_SHA256 = "0e8b21bc8c28a88a90d891d91020a21aab95a2cfa83f761dd2b1642d98c757ea"
CODEBERG_MODULE = "codeberg.org/readeck/go-readability/v2"
ENGINES = ("codeberg", "go-readability", "rust-readability")
CONTROL = "codeberg-aligned"
OUTPUT_FIELDS = ("text", "html", "title", "byline", "excerpt", "site_name", "image_url", "favicon", "language", "error")


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=True, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")


def metrics(counts):
    positive = counts["true_positives"]
    missed = counts["false_negatives"]
    unwanted = counts["false_positives"]
    negative = counts["true_negatives"]
    return {
        "precision": positive / (positive + unwanted) if positive + unwanted else 0.0,
        "recall": positive / (positive + missed) if positive + missed else 0.0,
        "f1": 2 * positive / (2 * positive + missed + unwanted) if positive + missed + unwanted else 0.0,
        "accuracy": (positive + negative) / (positive + missed + unwanted + negative),
    }


def command(arguments, cwd, environment, capture=False):
    result = subprocess.run(arguments, cwd=cwd, env=environment, check=True, capture_output=capture, text=capture)
    return result.stdout if capture else None


def source_hashes():
    paths = [path for path in (ROOT / "src").rglob("*") if path.is_file()]
    paths += list((ROOT / "tools/benchmark-go").glob("*.go"))
    paths += [ROOT / name for name in (
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "examples/benchmark.rs",
        "tools/benchmark.py", "tools/go_reference.py", "testdata/go-source.json", "testdata/go-modules.json",
    )]
    return {path.relative_to(ROOT).as_posix(): sha256(path) for path in sorted(paths)}


def prepare(arguments, environment):
    fork_files, fork_receipt = source_snapshot(arguments.go_source)
    if fork_receipt != (ROOT / "testdata/go-source.json").read_bytes():
        raise ValueError("Go fork source differs from the qualified oracle snapshot")
    archive = subprocess.run(
        [arguments.git, "-C", str(arguments.go_source), "-c", "core.autocrlf=false", "archive", "--format=tar", UPSTREAM_COMMIT],
        check=True, capture_output=True,
    ).stdout
    BUILD.mkdir(parents=True, exist_ok=True)
    receipt = {"upstream_commit": UPSTREAM_COMMIT, "engines": {}}
    with tempfile.TemporaryDirectory(prefix="readability-build-", dir=BUILD) as temporary:
        stage = Path(temporary)
        codeberg = stage / "codeberg"
        with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as source:
            for entry in source.getmembers():
                if entry.isdir():
                    continue
                if not entry.isfile() or Path(entry.name).is_absolute() or ".." in Path(entry.name).parts:
                    raise ValueError("Unexpected Codeberg archive path")
                destination = codeberg / entry.name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source.extractfile(entry).read())
        fork = stage / "go-readability"
        for name, content in fork_files.items():
            destination = fork / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(content)
        aligned = stage / CONTROL
        shutil.copytree(codeberg, aligned)
        command([arguments.go, "mod", "edit", "-go=1.26.0", "-toolchain=go1.27.1", "-require=golang.org/x/net@v0.59.0", "-require=golang.org/x/text@v0.42.0"], aligned, environment)
        command([arguments.go, "mod", "tidy"], aligned, environment)
        for name, source, module in (("codeberg", codeberg, CODEBERG_MODULE), ("go-readability", fork, MODULE), (CONTROL, aligned, CODEBERG_MODULE)):
            source_receipt = json.loads(source_snapshot(source)[1])
            wrapper = stage / (name + "-wrapper")
            wrapper.mkdir()
            for filename in ("main.go", "main_test.go"):
                content = (ROOT / "tools/benchmark-go" / filename).read_text(encoding="utf-8")
                if filename == "main.go":
                    if content.count('"' + MODULE + '"') != 1:
                        raise ValueError("Unexpected Go benchmark import template")
                    content = content.replace('"' + MODULE + '"', '"' + module + '"')
                (wrapper / filename).write_text(content, encoding="utf-8", newline="\n")
            main = str(wrapper / "main.go")
            command([arguments.go, "test", "-mod=readonly", main, str(wrapper / "main_test.go")], source, environment)
            command([arguments.go, "mod", "verify"], source, environment)
            graph = read_modules(command([arguments.go, "list", "-mod=readonly", "-m", "-json", "all"], source, environment, True))
            if graph.get(module) != "main":
                raise ValueError("Unexpected benchmark main module")
            if name == "go-readability" and graph != json.loads((ROOT / "testdata/go-modules.json").read_bytes()):
                raise ValueError("Go fork benchmark changed the qualified dependency graph")
            compiled = read_modules(command([arguments.go, "list", "-mod=readonly", "-deps", "-json=Module", main], source, environment, True), packages=True)
            if any(graph.get(dependency) != version for dependency, version in compiled.items()):
                raise ValueError("Compiled Go graph differs from selected modules")
            if name == CONTROL:
                qualified = json.loads((ROOT / "testdata/go-modules.json").read_bytes())
                if any(qualified.get(dependency) != version for dependency, version in compiled.items() if dependency != CODEBERG_MODULE):
                    raise ValueError("Dependency-aligned Codeberg control does not match the fork's runtime graph")
            binary = BUILD / (name + "-worker")
            command([arguments.go, "build", "-mod=readonly", "-trimpath", "-o", str(binary), main], source, environment)
            if json.loads(source_snapshot(source)[1]) != source_receipt:
                raise ValueError("Benchmark build modified reference source")
            receipt["engines"][name] = {
                "source_tree_sha256": source_receipt["tree_sha256"], "module": module,
                "selected_modules": graph, "compiled_modules": compiled,
                "binary": str(binary), "binary_sha256": sha256(binary),
            }
            if name == CONTROL:
                receipt["engines"][name]["dependency_overrides"] = {"golang.org/x/net": "v0.59.0", "golang.org/x/text": "v0.42.0"}
    rust_target = ROOT / "target/linux"
    command([arguments.cargo, "+1.98.1", "test", "--locked", "--release", "--example", "benchmark", "--target-dir", str(rust_target)], ROOT, environment)
    command([arguments.cargo, "+1.98.1", "build", "--locked", "--release", "--example", "benchmark", "--target-dir", str(rust_target)], ROOT, environment)
    binary = rust_target / "release/examples/benchmark"
    receipt["engines"]["rust-readability"] = {"binary": str(binary), "binary_sha256": sha256(binary)}
    receipt["source_sha256"] = source_hashes()
    receipt["go_version"] = command([arguments.go, "version"], ROOT, environment, True).strip()
    receipt["rust_version"] = command([str(Path(arguments.cargo).with_name("rustc")), "+1.98.1", "-Vv"], ROOT, environment, True).strip()
    save(BUILD / "build.json", receipt)
    return receipt


class Worker:
    def __init__(self, name, binary, corpus, environment):
        self.name = name
        self.process = subprocess.Popen(
            [binary, str(corpus)], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            text=True, encoding="utf-8", env=environment,
        )
        try:
            self.ready = self.read()
        except BaseException:
            self.process.kill()
            self.process.wait()
            raise

    def read(self):
        line = self.process.stdout.readline()
        if not line:
            raise RuntimeError(f"{self.name} terminated: {self.process.poll()}")
        return json.loads(line)

    def run(self, outputs=False):
        self.process.stdin.write(json.dumps({"outputs": outputs}) + "\n")
        self.process.stdin.flush()
        return self.read()

    def close(self):
        try:
            self.process.stdin.close()
            self.process.wait(timeout=30)
        except (BrokenPipeError, subprocess.TimeoutExpired):
            self.process.kill()
            self.process.wait()
        finally:
            self.process.stdout.close()
        if self.process.returncode:
            raise RuntimeError(f"{self.name} exited with {self.process.returncode}")


def compare_outputs(reference, actual):
    if len(reference) != len(actual):
        raise ValueError("Different corpus lengths")
    differences = []
    for expected, result in zip(reference, actual):
        if expected["file"] != result["file"]:
            raise ValueError("Different corpus order")
        fields = [field for field in OUTPUT_FIELDS if expected[field] != result[field]]
        if fields:
            differences.append({"file": expected["file"], "fields": fields})
    return differences


def main():
    parser = argparse.ArgumentParser(description="Codeberg, core Go fork and Rust Readability quality/speed comparison")
    parser.add_argument("--corpus", type=Path, default=ROOT.parent / "rust-domdistiller/target/benchmark/corpus.json")
    parser.add_argument("--go-source", type=Path, default=ROOT.parent / "go-readabilityV2")
    parser.add_argument("--go", default="go")
    parser.add_argument("--git", default="git")
    parser.add_argument("--cargo", default=str(Path.home() / ".cargo/bin/cargo"))
    parser.add_argument("--prepare", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    parser.add_argument("--output", type=Path, default=BUILD / "run/results.json")
    parser.add_argument("--samples", type=int, default=12)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--cpu", type=int, default=2)
    parser.add_argument("--limit", type=int, help="First-N smoke check only, never a representative benchmark")
    arguments = parser.parse_args()
    if platform.system() != "Linux":
        parser.error("Run the measurement under Linux or WSL2 for shared single-CPU affinity")
    if arguments.samples < 0 or arguments.warmups < 0 or (arguments.limit is not None and arguments.limit <= 0):
        parser.error("Invalid sample, warmup or smoke-limit count")
    arguments.go_source = arguments.go_source.resolve()
    if sha256(arguments.corpus) != CORPUS_SHA256:
        raise ValueError("Corpus bytes differ from the pinned shared 983-page benchmark")
    pages = json.loads(arguments.corpus.read_bytes())
    if (len(pages), sum(len(page["with"]) for page in pages), sum(len(page["without"]) for page in pages)) != (983, 2935, 2948):
        raise ValueError("Unexpected shared corpus inventory")
    if source_snapshot(arguments.go_source)[1] != (ROOT / "testdata/go-source.json").read_bytes():
        raise ValueError("Go source differs from the qualified oracle snapshot")
    environment = {**os.environ, "GOWORK": "off", "GOTOOLCHAIN": "go1.27.1", "CGO_ENABLED": "0", "GOMAXPROCS": "1", "TZ": "UTC", "CARGO_BUILD_JOBS": "1"}
    build = prepare(arguments, environment) if arguments.prepare or arguments.prepare_only else json.loads((BUILD / "build.json").read_bytes())
    if arguments.prepare_only:
        return
    if build["source_sha256"] != source_hashes():
        raise ValueError("Benchmark sources changed; rerun with --prepare")
    for engine in build["engines"].values():
        if sha256(engine["binary"]) != engine["binary_sha256"]:
            raise ValueError("Benchmark worker changed after preparation")
    corpus = arguments.corpus.resolve()
    if arguments.limit:
        pages = pages[:arguments.limit]
        corpus = arguments.output.parent / "subset.json"
        save(corpus, pages)
    os.sched_setaffinity(0, {arguments.cpu})
    results = {
        "created_utc": datetime.now(timezone.utc).isoformat(), "corpus_repository": "https://github.com/markusmobius/content-extractor-benchmark",
        "corpus_commit": CORPUS_COMMIT, "full_corpus_sha256": CORPUS_SHA256, "corpus_sha256": sha256(corpus),
        "pages": len(pages), "positive_snippets": sum(len(page["with"]) for page in pages), "negative_snippets": sum(len(page["without"]) for page in pages),
        "build": build, "platform": platform.platform(), "cpu": json.loads(command(["lscpu", "--json"], ROOT, environment, True)),
        "affinity": sorted(os.sched_getaffinity(0)), "python": sys.version, "options": "default parser, fresh parser per page, supplied original URL",
        "timing_scope": "pre-parsed DOM extraction, plain-text rendering and case-sensitive snippet scoring; excludes input I/O, decoding, initial parsing, startup, IPC and extra quality-pass HTML/metadata collection",
        "quality": {}, "differences": {}, "compared_fields": list(OUTPUT_FIELDS), "warmups": [], "samples": [],
    }
    workers = {}
    try:
        for name in (*ENGINES, CONTROL):
            workers[name] = Worker(name, build["engines"][name]["binary"], corpus, environment)
            if workers[name].ready != {"ready": len(pages)}:
                raise ValueError(f"Unexpected worker initialization: {name}")
        outputs = {}
        for name, worker in workers.items():
            response = worker.run(outputs=True)
            if len(response["outputs"]) != len(pages):
                raise ValueError(f"{name} skipped a page")
            save(arguments.output.parent / (name + ".json"), response)
            outputs[name] = response["outputs"]
            results["quality"][name] = {"counts": response["counts"], "errors": response["errors"], **metrics(response["counts"])}
            print(f"{name}: {results['quality'][name]}", flush=True)
        for name in ENGINES[1:]:
            differences = compare_outputs(outputs["codeberg"], outputs[name])
            results["differences"][name] = differences
            print(f"{name}: {len(pages) - len(differences)}/{len(pages)} exact pages", flush=True)
        results["aligned_codeberg_differences"] = compare_outputs(outputs[CONTROL], outputs["go-readability"])
        results["rust_fork_differences"] = compare_outputs(outputs["go-readability"], outputs["rust-readability"])
        print(f"Aligned Codeberg/fork differences: {len(results['aligned_codeberg_differences'])}; Rust/fork differences: {len(results['rust_fork_differences'])}", flush=True)
        save(arguments.output, results)
        if results["aligned_codeberg_differences"] or results["rust_fork_differences"]:
            raise RuntimeError("Exact extraction differences under matching dependencies; inspect saved outputs before timing")
        for name in ENGINES[1:]:
            if results["quality"][name] != results["quality"]["codeberg"]:
                raise RuntimeError("Quality differs from the original Codeberg release")
            if any(field != "html" for difference in results["differences"][name] for field in difference["fields"]):
                raise RuntimeError("Original Codeberg differs beyond HTML; inspect the dependency change before timing")
        workers.pop(CONTROL).close()
        outputs.clear()
        orders = tuple(itertools.permutations(ENGINES))
        for phase, repetitions in (("warmups", arguments.warmups), ("samples", arguments.samples)):
            for index in range(repetitions):
                order = orders[index % len(orders)]
                sample = {"order": list(order)}
                for name in order:
                    response = workers[name].run()
                    if response["counts"] != results["quality"][name]["counts"] or response["errors"] != results["quality"][name]["errors"]:
                        raise RuntimeError(f"Non-repeatable results from {name}")
                    sample[name] = response["elapsed_ns"]
                results[phase].append(sample)
                print(f"{phase} {index + 1}: " + ", ".join(f"{name}={sample[name] / 1e6:.1f} ms" for name in order), flush=True)
        if results["samples"]:
            results["median_ms"] = {name: statistics.median(sample[name] for sample in results["samples"]) / 1e6 for name in ENGINES}
            results["range_ms"] = {name: [min(sample[name] for sample in results["samples"]) / 1e6, max(sample[name] for sample in results["samples"]) / 1e6] for name in ENGINES}
            results["speedup_vs_codeberg"] = {name: results["median_ms"]["codeberg"] / results["median_ms"][name] for name in ENGINES}
        save(arguments.output, results)
    finally:
        for worker in workers.values():
            worker.close()


if __name__ == "__main__":
    main()