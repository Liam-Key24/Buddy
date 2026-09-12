import { useState } from "react";
import {
  Archive,
  ArrowRight,
  CheckCircle,
  Clock,
  Copy,
  LinkedinLogo,
  PaperPlaneTilt,
  ShareNetwork,
  XCircle,
  XLogo,
} from "@phosphor-icons/react";
import { LifeChatSplit } from "../components/LifeChatSplit";
import { ToolWorkspace } from "../components/ToolWorkspace";
import { IconToggleGroup } from "../components/life/IconToggleGroup";
import { WorkspaceNavItem, WorkspacePaneHeader } from "../components/life/WorkspaceChrome";
import { useLifePage } from "../hooks/useLifePage";
import {
  socialsArchivePost,
  socialsCommitApproved,
  socialsDeleteDraft,
  socialsDeleteIdea,
  socialsDeleteProject,
  socialsGeneratePosts,
  socialsGetPlan,
  socialsListDrafts,
  socialsListIdeas,
  socialsListProjects,
  socialsListThreads,
  socialsMarkPublished,
  socialsProfile,
  socialsPublished,
  socialsUpdatePost,
  socialsUpdateProfile,
  socialsUpdateThread,
  socialsUpsertDraft,
  socialsUpsertIdea,
  socialsUpsertProject,
  type SocialDraft,
  type SocialIdea,
  type SocialPost,
  type SocialProfile,
  type SocialProject,
  type SocialThread,
  type SocialWeeklyPlan,
} from "../lib/lifeApi";

const SECTIONS = [
  "Current Story",
  "Active Projects",
  "Content Ideas",
  "Drafts",
  "Weekly Review",
  "Published Posts",
  "Growth Insights",
] as const;
type Section = (typeof SECTIONS)[number];

