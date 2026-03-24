import importlib.util
from pathlib import Path


SCRIPT_PATH = Path(__file__).resolve().parents[1] / "summarize_llvm_cov.py"


def load_module():
    spec = importlib.util.spec_from_file_location("summarize_llvm_cov", SCRIPT_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_build_summary_uses_raw_file_metrics():
    module = load_module()
    export = {
        "cargo_llvm_cov": "cargo-llvm-cov 0.0.0",
        "data": [
            {
                "totals": {
                    "lines": {"count": 10, "covered": 8, "percent": 80.0},
                    "functions": {"count": 1, "covered": 1, "percent": 100.0},
                    "regions": {
                        "count": 10,
                        "covered": 8,
                        "notcovered": 2,
                        "percent": 80.0,
                    },
                    "instantiations": {
                        "count": 1,
                        "covered": 1,
                        "percent": 100.0,
                    },
                },
                "files": [
                    {
                        "filename": "/tmp/example.rs",
                        "summary": {
                            "lines": {"count": 10, "covered": 8, "percent": 80.0},
                            "functions": {"count": 1, "covered": 1, "percent": 100.0},
                            "regions": {
                                "count": 10,
                                "covered": 8,
                                "notcovered": 2,
                                "percent": 80.0,
                            },
                        },
                        "segments": [
                            [1, 1, 1, True, True, False],
                            [1, 2, 0, False, False, False],
                            [2, 1, 1, True, True, False],
                            [2, 2, 0, False, False, False],
                        ],
                    }
                ],
                "functions": [
                    {
                        "name": "test",
                        "count": 1,
                        "filenames": ["/tmp/example.rs"],
                        "regions": [
                            [1, 1, 1, 2, 1, 0, 0, 0],
                            [3, 1, 3, 1, 0, 0, 0, 0],
                        ],
                    }
                ],
            }
        ],
    }

    summary = module.build_summary(export)
    file_summary = summary["files"][0]

    assert file_summary["lines"]["count"] == 10
    assert file_summary["lines"]["covered"] == 8
    assert file_summary["lines"]["percent"] == 80.0
    assert file_summary["regions"]["count"] == 10
    assert file_summary["regions"]["covered"] == 8
    assert file_summary["regions"]["percent"] == 80.0


def test_build_summary_lowers_threshold_for_synthetic_tails():
    module = load_module()
    export = {
        "cargo_llvm_cov": "cargo-llvm-cov 0.0.0",
        "data": [
            {
                "totals": {
                    "lines": {"count": 100, "covered": 98, "percent": 98.0},
                    "functions": {"count": 1, "covered": 1, "percent": 100.0},
                    "regions": {
                        "count": 100,
                        "covered": 98,
                        "notcovered": 2,
                        "percent": 98.0,
                    },
                    "instantiations": {
                        "count": 1,
                        "covered": 1,
                        "percent": 100.0,
                    },
                },
                "files": [
                    {
                        "filename": "/tmp/example.rs",
                        "summary": {
                            "lines": {"count": 100, "covered": 98, "percent": 98.0},
                            "functions": {"count": 1, "covered": 1, "percent": 100.0},
                            "regions": {
                                "count": 100,
                                "covered": 98,
                                "notcovered": 2,
                                "percent": 98.0,
                            },
                        },
                        "segments": [
                            [10, 1, 1, True, True, False],
                            [10, 2, 0, True, True, False],
                        ],
                    }
                ],
                "functions": [
                    {
                        "name": "test",
                        "count": 1,
                        "filenames": ["/tmp/example.rs"],
                        "regions": [
                            [10, 1, 10, 2, 1, 0, 0, 0],
                            [20, 5, 20, 5, 0, 0, 0, 0],
                        ],
                    }
                ],
            }
        ],
    }

    summary = module.build_summary(export)
    file_summary = summary["files"][0]

    assert file_summary["lines"]["threshold"] == 97.0
    assert file_summary["lines"]["ok"] is True
    assert file_summary["regions"]["threshold"] == 97.0
    assert file_summary["regions"]["ok"] is True


def test_build_summary_keeps_functions_threshold_strict_with_zero_count_function():
    module = load_module()
    export = {
        "cargo_llvm_cov": "cargo-llvm-cov 0.0.0",
        "data": [
            {
                "totals": {
                    "lines": {"count": 10, "covered": 10, "percent": 100.0},
                    "functions": {"count": 2, "covered": 1, "percent": 50.0},
                    "regions": {"count": 10, "covered": 10, "percent": 100.0},
                    "instantiations": {"count": 1, "covered": 1, "percent": 100.0},
                },
                "files": [
                    {
                        "filename": "/tmp/functions.rs",
                        "summary": {
                            "lines": {"count": 10, "covered": 10, "percent": 100.0},
                            "functions": {"count": 2, "covered": 1, "percent": 50.0},
                            "regions": {"count": 10, "covered": 10, "percent": 100.0},
                        },
                        "segments": [[1, 1, 1, True, True, False]],
                    }
                ],
                "functions": [
                    {
                        "name": "covered",
                        "count": 1,
                        "filenames": ["/tmp/functions.rs"],
                        "regions": [[1, 1, 1, 2, 1, 0, 0, 0]],
                    },
                    {
                        "name": "bookkeeping_zero_count",
                        "count": 0,
                        "filenames": ["/tmp/functions.rs"],
                        "regions": [[2, 1, 2, 1, 0, 0, 0, 0]],
                    },
                ],
            }
        ],
    }

    summary = module.build_summary(export)
    file_summary = summary["files"][0]

    assert file_summary["functions"]["threshold"] == 100.0
    assert file_summary["functions"]["ok"] is False


def test_build_summary_lowers_threshold_for_uncovered_wildcard_fallback(tmp_path):
    module = load_module()
    source = tmp_path / "non_exhaustive_match.rs"
    source.write_text(
        "fn map(value: i32) -> i32 {\n"
        "    match value {\n"
        "        $(VALUE => RESULT,)+\n"
        "        _ => 0,\n"
        "    }\n"
        "}\n",
        encoding="utf-8",
    )
    export = {
        "cargo_llvm_cov": "cargo-llvm-cov 0.0.0",
        "data": [
            {
                "totals": {
                    "lines": {"count": 5, "covered": 4, "percent": 80.0},
                    "functions": {"count": 1, "covered": 1, "percent": 100.0},
                    "regions": {"count": 5, "covered": 4, "percent": 80.0},
                    "instantiations": {"count": 1, "covered": 1, "percent": 100.0},
                },
                "files": [
                    {
                        "filename": str(source),
                        "summary": {
                            "lines": {"count": 5, "covered": 4, "percent": 80.0},
                            "functions": {"count": 1, "covered": 1, "percent": 100.0},
                            "regions": {"count": 5, "covered": 4, "percent": 80.0},
                        },
                        "segments": [
                            [2, 1, 1, True, True, False],
                            [3, 1, 0, True, True, False],
                            [4, 1, 0, True, True, False],
                        ],
                    }
                ],
                "functions": [
                    {
                        "name": "map",
                        "count": 1,
                        "filenames": [str(source)],
                        "regions": [
                            [2, 1, 3, 10, 1, 0, 0, 0],
                            [4, 9, 4, 15, 0, 0, 0, 0],
                        ],
                    }
                ],
            }
        ],
    }

    summary = module.build_summary(export)
    file_summary = summary["files"][0]

    assert file_summary["lines"]["threshold"] == 97.0
    assert file_summary["regions"]["threshold"] == 97.0
