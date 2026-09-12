"""In-app Documents tools."""

from prompts.domains.base import DomainSpec

DOCS = DomainSpec(
    id="docs",
    prefixes=("docs.",),
    classify_rules=(
        "In-app Documents: better format → docs.format; small edit → docs.patch; create/paste body → docs.upsert; read → docs.get / docs.search / docs.list.",
        "write_file/edit_file are home-folder disk files only, NOT the Documents app.",
        "Long paste with title → docs.upsert. Reformat existing doc → docs.format (never rewrite body).",
        "Search then read: docs.search → docs.get in agent loop when full text needed.",
    ),
    fill_by_tool={
        "docs.format": ('{"id":"bello.today"} improve layout only — never rewrite the body.',),
        "docs.patch": (
            '{"id":"...","find":"...","replace":"..."} one exact snippet.',
            "Prefer patch over upsert for small edits.",
        ),
        "docs.upsert": (
            '{"title":"...","content":"<user paste, light cleanup only>"}.',
            "Do not fully rewrite long markdown. format optional (markdown|html|csv).",
        ),
        "docs.get": ('{"id":"..."} id or title.',),
        "docs.search": ('{"query":"...","limit":5}.',),
        "docs.list": ('{"folder_id?":"..."}.',),
        "docs.delete": ('{"id":"..."}.',),
    },
    continue_rules=(
        "After docs.search, call docs.get if user needs full document content for edit.",
        "After docs.get, call docs.patch or docs.format if user asked to change it.",
    ),
    respond_hints=(
        "When a document was created or updated, confirm the title and that it is in Documents.",
        "When formatting or patching, confirm what changed briefly.",
    ),
)