export function Socials() {
  const [section, setSection] = useState<Section>("Weekly Review");
  const [profile, setProfile] = useState<SocialProfile | null>(null);
  const [threads, setThreads] = useState<SocialThread[]>([]);
  const [projects, setProjects] = useState<SocialProject[]>([]);
  const [ideas, setIdeas] = useState<SocialIdea[]>([]);
  const [drafts, setDrafts] = useState<SocialDraft[]>([]);
  const [plan, setPlan] = useState<SocialWeeklyPlan | null>(null);
  const [published, setPublished] = useState<SocialPost[]>([]);
  const [notes, setNotes] = useState("");
  const [commitMsg, setCommitMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState<"generate" | "remake" | null>(null);
  const [genError, setGenError] = useState<string | null>(null);

  async function reload() {
    const [p, t, pr, i, d, pl, pub] = await Promise.all([
      socialsProfile(),
      socialsListThreads(),
      socialsListProjects(),
      socialsListIdeas(),
      socialsListDrafts(),
      socialsGetPlan(),
      socialsPublished(),
    ]);
    setProfile(p);
    setThreads(t);
    setProjects(pr);
    setIdeas(i);
    setDrafts(d);
    setPlan(pl);
    setPublished(pub);
    if (pl?.last_week_notes) setNotes(pl.last_week_notes);
  }

  useLifePage({
    event: "socials-updated",
    load: reload,
    context: `Page: socials. Section ${section}. Week ${plan?.week_start ?? "none"}. Generate/Remake in Weekly Review uses the model with current story, active projects, unused ideas. Archive post cards into Drafts. Read with socials.look. Write with update_post / commit_approved. Nothing to Calendar until approved.`,
  });

  return (
    <LifeChatSplit>
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <ToolWorkspace
        sidebar={
          <>
            {SECTIONS.map((s) => (
              <WorkspaceNavItem key={s} active={section === s} onClick={() => setSection(s)}>
                {s}
              </WorkspaceNavItem>
            ))}
          </>
        }
      >
        <WorkspacePaneHeader Icon={ShareNetwork} iconClassName="text-pink-400" title={section} />
        <div className="min-h-0 flex-1 overflow-y-auto pr-1 text-sm">
          {section === "Current Story" && profile && (
            <div className="space-y-3">
              <textarea
                defaultValue={profile.narrative}
                onBlur={(e) => socialsUpdateProfile(e.target.value, profile.tone_notes).then(reload)}
                rows={4}
                className="w-full rounded-xl border border-zinc-800 bg-zinc-950 px-3 py-2 text-sm text-zinc-200"
              />
              <textarea
                defaultValue={profile.tone_notes}
                onBlur={(e) => socialsUpdateProfile(profile.narrative, e.target.value).then(reload)}
                rows={3}
                className="w-full rounded-xl border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-400"
              />
              {threads.map((t) => (
                <label key={t.id} className="block">
                  <span className="text-[10px] uppercase tracking-wider text-zinc-500">{t.name}</span>
                  <input
                    defaultValue={t.current_chapter}
                    onBlur={(e) => socialsUpdateThread(t.id, e.target.value).then(reload)}
                    placeholder="Current chapter"
                    className="mt-1 w-full rounded-xl border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-200"
                  />
                </label>
              ))}
            </div>
          )}
          {section === "Active Projects" && (
            <SimpleList
              items={projects.map((p) => ({ id: p.id, title: p.name, body: p.notes }))}
              onAdd={async (title) => {
                await socialsUpsertProject({ id: "", name: title, status: "active", notes: "", created_at: 0, updated_at: 0 });
                await reload();
              }}
              onDelete={(id) => socialsDeleteProject(id).then(reload)}
            />
          )}
          {section === "Content Ideas" && (
            <SimpleList
              items={ideas.map((i) => ({
                id: i.id,
                title: i.body,
                body: i.used_at ? "Used" : "",
              }))}
              onAdd={async (title) => {
                await socialsUpsertIdea({ id: "", body: title, platform: null, thread_id: null, created_at: 0, updated_at: 0 });
                await reload();
              }}
              onDelete={(id) => socialsDeleteIdea(id).then(reload)}
            />
          )}
          {section === "Drafts" && (
            <SimpleList
              items={drafts.map((d) => ({
                id: d.id,
                title: d.title || d.platform,
                body: [d.archived ? "Archived" : "", d.body].filter(Boolean).join(" · "),
              }))}
              onAdd={async (title) => {
                await socialsUpsertDraft({ id: "", title, body: "", platform: "x", thread_id: null, created_at: 0, updated_at: 0 });
                await reload();
              }}
              onDelete={(id) => socialsDeleteDraft(id).then(reload)}
            />
          )}
          {section === "Weekly Review" && (
            <div className="space-y-3">
              <textarea
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="Notes or edits — used before Generate and when Remake revises posts"
                rows={3}
                className="w-full rounded-xl border border-zinc-800 bg-zinc-950 px-3 py-2 text-xs text-zinc-200"
              />
              <div className="flex flex-wrap items-center gap-2">
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={async () => {
                    setGenError(null);
                    setCommitMsg(null);
                    setBusy("generate");
                    try {
                      const p = await socialsGeneratePosts(null, notes, false);
                      setPlan(p);
                      await reload();
                    } catch (e) {
                      setGenError(e instanceof Error ? e.message : String(e));
                      await reload();
                    } finally {
                      setBusy(null);
                    }
                  }}
                  className="shrink-0 rounded-xl bg-blue-500 px-3 py-1.5 text-xs text-white disabled:opacity-40"
                >
                  {busy === "generate" ? "Generating…" : "Generate"}
                </button>
                <button
                  type="button"
                  disabled={busy !== null || !plan}
                  onClick={async () => {
                    setGenError(null);
                    setCommitMsg(null);
                    setBusy("remake");
                    try {
                      const p = await socialsGeneratePosts(null, notes, true);
                      setPlan(p);
                      await reload();
                    } catch (e) {
                      setGenError(e instanceof Error ? e.message : String(e));
                      await reload();
                    } finally {
                      setBusy(null);
                    }
                  }}
                  className="shrink-0 rounded-xl bg-zinc-800 px-3 py-1.5 text-xs text-zinc-200 disabled:opacity-40"
                >
                  {busy === "remake" ? "Remaking…" : "Remake"}
                </button>
                {plan && (
                  <button
                    type="button"
                    disabled={busy !== null}
                    onClick={async () => {
                      const res = await socialsCommitApproved(plan.id);
                      setCommitMsg(`${res.created} pinned`);
                      await reload();
                    }}
                    className="shrink-0 rounded-xl bg-zinc-800 px-3 py-1.5 text-xs text-zinc-200 disabled:opacity-40"
                  >
                    Commit
                  </button>
                )}
              </div>
              {genError && <p className="text-xs text-rose-400">{genError}</p>}
              {busy && (
                <p className="text-xs text-zinc-500">Using story, active projects, and unused ideas…</p>
              )}
              {plan && (
                <div className="flex items-center justify-between text-xs text-zinc-400">
                  <span className="flex items-center gap-1" title="LinkedIn">
                    <LinkedinLogo size={14} weight="fill" className="text-[#0A66C2]" />
                    {plan.posts.filter((p) => p.platform === "linkedin").length}
                  </span>
                  <span className="flex items-center gap-1" title="X">
                    <XLogo size={14} weight="fill" />
                    {plan.posts.filter((p) => p.platform === "x").length}
                  </span>
                </div>
              )}
              {commitMsg && <p className="text-xs text-emerald-400">{commitMsg}</p>}
              {plan && (
                <div className="space-y-2">
                  {plan.posts.filter((p) => p.platform !== "github").map((post) => (
                    <PostCard
                      key={`${post.id}:${post.updated_at}`}
                      post={post}
                      onChange={async (next) => {
                        await socialsUpdatePost(next);
                        await reload();
                      }}
                      onArchive={async () => {
                        await socialsArchivePost(post.id);
                        await reload();
                      }}
                    />
                  ))}
                </div>
              )}
            </div>
          )}
          {section === "Published Posts" && (
            <div className="space-y-2">
              {published.map((p) => (
                <div key={p.id} className="rounded-xl border border-zinc-800 p-3 text-xs text-zinc-300">
                  <p className="text-zinc-500">{p.slot_date} · {p.platform}</p>
                  <p className="mt-1 whitespace-pre-wrap">{p.body}</p>
                </div>
              ))}
              {published.length === 0 && <p className="text-zinc-500">None yet</p>}
            </div>
          )}
          {section === "Growth Insights" && <Growth posts={published} />}
        </div>
      </ToolWorkspace>
    </div>
    </LifeChatSplit>
  );
}

function slotLabel(date: string, time: string) {
  const d = new Date(`${date}T00:00:00`);
  if (Number.isNaN(d.getTime())) return time;
  const day = d.toLocaleDateString(undefined, { day: "numeric", month: "short" });
  return time ? `${day} ${time.slice(0, 5)}` : day;
}

const STATUSES = [
  { id: "proposed", label: "Proposed", Icon: Clock, active: "text-amber-400" },
  { id: "approved", label: "Approved", Icon: CheckCircle, active: "text-emerald-400" },
  { id: "rejected", label: "Rejected", Icon: XCircle, active: "text-rose-400" },
  { id: "published", label: "Published", Icon: PaperPlaneTilt, active: "text-blue-400" },
] as const;

function PostCard({
  post,
  onChange,
  onArchive,
}: {
  post: SocialPost;
  onChange: (p: SocialPost) => void;
  onArchive: () => void;
}) {
  const [copied, setCopied] = useState(false);
  const PlatformIcon = post.platform === "linkedin" ? LinkedinLogo : XLogo;

  async function copyPost() {
    const text = post.body.trim();
    if (!text) return;
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1200);
    } catch {
      /* ignore */
    }
  }

  return (
    <div className="rounded-xl border border-zinc-800 p-2.5">
      <div className="mb-1 flex items-center justify-end gap-0.5" role="group" aria-label="Status">
        <IconToggleGroup
          value={post.status}
          onChange={(id) => onChange({ ...post, status: id })}
          items={STATUSES}
          size={16}
        />
      </div>
      <textarea
        defaultValue={post.body}
        onBlur={(e) => onChange({ ...post, body: e.target.value })}
        rows={4}
        placeholder="Post"
        className="w-full resize-y rounded-lg bg-zinc-950 px-2.5 py-2 text-sm text-zinc-200 outline-none"
      />
      {post.suggested_media ? (
        <input
          defaultValue={post.suggested_media}
          onBlur={(e) => onChange({ ...post, suggested_media: e.target.value })}
          placeholder="Media"
          className="mt-1 w-full bg-transparent text-xs text-zinc-500 outline-none"
        />
      ) : null}
      <div className="mt-1.5 flex items-center justify-between gap-2">
        <PlatformIcon
          size={16}
          weight="fill"
          className={post.platform === "linkedin" ? "text-[#0A66C2]" : "text-zinc-300"}
        />
        <span className="text-xs text-zinc-500">
          {slotLabel(post.slot_date, post.slot_time)}
        </span>
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            title="Archive to drafts"
            aria-label="Archive to drafts"
            onClick={() => void onArchive()}
            className="rounded-md p-1 text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-200"
          >
            <Archive size={14} />
          </button>
          <button
            type="button"
            title="Copy"
            aria-label="Copy"
            onClick={() => void copyPost()}
            className="rounded-md p-1 text-zinc-500 transition hover:bg-zinc-800 hover:text-zinc-200"
          >
            <Copy size={14} weight={copied ? "fill" : "regular"} />
          </button>
        </div>
      </div>
      {post.calendar_event_id && (
        <button type="button" onClick={() => socialsMarkPublished(post.id)} className="mt-1 text-xs text-emerald-400">
          Published
        </button>
      )}
    </div>
  );
}

