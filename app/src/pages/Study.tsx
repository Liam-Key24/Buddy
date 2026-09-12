import { useState } from "react";
import {
  Clock,
  Exam,
  Notebook,
  NotePencil,
  Plus,
  Trash,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { ToolWorkspace } from "../components/ToolWorkspace";
import { IconToggleGroup } from "../components/life/IconToggleGroup";
import { WorkspaceNavItem, WorkspacePaneHeader } from "../components/life/WorkspaceChrome";
import { TASK_STATUSES } from "../components/life/statusOptions";
import { useLifePage } from "../hooks/useLifePage";
import { shortDate } from "../lib/dates";
import {
  studyDeleteSubject,
  studyListAssignments,
  studyListSubjects,
  studyListTopics,
  studyUpsertAssignment,
  studyUpsertSubject,
  studyUpsertTopic,
  type StudyAssignment,
  type StudySubject,
  type StudyTopic,
  type TopicWithRisk,
} from "../lib/lifeApi";

function riskDot(risk: string) {
  if (risk === "unlikely") return "bg-rose-400";
  if (risk === "at_risk") return "bg-amber-400";
  return "bg-emerald-400";
}

export function Study() {
  const [subjects, setSubjects] = useState<StudySubject[]>([]);
  const [subjectId, setSubjectId] = useState<string | null>(null);
  const [topics, setTopics] = useState<TopicWithRisk[]>([]);
  const [assignments, setAssignments] = useState<StudyAssignment[]>([]);
  const [newSubject, setNewSubject] = useState("");

  async function reload() {
    const s = await studyListSubjects();
    setSubjects(s);
    const sid = subjectId ?? s[0]?.id ?? null;
    if (!subjectId && sid) setSubjectId(sid);
    const [t, a] = await Promise.all([
      studyListTopics(sid),
      studyListAssignments(sid),
    ]);
    setTopics(t);
    setAssignments(a);
  }

  const name = subjects.find((s) => s.id === subjectId)?.name;
  useLifePage({
    event: "study-updated",
    load: reload,
    loadDeps: [subjectId],
    context: name
      ? `Page: study. Active subject "${name}" (id=${subjectId}). Read with study.status / study.look (sessions|topics|assignments).`
      : "Page: study. No subject selected. Read with study.status / study.look.",
  });

  async function addSubject() {
    if (!newSubject.trim()) return;
    const s = await studyUpsertSubject(newSubject.trim());
    setNewSubject("");
    setSubjectId(s.id);
    await reload();
  }

  return (
    <LifeChatSplit>
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <ToolWorkspace
        narrow
        sidebar={
          <>
            <div className="relative">
              <input
                value={newSubject}
                onChange={(e) => setNewSubject(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    void addSubject();
                  }
                }}
                placeholder="Subject"
                className="w-full rounded-xl border border-zinc-800 bg-zinc-950 py-1.5 pl-2 pr-7 text-xs text-zinc-200 outline-none"
              />
              <button
                type="button"
                title="Add subject"
                aria-label="Add subject"
                onClick={() => void addSubject()}
                className="absolute right-1 top-1/2 flex h-5 w-5 -translate-y-1/2 items-center justify-center rounded-md bg-blue-500 text-white"
              >
                <Plus size={11} weight="bold" />
              </button>
            </div>
            {subjects.map((s) => (
              <WorkspaceNavItem key={s.id} active={subjectId === s.id} onClick={() => setSubjectId(s.id)}>
                {s.name}
              </WorkspaceNavItem>
            ))}
          </>
        }
      >
        <WorkspacePaneHeader Icon={Notebook}>
          <div className="min-w-0 flex-1">
            <h3 className="text-sm font-medium text-zinc-200">Study sheet</h3>
            {subjectId && (
              <p className="truncate text-xs text-zinc-500">
                {subjects.find((s) => s.id === subjectId)?.name}
              </p>
            )}
          </div>
          {subjectId && (
            <button
              type="button"
              title="Delete subject"
              aria-label="Delete subject"
              onClick={() => studyDeleteSubject(subjectId).then(() => { setSubjectId(null); reload(); })}
              className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-rose-400"
            >
              <Trash size={15} />
            </button>
          )}
        </WorkspacePaneHeader>
        {!subjectId ? (
          <div className="min-h-0 flex-1 rounded-xl border border-dashed border-zinc-800" />
        ) : (
          <div className="min-h-0 flex-1 space-y-4 overflow-y-auto">
            <section>
              <div className="mb-2 flex items-center justify-between">
                <h4 className="text-xs uppercase tracking-wider text-zinc-500">Topics</h4>
                <button
                  type="button"
                  title="Add topic"
                  aria-label="Add topic"
                  onClick={async () => {
                    const t: StudyTopic = {
                      id: "",
                      subject_id: subjectId,
                      name: "New topic",
                      status: "not_started",
                      last_studied: null,
                      deadline: null,
                      priority: "medium",
                      remaining_estimate: 2,
                      notes: null,
                      created_at: 0,
                      updated_at: 0,
                    };
                    await studyUpsertTopic(t);
                    await reload();
                  }}
                  className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
                >
                  <Plus size={14} weight="bold" />
                </button>
              </div>
              <div className="space-y-1.5">
                {topics.length === 0 && (
                  <p className="px-1 text-xs text-zinc-600">No topics yet</p>
                )}
                {topics.map(({ topic, risk, reason }) => (
                  <div
                    key={topic.id}
                    className="flex items-center gap-2 rounded-xl border border-zinc-800 bg-zinc-950/40 px-2.5 py-2"
                  >
                    <input
                      defaultValue={topic.name}
                      onBlur={(e) => studyUpsertTopic({ ...topic, name: e.target.value }).then(reload)}
                      className="min-w-0 flex-1 bg-transparent text-sm text-zinc-200 outline-none"
                    />
                    <StatusIcons
                      value={topic.status}
                      onChange={(status) => studyUpsertTopic({ ...topic, status }).then(reload)}
                    />
                    <span
                      title={topic.last_studied ? `Last studied ${shortDate(topic.last_studied)}` : "Not studied yet"}
                      className="flex shrink-0 items-center gap-1 text-[11px] text-zinc-500"
                    >
                      <Clock size={12} />
                      {shortDate(topic.last_studied) ?? "—"}
                    </span>
                    <span title={reason} className={`h-2 w-2 shrink-0 rounded-full ${riskDot(risk)}`} />
                  </div>
                ))}
              </div>
            </section>
            <section>
              <div className="mb-2 flex items-center justify-between">
                <h4 className="text-xs uppercase tracking-wider text-zinc-500">Assignments & exams</h4>
                <button
                  type="button"
                  title="Add assignment"
                  aria-label="Add assignment"
                  onClick={async () => {
                    await studyUpsertAssignment({
                      id: "",
                      subject_id: subjectId,
                      topic_id: null,
                      title: "New assignment",
                      kind: "assignment",
                      status: "not_started",
                      deadline: null,
                      priority: "medium",
                      notes: null,
                      created_at: 0,
                      updated_at: 0,
                    });
                    await reload();
                  }}
                  className="rounded-md p-1 text-blue-400 hover:bg-zinc-800"
                >
                  <Plus size={14} weight="bold" />
                </button>
              </div>
              <div className="space-y-2">
                {assignments.length === 0 && (
                  <p className="px-1 text-xs text-zinc-600">None yet</p>
                )}
                {assignments.map((a) => (
                  <AssignmentCard key={a.id} assignment={a} onChange={reload} />
                ))}
              </div>
            </section>
          </div>
        )}
      </ToolWorkspace>
    </div>
    </LifeChatSplit>
  );
}

