#!/usr/bin/env python3
"""Generate expected.json by running a KNOWN-GOOD parser over the corpus.

The expectations are not hand-written. They are produced by the implementation that has been in
production against a real corpus for weeks and has had four separate blind spots beaten out of it —
wrapping, quote blocks, `<br>` in table cells, and prose-in-the-token. Hand-writing them would encode
my reading of the rules rather than the rules.

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
    # this against what the parser returned is the whole reason the reference's three remaining blind
    # spots were found at all — so it is emitted per file, not just totalled.
    raw_pattern = __import__("re").compile(r"Verify:\s*(\d{4}-\d{2}-\d{2})\s*—")

    records = []
    coverage = {}
    for path in sorted(glob.glob(os.path.join(HERE, "corpus", "*.md"))):
        name = os.path.basename(path)
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        raw_hits = len(raw_pattern.findall(text))
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
        # READ THIS BEFORE USING `records` AS A TARGET. The records below are what the reference
        # implementation produces, which is NOT the same as what a correct implementation should
        # produce. `corpus/06-reference-gaps.md` holds four well-formed annotations the reference
        # finds none of; `01-forms.md` holds one deliberate near-miss it correctly rejects. Treat
        # `records` as a differential baseline and `coverage` as the conformance target.
        "reference_is_not_ground_truth": True,
        "known_reference_gaps": {
            "corpus/06-reference-gaps.md": (
                "4 well-formed annotations, 1 found by the reference. Three mechanisms: (1) a "
                "`Verify:` ending a table cell with no preceding `<br>`; (2) the second of two "
                "consecutive table rows each carrying `<br>Verify:`, absorbed into the first's body "
                "with its date and horizon both lost; (3) an annotation wrapped in inline-code "
                "backticks."
            ),
            "corpus/03-hidden-positions.md": (
                "7 well-formed annotations, 5 found. The two misses are mechanisms (1) and (2) "
                "above, occurring incidentally in a file written to test the three positions the "
                "reference already handles. That is the point worth taking: the gaps were not "
                "sought, they fell out of writing a realistic table."
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