function SimpleList({
  items,
  onAdd,
  onDelete,
}: {
  items: { id: string; title: string; body: string }[];
  onAdd: (title: string) => Promise<void>;
  onDelete: (id: string) => void;
}) {
  const [title, setTitle] = useState("");

  async function submit() {
    const next = title.trim();
    if (!next) return;
    await onAdd(next);
    setTitle("");
  }

  return (
    <div className="space-y-2">
      <div className="relative">
        <input
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              void submit();
            }
          }}
          className="w-full rounded-xl border border-zinc-800 bg-zinc-950 py-1.5 pl-2.5 pr-8 text-xs text-zinc-200 outline-none"
        />
        <button
          type="button"
          title="Add"
          aria-label="Add"
          onClick={() => void submit()}
          disabled={!title.trim()}
          className="absolute right-1 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-lg bg-blue-500 text-white disabled:opacity-40"
        >
          <ArrowRight size={12} weight="bold" />
        </button>
      </div>
      {items.map((i) => (
        <div key={i.id} className="flex justify-between rounded-xl border border-zinc-800 px-3 py-2 text-xs">
          <div>
            <p className="text-zinc-200">{i.title}</p>
            {i.body && <p className="text-zinc-500">{i.body}</p>}
          </div>
          <button type="button" onClick={() => onDelete(i.id)} className="text-rose-400">×</button>
        </div>
      ))}
    </div>
  );
}

function Growth({ posts }: { posts: SocialPost[] }) {
  const byCat: Record<string, number> = {};
  for (const p of posts) {
    const c = p.category || "uncategorized";
    byCat[c] = (byCat[c] || 0) + 1;
  }
  return (
    <div className="space-y-2 text-xs text-zinc-400">
      <p>{posts.length} published</p>
      {Object.entries(byCat).map(([c, n]) => (
        <p key={c}>{c}: {n}</p>
      ))}
    </div>
  );
}
