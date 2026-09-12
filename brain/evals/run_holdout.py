"""Hold-out paraphrase evals for classify→fill (soft NL scoring).

Runs without MLX: mlx_complete raises so repair/heuristic fill is exercised.
Do NOT tune CALENDAR_* regexes against these files — they are frozen hold-outs.

Usage (from brain/):
  python evals/run_holdout.py
"""

from __future__ import annotations

import json
import logging
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from plan_pipeline import run_classify_fill  # noqa: E402

logging.getLogger("buddy.brain.pipeline").setLevel(logging.ERROR)

EVAL_FILES = [
    "calendar_holdout.jsonl",
    "spark_holdout.jsonl",
    "fs_holdout.jsonl",
    "coder_holdout.jsonl",
    "chat_holdout.jsonl",
]


def _failing_mlx(_system: str, _messages: list) -> str:
    raise RuntimeError("holdout eval forces repair path (no MLX)")


def _tool_input_obj(plan) -> dict:
    raw = plan.tool_input or "{}"
    if isinstance(raw, dict):
        return raw
    try:
        data = json.loads(raw)
        return data if isinstance(data, dict) else {}
    except json.JSONDecodeError:
        return {}


def score_case(case: dict) -> list[str]:
    """Return list of failure strings (empty = pass)."""
    failures: list[str] = []
    plan = run_classify_fill(
        message=case["message"],
        history=[],
        memory_messages=[{"role": "user", "content": case["message"]}],
        available_tools=(
            "- save_spark\n- coder.run\n- read_file\n- list_dir\n"
            "- calendar.look\n- calendar.pin\n- calendar.organize"
        ),
        mlx_complete=_failing_mlx,
        scratchpad=None,
    )

    expect_intent = case.get("expect_intent")
    if expect_intent and plan.intent != expect_intent:
        failures.append(f"intent={plan.intent!r} want {expect_intent!r}")

    if expect_intent == "chat" and plan.intent == "tool_use":
        failures.append("false tool_use on chat hold-out")

    expect_tool = case.get("expect_tool")
    if expect_tool and plan.tool != expect_tool:
        failures.append(f"tool={plan.tool!r} want {expect_tool!r}")

    family = case.get("expect_tool_family")
    if family:
        tool = plan.tool or ""
        ok = False
        if family == "calendar.schedule":
            ok = tool in (
                "calendar.organize",
                "calendar.look",
                "calendar.pin",
            )
        elif family == "calendar":
            ok = tool.startswith("calendar.")
        elif family == "read_file":
            ok = "read" in tool
        elif family == "list":
            ok = "list" in tool
        else:
            ok = tool.startswith(family) or family in tool
        if not ok:
            failures.append(f"tool={tool!r} not in family {family!r}")

    if case.get("forbid_duration"):
        obj = _tool_input_obj(plan)
        if "duration_minutes" in obj and obj["duration_minutes"] not in (None, "", 0):
            failures.append(
                f"invented duration_minutes={obj.get('duration_minutes')!r}"
            )
        for item in obj.get("items") or []:
            if isinstance(item, dict) and item.get("duration_minutes") not in (
                None,
                "",
                0,
            ):
                failures.append(
                    f"invented item duration_minutes={item.get('duration_minutes')!r}"
                )

    return failures


def main() -> int:
    eval_dir = Path(__file__).resolve().parent
    total = 0
    failed = 0
    for name in EVAL_FILES:
        path = eval_dir / name
        if not path.exists():
            print(f"MISSING {name}")
            failed += 1
            continue
        print(f"== {name} ==")
        for line in path.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line:
                continue
            case = json.loads(line)
            total += 1
            failures = score_case(case)
            if failures:
                failed += 1
                print(f"  FAIL {case.get('id')}: {case['message'][:60]!r}")
                for f in failures:
                    print(f"    - {f}")
            else:
                print(f"  ok   {case.get('id')}")
    print(f"\n{total - failed}/{total} passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
