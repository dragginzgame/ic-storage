"""Run matched installed Storage suites and retain per-case measurements."""
import csv
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
SUITES = {
    "storage-engine-matrix": "examples/engine_matrix.wasm",
    "storage-read-paths": "examples/read_paths.wasm",
    "storage-installed-test": "storage_canister.wasm",
}


def measurements(log):
    result = {}
    for line in log.splitlines():
        if line.startswith(("matrix ", "comparison ", "read,")):
            fields = line.split(",") if line.startswith("read,") else line.split()
            metrics = ["instructions", "peak_heap_bytes"]
            if fields[0] == "read":
                metrics.extend(["nodes", "external_reads"])
            else:
                metrics.extend(["stable_growth_pages", "wasm_growth_pages"])
            case = " ".join(fields[:-4])
            values = dict(zip(metrics, map(int, fields[-4:]), strict=True))
        elif line.startswith("delete mutations="):
            fields = line.split()
            case = " ".join(fields[:5])
            values = {k: int(v) for k, v in (f.split("=") for f in fields[5:])}
        else:
            continue
        if case in result:
            raise ValueError(f"duplicate measurement {case}")
        result[case] = values
    if not result:
        raise ValueError("suite emitted no measurements")
    return result


def run_suite(suite, manifest, evidence, arm):
    build = evidence / "build"
    log = evidence / f"{arm}-{suite}.log"
    with log.open("w") as output:
        command = ["make", suite, f"QUAL_MANIFEST={manifest}", f"QUAL_TARGET_DIR={build}"]
        proc = subprocess.run(command, cwd=ROOT, stdout=output, stderr=subprocess.STDOUT)
    if proc.returncode:
        raise SystemExit(f"{suite} failed. See {log}")
    data = measurements(log.read_text())
    wasm = build / "wasm32-unknown-unknown/release" / SUITES[suite]
    data["fixture"] = {"wasm_bytes": wasm.stat().st_size}
    return data


def main(ref):
    revision = subprocess.check_output(
        ["git", "rev-parse", "--verify", "--end-of-options", f"{ref}^{{commit}}"],
        cwd=ROOT, text=True,
    ).strip()
    parent = ROOT / "target/evidence"
    parent.mkdir(parents=True, exist_ok=True)
    evidence = Path(tempfile.mkdtemp(prefix="release-comparison.", dir=parent))
    before = evidence / "baseline"
    before.mkdir()
    print(f"Comparing {revision} with the working tree. Evidence in {evidence}", flush=True)
    archive = evidence / "baseline.tar"
    subprocess.run(["git", "archive", "-o", archive, revision], cwd=ROOT, check=True)
    subprocess.run(["tar", "-xf", archive, "-C", before], check=True)
    archive.unlink()
    # Keep the released library and manifests. Use identical benchmark code in both arms.
    for fixture in ("storage-tests", "storage-canister"):
        source = ROOT / "qualification" / fixture
        for file in source.rglob("*.rs"):
            dest = before / file.relative_to(ROOT)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(file, dest)
    with (evidence / "paired.tsv").open("w") as output:
        writer = csv.writer(output, delimiter="\t", lineterminator="\n")
        writer.writerow(["suite", "case", "metric", "released", "candidate", "delta"])
        for suite in SUITES:
            base = run_suite(suite, before / "qualification/Cargo.toml", evidence, "released")
            current = run_suite(suite, ROOT / "qualification/Cargo.toml", evidence, "candidate")
            if base.keys() != current.keys():
                raise ValueError(f"{suite} emitted different cases")
            for case, metrics in base.items():
                if metrics.keys() != current[case].keys():
                    raise ValueError(f"{case} emitted different metrics")
                for metric, value in metrics.items():
                    after = current[case][metric]
                    writer.writerow([suite, case, metric, value, after, after - value])
            changes = [current[k]["instructions"] - v["instructions"] for k,v in base.items()
                       if "instructions" in v and not k.startswith("matrix 1 ")]
            print(f"{suite}: {len(changes)} instruction comparisons, "
                  f"{sum(d > 0 for d in changes)} slower", flush=True)


if __name__ == "__main__":
    main(sys.argv[1])
