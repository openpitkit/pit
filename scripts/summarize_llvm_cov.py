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
import sys
from collections import defaultdict
from pathlib import Path
from typing import Any, DefaultDict, Dict, List, Optional, Set, Tuple, Union


MetricSummary = Dict[str, Union[int, float]]
Span = Tuple[int, int, int, int]


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


def collect_function_zero_counts(
    data_items: List[Dict[str, Any]],
) -> DefaultDict[str, int]:
    zero_counts: DefaultDict[str, int] = defaultdict(int)
    for item in data_items:
        for function_entry in item.get("functions", []):
            filenames = function_entry.get("filenames", [])
            if int(function_entry.get("count", 0)) != 0:
                continue
            for filename in filenames:
                zero_counts[str(filename)] += 1
    return zero_counts


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


def threshold_for_lines(metric: MetricSummary, zero_lines: Set[int]) -> float:
    if metric_notcovered(metric) > 0 and not zero_lines:
        return 97.0
    return 100.0


def threshold_for_functions(metric: MetricSummary, zero_count: int) -> float:
    if metric_notcovered(metric) > 0 and zero_count == 0:
        return 97.0
    return 100.0


def threshold_for_regions(
    metric: MetricSummary,
    covered_spans: Set[Span],
    zero_spans: Set[Span],
) -> float:
    if (
        metric_notcovered(metric) > 0
        and zero_spans
        and zero_spans.issubset(covered_spans)
    ):
        return 97.0
    return 100.0


def build_summary(export: Dict[str, Any]) -> Dict[str, Any]:
    data_items = export.get("data", [])
    function_zero_counts = collect_function_zero_counts(data_items)
    region_covered_spans, region_zero_spans = collect_region_spans(data_items)
    files_summary: List[Dict[str, Any]] = []
    problem_files: List[Dict[str, Any]] = []

    for item in data_items:
        for file_entry in item.get("files", []):
            filename = str(file_entry["filename"])
            summary = file_entry["summary"]
            line_threshold = threshold_for_lines(
                summary["lines"], collect_line_zeroes(file_entry)
            )
            function_threshold = threshold_for_functions(
                summary["functions"],
                function_zero_counts.get(filename, 0),
            )
            region_threshold = threshold_for_regions(
                summary["regions"],
                region_covered_spans.get(filename, set()),
                region_zero_spans.get(filename, set()),
            )

            lines = metric_with_threshold(summary["lines"], line_threshold)
            functions = metric_with_threshold(summary["functions"], function_threshold)
            regions = metric_with_threshold(summary["regions"], region_threshold)

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
