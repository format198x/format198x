#!/usr/bin/env python3
"""Run the public Netpbm ILBM checks and print a JSON evidence record."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
CRATE = "format198x-commodore-amiga-ilbm"
CHECKS = {
    "decode": (
        "ppmtoilbm",
        "ppmtoilbm_output_decodes_with_our_decoder",
        "netpbm-encode/format198x-decode",
        ("in.ppm",),
    ),
    "encode": (
        "ilbmtoppm",
        "ilbmtoppm_reads_our_encoder_output",
        "format198x-encode/netpbm-decode",
        ("out-uncompressed.iff", "out-byterun1.iff"),
    ),
}


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def run_check(check, selected):
    tool, test, direction, fixture_names = CHECKS[check]
    record = {
        "check": check,
        "direction": direction,
        "tool": {"name": tool},
        "fixture_provenance": f"crates/{CRATE}/tests/netpbm.rs::test_image",
        "fixtures": [],
    }
    if not selected:
        return dict(record, status="not_run", reason="check was not selected")
    executable = shutil.which(tool)
    if executable is None:
        return dict(record, status="skipped", reason=f"{tool} was not found on PATH")
    try:
        version = subprocess.run(
            [executable, "-version"], capture_output=True, text=True, check=True
        )
        identity = (version.stdout + version.stderr).strip()
        if not identity:
            return dict(record, status="failed", reason=f"{tool} did not report a version")
        record["tool"].update(version=identity, sha256=sha256(Path(executable)))
        with tempfile.TemporaryDirectory(prefix="format198x-ilbm-") as directory:
            run = subprocess.run(
                [
                    "cargo", "test", "--locked", "-p", CRATE, "--test", "netpbm",
                    "--", "--ignored", "--exact", test,
                ],
                cwd=ROOT,
                env={**os.environ, "FORMAT198X_REFERENCE_EVIDENCE_DIR": directory},
                capture_output=True,
                text=True,
            )
            for name in fixture_names:
                path = Path(directory) / name
                if path.is_file() and path.stat().st_size:
                    record["fixtures"].append({"name": name, "sha256": sha256(path)})
            if run.returncode:
                return dict(record, status="failed", reason=(run.stdout + run.stderr).strip())
            if len(record["fixtures"]) != len(fixture_names):
                return dict(record, status="failed", reason="required fixture evidence is missing")
    except (OSError, subprocess.CalledProcessError) as error:
        return dict(record, status="failed", reason=str(error))
    return dict(record, status="passed", reason=None)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--strict", action="store_true", help="fail for a missing selected tool")
    parser.add_argument("--check", choices=CHECKS, action="append", help="select a direction")
    args = parser.parse_args()
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT))
    selected = args.check or CHECKS
    results = [run_check(check, check in selected) for check in CHECKS]
    print(json.dumps({"crate": CRATE, "revision": revision, "dirty": dirty, "results": results}, indent=2))
    failures = {"failed", "skipped"} if args.strict else {"failed"}
    return int(any(result["status"] in failures for result in results))


if __name__ == "__main__":
    raise SystemExit(main())
