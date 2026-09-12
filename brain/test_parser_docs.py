"""Docs create/update heuristic routing."""

from __future__ import annotations

import json

from parser import heuristic_fill_for_tool, repair_classified_tool, try_fast_heuristic_plan


def test_format_existing_doc_fast_path():
    msg = "edit bello.today document and make the format better please"
    plan = try_fast_heuristic_plan(msg)
    assert plan is not None
    assert plan.tool == "docs.format"
    data = json.loads(plan.tool_input or "{}")
    assert data["id"] == "bello.today"


def test_named_document_fast_path():
    msg = (
        "make a document called bello.today and add make this readme.md a better format : "
        "This is a [Next.js](https://nextjs.org) project bootstrapped with create-next-app."
    )
    plan = try_fast_heuristic_plan(msg)
    assert plan is not None
    assert plan.tool == "docs.upsert"
    data = json.loads(plan.tool_input or "{}")
    assert data["title"] == "bello.today"
    assert "Next.js" in data["content"]


def test_write_file_repaired_to_docs():
    intent, tool = repair_classified_tool(
        "tool_use",
        "write_file",
        "make a document called notes",
    )
    assert intent == "tool_use"
    assert tool == "docs.upsert"


def test_fill_docs_upsert():
    raw = heuristic_fill_for_tool(
        "docs.upsert",
        "create a document called hello : short body here ok",
    )
    data = json.loads(raw)
    assert data["title"] == "hello"
    assert "short body" in data["content"]


if __name__ == "__main__":
    test_named_document_fast_path()
    test_write_file_repaired_to_docs()
    test_fill_docs_upsert()
    print("ok parser docs")
