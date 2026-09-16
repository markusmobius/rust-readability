import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile


ROOT = Path(__file__).resolve().parents[1]
MODULE = "github.com/markusmobius/go-readabilityV2"
UPSTREAM_COMMIT = "b18540d99ebf105cd67122585a0a41ec299b70bc"
DOMDISTILLER_SHA256 = "86ee78455228a72f9133dbd11b51c85fd13fc9b3cb4b708bd239671f71d489b6"


def run(command, **kwargs):
    return subprocess.run(command, check=True, **kwargs)


def read_modules(stream, packages=False):
    decoder = json.JSONDecoder()
    modules = {}
    while stream.strip():
        stream = stream.lstrip()
        module, offset = decoder.raw_decode(stream)
        stream = stream[offset:]
        if packages:
            module = module.get("Module")
            if not module:
                continue
        if module.get("Replace"):
            raise ValueError("Reference module replacements are forbidden")
        modules[module["Path"]] = module.get("Version", "main")
    return modules


def vendor_reader(archive_path, write):
    data = Path(archive_path).read_bytes()
    if hashlib.sha256(data).hexdigest() != DOMDISTILLER_SHA256:
        raise ValueError("Reader source archive differs from published rust-domdistiller 1.0.0")
    files = {
        "src/encoding.rs": "src/encoding.rs",
        "src/encoding/decoder.rs": "src/encoding/decoder.rs",
        "src/encoding/tables.rs": "src/encoding/tables.rs",
        "licenses/LICENSE-domdistiller.txt": "LICENSE",
        "licenses/LICENSE-go.txt": "licenses/LICENSE-go.txt",
        "licenses/LICENSE-shiori.txt": "licenses/LICENSE-shiori.txt",
        "licenses/LICENSE-chardet.txt": "licenses/LICENSE-chardet.txt",
        "licenses/icu-license.html": "licenses/icu-license.html",
    }
    hashes = {}
    upstream_hashes = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for destination, original in files.items():
            content = archive.extractfile("rust-domdistiller-1.0.0/" + original).read()
            upstream_hashes[destination] = hashlib.sha256(content).hexdigest()
            if original == "src/encoding.rs":
                for before, after in (
                    (b"        .nfd()\n", b"        .nfd()\n        .stream_safe()\n"),
                    (b"        .nfc()\n", b"        .stream_safe()\n        .nfc()\n"),
                ):
                    if content.count(before) != 1:
                        raise ValueError("Reader normalization adaptation no longer matches its source")
                    content = content.replace(before, after)
            path = ROOT / destination
            if write:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(content)
            elif not path.is_file() or path.read_bytes() != content:
                raise ValueError(f"Vendored reader source differs: {path}")
            hashes[destination] = hashlib.sha256(content).hexdigest()
            print(f"{destination}: sha256={hashes[destination]}")
    receipt = json.dumps({"crate": "rust-domdistiller", "version": "1.0.0", "archive_sha256": DOMDISTILLER_SHA256, "upstream_files": upstream_hashes, "files": hashes, "adaptations": ["Go stream-safe normalization before and after soft-hyphen removal"]}, sort_keys=True, indent=2).encode() + b"\n"
    path = ROOT / "testdata/reader-provenance.json"
    if write:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(receipt)
    elif not path.is_file() or path.read_bytes() != receipt:
        raise ValueError("Reader provenance differs")


def source_snapshot(root):
    paths = set(root.glob("*.go"))
    paths.update(root / name for name in ("go.mod", "go.sum", "LICENSE"))
    for directory in ("internal", "render", "test-pages"):
        paths.update(path for path in (root / directory).rglob("*") if path.is_file())
    files = {}
    for path in sorted(paths):
        if path.is_symlink() or not path.is_file():
            raise ValueError(f"Missing or linked reference source: {path}")
        relative = path.relative_to(root).as_posix()
        content = path.read_bytes()
        if path.suffix in (".go", ".re") or path.name in ("go.mod", "go.sum"):
            content = content.replace(b"\r\n", b"\n")
        files[relative] = content
    hashes = {name: hashlib.sha256(content).hexdigest() for name, content in files.items()}
    tree = b"".join(name.encode() + b"\0" + digest.encode() + b"\n" for name, digest in sorted(hashes.items()))
    receipt = {
        "module": MODULE,
        "status": "unreleased-source-snapshot",
        "upstream_module": "codeberg.org/readeck/go-readability/v2@v2.1.2",
        "upstream_commit": UPSTREAM_COMMIT,
        "normalization": "CRLF to LF in Go, re2go, go.mod and go.sum; other files byte-preserved",
        "tree_sha256": hashlib.sha256(tree).hexdigest(),
        "files": hashes,
    }
    return files, json.dumps(receipt, sort_keys=True, indent=2).encode() + b"\n"


