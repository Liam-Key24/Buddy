"""Home-folder filesystem tools."""

from prompts.domains.base import DomainSpec

FS = DomainSpec(
    id="fs",
    prefixes=(),
    exact_tools=("read_file", "write_file", "edit_file", "delete_file", "list_dir"),
    classify_rules=(
        "Disk files under ~/ or absolute home paths → read_file / write_file / edit_file / delete_file / list_dir.",
        "NOT for in-app Documents — use docs.* instead.",
        'List/show folder contents → list_dir. Read/open file → read_file.',
        "Create or overwrite disk file → write_file. Targeted find/replace → edit_file.",
    ),
    fill_by_tool={
        "read_file": ('{"path":"~/..."} path relative to home or absolute within home.',),
        "write_file": ('{"path":"...","content":"..."} full file contents.',),
        "edit_file": (
            '{"path":"...","old":"...","new":"..."} targeted change, OR {"path":"...","content":"..."} full replace.',
        ),
        "delete_file": ('{"path":"..."}.',),
        "list_dir": ('{"path":"Downloads"|"Desktop"|"~/..." , "depth?": number}.',),
    },
    continue_rules=(
        "After list_dir, read_file if user asked about a specific file.",
        "After read_file, edit_file if user asked to change content.",
    ),
    respond_hints=(
        "For file reads, summarize key content unless they asked for verbatim output.",
        "For writes/edits, confirm path and what changed.",
    ),
)