function StatusIcons({
  value,
  onChange,
}: {
  value: string;
  onChange: (status: string) => void;
}) {
  return <IconToggleGroup value={value} onChange={onChange} items={TASK_STATUSES} />;
}

function AssignmentCard({
  assignment,
  onChange,
}: {
  assignment: StudyAssignment;
  onChange: () => void;
}) {
  const isExam = assignment.kind === "exam";
  return (
    <div className="flex items-center gap-2.5 rounded-xl border border-zinc-800 bg-zinc-950/40 px-3 py-2.5">
      <button
        type="button"
        title={isExam ? "Exam — click to set assignment" : "Assignment — click to set exam"}
        aria-label={isExam ? "Exam" : "Assignment"}
        onClick={() =>
          studyUpsertAssignment({
            ...assignment,
            kind: isExam ? "assignment" : "exam",
          }).then(onChange)
        }
        className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-lg ${
          isExam ? "bg-amber-500/15 text-amber-400" : "bg-blue-500/15 text-blue-400"
        }`}
      >
        {isExam ? <Exam size={16} weight="fill" /> : <NotePencil size={16} weight="fill" />}
      </button>
      <div className="min-w-0 flex-1">
        <input
          defaultValue={assignment.title}
          onBlur={(e) => studyUpsertAssignment({ ...assignment, title: e.target.value }).then(onChange)}
          className="w-full bg-transparent text-sm text-zinc-100 outline-none"
        />
        <p className="text-[11px] text-zinc-500">{isExam ? "Exam" : "Assignment"}</p>
      </div>
      <StatusIcons
        value={assignment.status}
        onChange={(status) => studyUpsertAssignment({ ...assignment, status }).then(onChange)}
      />
    </div>
  );
}