def main():
    parser = argparse.ArgumentParser(description="Regenerate independent core-only Go Readability expectations.")
    parser.add_argument("--go", default="go")
    parser.add_argument("--source", type=Path, default=ROOT.parent / "go-readabilityV2", help="Checkout of the core-only Go fork matching testdata/go-source.json")
    parser.add_argument("--write", action="store_true")
    corpus = parser.add_mutually_exclusive_group()
    corpus.add_argument("--corpus", action="store_true", help="Export all upstream saved pages to ignored target/go-corpus.jsonl")
    corpus.add_argument("--html-corpus", action="store_true", help="Export pinned x/net HTML test inputs through the default document parser")
    parser.add_argument("--native-timezone", action="store_true", help="Export this machine's local timezone cases to ignored target/go-native-timezone.json")
    parser.add_argument("--reader-archive", help="Verify or restore reader sources from the checksum-pinned published DomDistiller crate")
    arguments = parser.parse_args()
    if arguments.reader_archive:
        vendor_reader(arguments.reader_archive, arguments.write)
        return
    source_root = arguments.source.resolve()
    if not (source_root / "go.mod").is_file():
        raise ValueError("Go fork checkout not found; pass --source /path/to/go-readabilityV2")
    files, receipt = source_snapshot(source_root)
    receipt_path = ROOT / "testdata/go-source.json"
    if not arguments.write and (not receipt_path.is_file() or receipt_path.read_bytes() != receipt):
        raise ValueError("Go fork source differs from testdata/go-source.json; source changes require explicit requalification")
    environment = dict(os.environ, GOWORK="off", CGO_ENABLED="0", GOTOOLCHAIN="go1.27.1", TZ="UTC")
    with tempfile.TemporaryDirectory(prefix="rust-readability-reference-") as directory:
        temporary = Path(directory)
        source = temporary / "source"
        for name, content in files.items():
            destination = source / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(content)
        run([arguments.go, "list", "-mod=readonly", "-deps", "-test", "."], cwd=source, env=environment, stdout=subprocess.DEVNULL)
        run([arguments.go, "mod", "verify"], cwd=source, env=environment)
        graph_json = run([arguments.go, "list", "-mod=readonly", "-m", "-json", "all"], cwd=source, env=environment, capture_output=True, text=True).stdout
        modules = read_modules(graph_json)
        for name, version in {MODULE: "main", "golang.org/x/net": "v0.59.0", "golang.org/x/text": "v0.42.0", "github.com/itlightning/dateparse": "v0.2.1", "github.com/go-shiori/dom": "v0.0.0-20230515143342-73569d674e1c"}.items():
            if modules.get(name) != version:
                raise ValueError(f"Unexpected selected reference dependency: {name}")
        overlay = temporary / "overlay.json"
        overlay.write_text(json.dumps({"Replace": {str(source / "z_rust_reference_test.go"): str(ROOT / "tools/go-reference/export_test.go")}}), encoding="utf-8")
        packages_json = run([arguments.go, "list", "-mod=readonly", "-overlay", str(overlay), "-deps", "-test", "-json=Module", "."], cwd=source, env=environment, capture_output=True, text=True).stdout
        for name, version in read_modules(packages_json, packages=True).items():
            if modules.get(name) != version:
                raise ValueError(f"Unexpected compiled reference dependency: {name}@{version}")
        fixture = temporary / "reference.jsonl"
        helpers = temporary / "helpers.json"
        abbreviations = temporary / "windows-abbreviations.json"
        native_timezone = temporary / "native-timezone.json"
        atoms = temporary / "html-atoms.json"
        date_module = json.loads(run([arguments.go, "list", "-mod=readonly", "-m", "-json", "github.com/itlightning/dateparse"], cwd=source, env=environment, capture_output=True, text=True).stdout)
        html_module = json.loads(run([arguments.go, "list", "-mod=readonly", "-m", "-json", "golang.org/x/net"], cwd=source, env=environment, capture_output=True, text=True).stdout)
        environment.update(RUST_READABILITY_OUTPUT=str(fixture), RUST_READABILITY_HELPERS=str(helpers), RUST_READABILITY_CORPUS="1" if arguments.corpus else "", RUST_READABILITY_DATE_TESTS=str(Path(date_module["Dir"]) / "parseany_test.go"))
        environment.update(RUST_READABILITY_ATOM_SOURCE=str(Path(html_module["Dir"]) / "html/atom/table.go"), RUST_READABILITY_ATOMS=str(atoms))
        environment.update(RUST_READABILITY_HTML_TESTDATA=str(Path(html_module["Dir"]) / "html/testdata"))
        environment.update(RUST_READABILITY_ABBREVIATIONS=str(abbreviations), RUST_READABILITY_NATIVE_TIMEZONE=str(native_timezone) if arguments.native_timezone else "")
        tests = "^TestRustExportReference$" if arguments.corpus else "^TestRustExport(Reference|Helpers)$"
        if arguments.html_corpus:
            tests = "^TestRustExportHTML$"
        run([arguments.go, "test", "-mod=readonly", "-count=1", "-overlay", str(overlay), "-run", tests, "-v", "."], cwd=source, env=environment)
        if source_snapshot(source)[1] != receipt:
            raise ValueError("Oracle execution modified the copied Go reference source")
        destination = ROOT / ("target/go-corpus.jsonl" if arguments.corpus else "testdata/go-reference.jsonl")
        if arguments.html_corpus:
            destination = ROOT / "target/go-html-corpus.jsonl"
        outputs = {destination: fixture}
        if not arguments.corpus and not arguments.html_corpus:
            outputs[ROOT / "testdata/go-helpers.json"] = helpers
            outputs[ROOT / "src/html-atoms.json"] = atoms
            outputs[ROOT / "src/timestamp/windows-abbreviations.json"] = abbreviations
            go_root = Path(run([arguments.go, "env", "GOROOT"], cwd=source, env=environment, capture_output=True, text=True).stdout.strip())
            outputs[ROOT / "src/timestamp/zoneinfo.zip"] = go_root / "lib/time/zoneinfo.zip"
            if arguments.native_timezone:
                native_platform = json.loads(native_timezone.read_bytes())["goos"]
                outputs[ROOT / f"target/go-native-timezone-{native_platform}.json"] = native_timezone
            outputs[ROOT / "licenses/LICENSE-dateparse.txt"] = Path(date_module["Dir"]) / "LICENSE"
            outputs[ROOT / "LICENSE"] = source / "LICENSE"
            graph = temporary / "modules.json"
            graph.write_text(json.dumps(modules, sort_keys=True, indent=2) + "\n", encoding="utf-8", newline="\n")
            outputs[ROOT / "testdata/go-modules.json"] = graph
            manifest = temporary / "go-source.json"
            manifest.write_bytes(receipt)
            outputs[receipt_path] = manifest
        for destination, original in outputs.items():
            if arguments.write or arguments.corpus or arguments.html_corpus or original == native_timezone:
                destination.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(original, destination)
            elif not destination.is_file() or destination.read_bytes() != original.read_bytes():
                raise ValueError(f"Reference differs: {destination}")
            print(f"{destination.relative_to(ROOT)}: sha256={hashlib.sha256(original.read_bytes()).hexdigest()}")
        print(f"Core Go fork {MODULE}; source sha256={json.loads(receipt)['tree_sha256']}; {len(modules)} selected modules, no replacements")


if __name__ == "__main__":
    main()