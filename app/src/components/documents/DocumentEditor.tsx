import { useEffect, useMemo } from "react";
import { EditorContent, useEditor } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import Link from "@tiptap/extension-link";
import Placeholder from "@tiptap/extension-placeholder";
import { Table } from "@tiptap/extension-table";
import TableRow from "@tiptap/extension-table-row";
import TableCell from "@tiptap/extension-table-cell";
import TableHeader from "@tiptap/extension-table-header";
import TaskList from "@tiptap/extension-task-list";
import TaskItem from "@tiptap/extension-task-item";

export function RichDocumentEditor({
  content,
  onChange,
}: {
  content: string;
  onChange: (html: string) => void;
}) {
  const editor = useEditor({
    extensions: [
      StarterKit,
      Link.configure({ openOnClick: false }),
      Placeholder.configure({ placeholder: "Write…" }),
      Table.configure({ resizable: true }),
      TableRow,
      TableHeader,
      TableCell,
      TaskList,
      TaskItem.configure({ nested: true }),
    ],
    content: content || "<p></p>",
    onUpdate: ({ editor: ed }) => onChange(ed.getHTML()),
  });

  useEffect(() => {
    if (!editor) return;
    const current = editor.getHTML();
    if (content !== current && content) {
      editor.commands.setContent(content);
    }
  }, [content, editor]);

  if (!editor) return null;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="mb-2 flex flex-wrap gap-1 border-b border-zinc-800 pb-2">
        <Mark btn={() => editor.chain().focus().toggleBold().run()} label="B" />
        <Mark btn={() => editor.chain().focus().toggleItalic().run()} label="I" />
        <Mark btn={() => editor.chain().focus().toggleHeading({ level: 1 }).run()} label="H1" />
        <Mark btn={() => editor.chain().focus().toggleHeading({ level: 2 }).run()} label="H2" />
        <Mark btn={() => editor.chain().focus().toggleBulletList().run()} label="List" />
        <Mark btn={() => editor.chain().focus().toggleOrderedList().run()} label="1." />
        <Mark btn={() => editor.chain().focus().toggleTaskList().run()} label="☐" />
        <Mark btn={() => editor.chain().focus().toggleCodeBlock().run()} label="Code" />
        <Mark
          btn={() => editor.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run()}
          label="Table"
        />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto rounded-xl border border-zinc-800 bg-zinc-950/40 px-4 py-3 text-sm text-zinc-200 prose-invert [&_.ProseMirror]:min-h-full [&_.ProseMirror]:outline-none">
        <EditorContent editor={editor} />
      </div>
    </div>
  );
}

function Mark({ btn, label }: { btn: () => void; label: string }) {
  return (
    <button
      type="button"
      onClick={btn}
      className="rounded-lg px-2 py-1 text-[11px] text-zinc-400 hover:bg-zinc-800 hover:text-zinc-200"
    >
      {label}
    </button>
  );
}

export function CsvEditor({
  content,
  onChange,
}: {
  content: string;
  onChange: (csv: string) => void;
}) {
  const rows = useMemo(() => parseCsv(content), [content]);

  function updateCell(r: number, c: number, value: string) {
    const next = rows.map((row) => [...row]);
    while (next.length <= r) next.push([""]);
    while (next[r].length <= c) next[r].push("");
    next[r][c] = value;
    onChange(next.map((row) => row.join(",")).join("\n"));
  }

  function addRow() {
    const cols = Math.max(1, ...rows.map((r) => r.length));
    onChange([...rows, Array.from({ length: cols }, () => "")].map((r) => r.join(",")).join("\n"));
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <table className="min-w-full border-collapse text-xs">
        <tbody>
          {rows.map((row, ri) => (
            <tr key={ri}>
              {row.map((cell, ci) => (
                <td key={ci} className="border border-zinc-800/70 p-0">
                  <input
                    value={cell}
                    onChange={(e) => updateCell(ri, ci, e.target.value)}
                    className="w-full min-w-[6rem] bg-transparent px-2 py-1.5 text-zinc-200 outline-none"
                  />
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      <button type="button" onClick={addRow} className="mt-2 text-xs text-blue-400 hover:underline">
        Add row
      </button>
    </div>
  );
}

function parseCsv(text: string): string[][] {
  const lines = text.split(/\r?\n/).filter((l, i, arr) => l.length > 0 || i < arr.length - 1);
  if (lines.length === 0) return [["", ""]];
  return lines.map((line) => line.split(","));
}
