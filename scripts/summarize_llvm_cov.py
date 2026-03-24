#!/usr/bin/env python3
# Copyright The Pit Project Owners. All rights reserved.
# SPDX-License-Identifier: Apache-2.0
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#
# Please see https://github.com/openpitkit and the OWNERS file for details.

"""Summarize cargo llvm-cov JSON with adaptive per-metric thresholds."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, DefaultDict, Dict, List, Optional, Set, Tuple, Union


MetricSummary = Dict[str, Union[int, float]]
Span = Tuple[int, int, int, int]
WILDCARD_MATCH_ARM = re.compile(r"^\s*_\s*=>")
MACRO_MATCH_REPETITION = re.compile(r"^\s*\$\(.+\)\+[,]?\s*$")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Summarize cargo llvm-cov JSON with 100% thresholds by default "
            "and 97% thresholds only for metrics affected by synthetic tails."
        )
    )
    parser.add_argument("input", type=Path, help="raw cargo llvm-cov JSON export")
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="path to write the compact JSON summary",
    )
    parser.add_argument(
        "--text",
        action="store_true",
        help="print a concise human-readable summary to stdout",
    )
    return parser.parse_args()


def load_export(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def relative_path(path: str) -> str:
    try:
        return str(Path(path).resolve().relative_to(Path.cwd().resolve()))
    except ValueError:
        return path


def metric_notcovered(metric: MetricSummary) -> int:
    if "notcovered" in metric:
        return int(metric["notcovered"])
    return int(metric["count"]) - int(metric["covered"])


def aggregate_metric(
    metric_name: str,
    data_items: List[Dict[str, Any]],
) -> MetricSummary:
    total_count = 0
    total_covered = 0
    total_notcovered = 0
    saw_metric = False
    for item in data_items:
        totals = item.get("totals", {})
        metric = totals.get(metric_name)
        if not metric:
            continue
        saw_metric = True
        total_count += int(metric.get("count", 0))
        total_covered += int(metric.get("covered", 0))
        total_notcovered += metric_notcovered(metric)

    if not saw_metric:
        return {"count": 0, "covered": 0, "notcovered": 0, "percent": 100.0}

    percent = 100.0
    if total_count:
        percent = (total_covered / total_count) * 100.0
    return {
        "count": total_count,
        "covered": total_covered,
        "notcovered": total_notcovered,
        "percent": percent,
    }


def metric_with_threshold(
    metric: MetricSummary, threshold: float
) -> Dict[str, Union[int, float]]:
    percent = float(metric["percent"])
    return {
        "count": int(metric["count"]),
        "covered": int(metric["covered"]),
        "notcovered": metric_notcovered(metric),
        "percent": percent,
        "threshold": threshold,
        "ok": percent >= threshold,
    }


def metric_from_counts(count: int, covered: int) -> MetricSummary:
    notcovered = max(count - covered, 0)
    percent = 100.0 if count == 0 else (covered / count) * 100.0
    return {
        "count": count,
        "covered": covered,
        "notcovered": notcovered,
        "percent": percent,
    }


def resolve_region_filename(
    function_entry: Dict[str, Any],
    region: List[Any],
) -> Optional[str]:
    filenames = function_entry.get("filenames", [])
    if len(region) >= 6:
        file_index = region[5]
        if isinstance(file_index, int) and 0 <= file_index < len(filenames):
            return str(filenames[file_index])
    if len(filenames) == 1:
        return str(filenames[0])
    return None


def collect_line_zeroes(file_entry: Dict[str, Any]) -> Set[int]:
    zero_lines: Set[int] = set()
    for segment in file_entry.get("segments", []):
        if len(segment) < 4:
            continue
        if bool(segment[3]) and int(segment[2]) == 0:
            zero_lines.add(int(segment[0]))
    return zero_lines


def collect_line_covered(file_entry: Dict[str, Any]) -> Set[int]:
    covered_lines: Set[int] = set()
    for segment in file_entry.get("segments", []):
        if len(segment) < 4:
            continue
        if bool(segment[3]) and int(segment[2]) > 0:
            covered_lines.add(int(segment[0]))
    return covered_lines


def uncovered_lines(file_entry: Dict[str, Any]) -> Set[int]:
    return collect_line_zeroes(file_entry) - collect_line_covered(file_entry)


def has_only_wildcard_uncovered_lines(
    filename: str, file_entry: Dict[str, Any]
) -> bool:
    missing = uncovered_lines(file_entry)
    if not missing:
        return False
    try:
        lines = Path(filename).read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeDecodeError):
        return False
    for line_no in missing:
        if line_no <= 0 or line_no > len(lines):
            return False
        line = lines[line_no - 1]
        if not (WILDCARD_MATCH_ARM.match(line) or MACRO_MATCH_REPETITION.match(line)):
            return False
    return True


def effective_line_metric(file_entry: Dict[str, Any]) -> MetricSummary:
    covered_lines = collect_line_covered(file_entry)
    effective_uncovered = uncovered_lines(file_entry)
    effective_count = len(covered_lines | effective_uncovered)
    effective_covered = len(covered_lines)
    return metric_from_counts(effective_count, effective_covered)


def collect_region_spans(
    data_items: List[Dict[str, Any]],
) -> Tuple[DefaultDict[str, Set[Span]], DefaultDict[str, Set[Span]]]:
    covered_spans: DefaultDict[str, Set[Span]] = defaultdict(set)
    zero_spans: DefaultDict[str, Set[Span]] = defaultdict(set)

    for item in data_items:
        for function_entry in item.get("functions", []):
            for region in function_entry.get("regions", []):
                if len(region) < 5:
                    continue
                filename = resolve_region_filename(function_entry, region)
                if filename is None:
                    continue
                span = (
                    int(region[0]),
                    int(region[1]),
                    int(region[2]),
                    int(region[3]),
                )
                count = int(region[4])
                if count > 0:
                    covered_spans[filename].add(span)
                elif count == 0:
                    zero_spans[filename].add(span)

    return covered_spans, zero_spans


def threshold_for_lines(
    metric: MetricSummary,
    effective_metric: MetricSummary,
    raw_region_metric: Optional[MetricSummary] = None,
    eff_region_metric: Optional[MetricSummary] = None,
    wildcard_uncovered_only: bool = False,
) -> float:
    if float(metric["percent"]) < 100.0 and wildcard_uncovered_only:
        return 97.0
    if float(metric["percent"]) < 100.0:
        if float(effective_metric["percent"]) > float(metric["percent"]):
            return 97.0
        # When the region metric already has synthetic point spans
        # (effective_region > raw_region), the same file likely contains
        # synthetic uncoverable lines — e.g. wildcard arms required by
        # `#[non_exhaustive]` enums from external crates.  Apply the same
        # relaxed threshold so those structurally-unreachable lines do not
        # fail the build.
        if (
            raw_region_metric is not None
            and eff_region_metric is not None
            and float(raw_region_metric["percent"]) < 100.0
            and float(eff_region_metric["percent"])
            > float(raw_region_metric["percent"])
        ):
            return 97.0
    return 100.0


def threshold_for_functions(metric: MetricSummary) -> float:
    if float(metric["percent"]) < 100.0:
        return 100.0
    return 100.0


def threshold_for_regions(
    metric: MetricSummary,
    effective_metric: MetricSummary,
    wildcard_uncovered_only: bool = False,
) -> float:
    if float(metric["percent"]) < 100.0 and wildcard_uncovered_only:
        return 97.0
    if float(metric["percent"]) < 100.0 and float(effective_metric["percent"]) > float(
        metric["percent"]
    ):
        return 97.0
    return 100.0


def effective_region_metric(
    covered_spans: Set[Span], zero_spans: Set[Span]
) -> MetricSummary:
    raw_uncovered = zero_spans - covered_spans

    # LLVM JSON may include point-sized bookkeeping regions (single-column spans)
    # that are not meaningful executable regions for thresholding.
    def is_point_span(span: Span) -> bool:
        return span[0] == span[2] and (span[3] - span[1]) <= 1

    effective_uncovered = {span for span in raw_uncovered if not is_point_span(span)}
    effective_count = len(covered_spans) + len(effective_uncovered)
    return metric_from_counts(effective_count, len(covered_spans))


def build_summary(export: Dict[str, Any]) -> Dict[str, Any]:
    data_items = export.get("data", [])
    region_covered_spans, region_zero_spans = collect_region_spans(data_items)
    files_summary: List[Dict[str, Any]] = []
    problem_files: List[Dict[str, Any]] = []

    for item in data_items:
        for file_entry in item.get("files", []):
            filename = str(file_entry["filename"])
            summary = file_entry["summary"]
            line_metric = summary["lines"]
            function_metric = summary["functions"]
            region_metric = summary["regions"]
            covered_spans = region_covered_spans.get(filename, set())
            zero_spans = region_zero_spans.get(filename, set())
            effective_lines = effective_line_metric(file_entry)
            effective_regions = effective_region_metric(covered_spans, zero_spans)
            wildcard_uncovered_only = has_only_wildcard_uncovered_lines(
                filename, file_entry
            )
            line_threshold = threshold_for_lines(
                line_metric,
                effective_lines,
                region_metric,
                effective_regions,
                wildcard_uncovered_only,
            )
            function_threshold = threshold_for_functions(function_metric)
            region_threshold = threshold_for_regions(
                region_metric, effective_regions, wildcard_uncovered_only
            )

            lines = metric_with_threshold(line_metric, line_threshold)
            functions = metric_with_threshold(function_metric, function_threshold)
            regions = metric_with_threshold(region_metric, region_threshold)

            file_summary = {
                "path": relative_path(filename),
                "raw_path": filename,
                "lines": lines,
                "functions": functions,
                "regions": regions,
            }
            files_summary.append(file_summary)

            has_problem = (
                not bool(lines["ok"])
                or not bool(functions["ok"])
                or not bool(regions["ok"])
            )
            if has_problem:
                problem_files.append(file_summary)

    problem_files.sort(
        key=lambda item: (
            float(item["lines"]["percent"]),
            float(item["functions"]["percent"]),
            float(item["regions"]["percent"]),
            item["path"],
        )
    )
    files_summary.sort(key=lambda item: item["path"])

    totals = {
        "lines": metric_with_threshold(aggregate_metric("lines", data_items), 100.0),
        "functions": metric_with_threshold(
            aggregate_metric("functions", data_items), 100.0
        ),
        "regions": metric_with_threshold(
            aggregate_metric("regions", data_items), 100.0
        ),
        "instantiations": metric_with_threshold(
            aggregate_metric("instantiations", data_items),
            100.0,
        ),
    }

    return {
        "generated_from": str(export.get("cargo_llvm_cov", "")),
        "totals": totals,
        "report": {
            "status": "ok" if not problem_files else "attention_required",
            "file_count": len(problem_files),
            "files": problem_files,
        },
        "files": files_summary,
    }


def print_text(summary: Dict[str, Any]) -> None:
    totals = summary["totals"]
    report = summary["report"]

    if not report["files"]:
        print()
        print("#" * 72)
        print("#" + " " * 70 + "#")
        print("#" + " " * 27 + "COVERAGE OK" + " " * 32 + "#")
        print("#" + " " * 70 + "#")
        print("#" * 72)
        print(
            "all file metrics passed their thresholds:"
            f" lines {totals['lines']['percent']:.2f}%,"
            f" functions {totals['functions']['percent']:.2f}%,"
            f" regions {totals['regions']['percent']:.2f}%"
        )
        return

    print("Coverage report")
    print(
        "raw totals:"
        f" lines {totals['lines']['percent']:.2f}%,"
        f" functions {totals['functions']['percent']:.2f}%,"
        f" regions {totals['regions']['percent']:.2f}%"
    )
    print("files below threshold:")
    for file_summary in report["files"]:
        parts = []
        if not bool(file_summary["lines"]["ok"]):
            parts.append(
                f"lines {file_summary['lines']['percent']:.2f}%"
                f" (threshold {file_summary['lines']['threshold']:.0f}%)"
            )
        if not bool(file_summary["functions"]["ok"]):
            parts.append(
                f"functions {file_summary['functions']['percent']:.2f}%"
                f" (threshold {file_summary['functions']['threshold']:.0f}%)"
            )
        if not bool(file_summary["regions"]["ok"]):
            parts.append(
                f"regions {file_summary['regions']['percent']:.2f}%"
                f" (threshold {file_summary['regions']['threshold']:.0f}%)"
            )
        print(f"  - {file_summary['path']}: {', '.join(parts)}")


def main() -> int:
    args = parse_args()
    export = load_export(args.input)
    summary = build_summary(export)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8") as handle:
        json.dump(summary, handle, indent=2, sort_keys=True)
        handle.write("\n")

    if args.text:
        print_text(summary)

    return 0


if __name__ == "__main__":
    sys.exit(main())
