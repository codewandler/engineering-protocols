#!/usr/bin/env python3
"""Generate expected.json by running a KNOWN-GOOD parser over the corpus.

The expectations are not hand-written. They are produced by an implementation that has been in
production against a real corpus for months and has had SEVEN separate blind spots beaten out of it —
three found in production, four found by this corpus. Hand-writing them would encode one reading of
the rules rather than the rules.

  python3 generate_expected.py --impl /path/to/check-verify.py [--today 2026-09-01]

Re-run it if the corpus changes. A fixture whose expectations drifted from its corpus is worse than no
fixture, because it fails for the wrong reason.
"""
import argparse
import datetime
import glob
import importlib.util
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REFERENCE_DATE = "2026-09-01"
WARN_DAYS = 2


def load_impl(path):
    """Import a module from a path whose filename is not a legal identifier."""
    bindir = os.path.dirname(os.path.abspath(path))
    sys.path.insert(0, bindir)  # its sibling helper imports
    spec = importlib.util.spec_from_file_location("reference_impl", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--impl", required=True, help="path to the reference check-verify.py")
    ap.add_argument("--today", default=REFERENCE_DATE)
    ap.add_argument("--out", default=os.path.join(HERE, "expected.json"))
    args = ap.parse_args()

    impl = load_impl(args.impl)
    today = datetime.date.fromisoformat(args.today)

    # A lower bound on what a human would call an annotation, independent of any parser. Comparing
    # this against what the parser returned is the whole reason four blind spots were found at all —
    # so it is emitted per file, not just totalled. `missed_by_reference: 0` everywhere is the target.
    raw_pattern = __import__("re").compile(r"Verify:\s*(\d{4}-\d{2}-\d{2})\s*—")
    fence_pattern = __import__("re").compile(r"^\s*```")

    def strip_fences(text):
        """Blank fenced blocks, keeping line count. Implemented HERE rather than imported from the
        implementation under test: the fixture must be able to judge an implementation that has not
        thought about fences yet, and one that counts its own examples as claims should fail this
        corpus rather than agree with itself."""
        out, inside = [], False
        for line in text.split("\n"):
            if fence_pattern.match(line):
                inside = not inside
                out.append("")
                continue
            out.append("" if inside else line)
        return "\n".join(out)

    records = []
    coverage = {}
    for path in sorted(glob.glob(os.path.join(HERE, "corpus", "*.md"))):
        name = os.path.basename(path)
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        raw_hits = len(raw_pattern.findall(strip_fences(text)))
        verifies = impl.parse_verifies(text)
        coverage[name] = {
            "raw_occurrences": raw_hits,
            "parsed_by_reference": len(verifies),
            "missed_by_reference": raw_hits - len(verifies),
        }
        expired, expiring, ok = impl.classify(verifies, today, warn_days=WARN_DAYS)
        bucket = {}
        for entry, days in expired:
            bucket[id(entry)] = ("expired", days)
        for entry, days in expiring:
            bucket[id(entry)] = ("expiring", days)
        for entry, days in ok:
            bucket[id(entry)] = ("ok", days)
        for entry in verifies:
            state, days = bucket[id(entry)]
            records.append({
                "file": name,
                "date": entry["date"],
                "horizon": entry["horizon"],
                "malformed": entry["malformed"],
                "state": state,
                # days over for `expired`, days left otherwise
                "days": days,
                "what": entry["what"],
            })

    summary = {
        "total": len(records),
        "malformed": sum(1 for r in records if r["malformed"]),
        "expired": sum(1 for r in records if r["state"] == "expired"),
        "expiring": sum(1 for r in records if r["state"] == "expiring"),
        "ok": sum(1 for r in records if r["state"] == "ok"),
        "per_file": {},
    }
    for record in records:
        summary["per_file"][record["file"]] = summary["per_file"].get(record["file"], 0) + 1

    summary["raw_occurrences_all_files"] = sum(c["raw_occurrences"] for c in coverage.values())
    summary["missed_by_reference"] = sum(c["missed_by_reference"] for c in coverage.values())

    document = {
        "reference_date": args.today,
        "warn_days": WARN_DAYS,
        "default_horizon_when_malformed": impl.DEFAULT_HORIZON,
        "generated_by": "generate_expected.py against a reference implementation",
        # Was True while the reference had four blind spots this corpus found. It now parses all
        # of them, so these records ARE the target — but the flag is kept as a field rather than
        # deleted, because the next position nobody has thought of will flip it back.
        "reference_is_not_ground_truth": False,
        "known_reference_gaps": {
            "status": (
                "None outstanding. Four positions were found by this corpus and fixed upstream on "
                "2026-08-21: (1) a `Verify:` ending a table cell with no preceding `<br>`; (2) the "
                "second of two consecutive table rows each carrying `<br>Verify:`, absorbed into the "
                "first's body with its date and horizon both lost; (3) an annotation wrapped in "
                "inline-code backticks; (4) the same, mid-line after prose. A fifth rule came with "
                "them: an annotation inside a fenced code block is an example and is excluded from "
                "both parsing and the coverage count."
            ),
            "why_this_field_stays": (
                "Positions 1-3 of the same class were each found in production, fixed, and believed "
                "complete before 4-7 turned up. Assume there is another one."
            ),
        },
        "coverage": coverage,
        "summary": summary,
        "records": records,
    }
    with open(args.out, "w", encoding="utf-8") as handle:
        json.dump(document, handle, indent=1, ensure_ascii=False)
        handle.write("\n")
    print(f"{args.out}: {summary['total']} record(s) — "
          f"{summary['ok']} ok, {summary['expiring']} expiring, {summary['expired']} expired, "
          f"{summary['malformed']} malformed")


if __name__ == "__main__":
    main()
