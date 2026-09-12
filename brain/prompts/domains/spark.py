"""Spark idea capture tools."""

from prompts.domains.base import DomainSpec

SPARK = DomainSpec(
    id="spark",
    prefixes=(),
    exact_tools=("save_spark", "list_sparks", "update_spark"),
    classify_rules=(
        "Ideas / notes to revisit → save_spark. List saved ideas → list_sparks.",
        "Archive/respark/edit/delete spark → update_spark with action.",
        "Ambiguous spark vs chat vs calendar → intent chat with one short clarifying question.",
        "High-precision idea phrases (brain dump, note to self, spark:) → save_spark even if brief.",
    ),
    fill_by_tool={
        "save_spark": ('content + tags from context (e.g. the_van, travelling, projects).',),
        "list_sparks": ('{"status?":"active|archived","limit?":30}.',),
        "update_spark": (
            '{"id":"...","action":"respark|archive|edit|delete","content?":"...","tags?":[]}.',
        ),
    },
    respond_hints=(
        'When a spark was saved, confirm the tags and a short preview (e.g. "Saved to Spark → The Van, Travelling").',
        "When listing sparks, use bullets with title/tag hints.",
    ),
)
