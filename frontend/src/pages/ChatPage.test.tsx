import { useMemo, useState } from "react";
import { fireEvent, render, screen, waitFor, cleanup } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ChatNavContext, type ChatNavValue } from "../chatNav";
import { GoalCompleteProvider } from "../components/ui/GoalCompleteOverlay";
import { ToastProvider } from "../components/ui/Toast";
import { ChatPage } from "./ChatPage";
import type { ChatResponse, Conversation, OpenProposal } from "../api";

const api = vi.hoisted(() => ({
  saveDraft: vi.fn<(id: string, draft: Record<string, unknown>) => Promise<{ id: string }>>(
    async () => ({ id: "conv-1" }),
  ),
  sendChat: vi.fn<() => Promise<ChatResponse>>(),
  fetchMessages: vi.fn<(id: string) => Promise<Array<{ id: string; role: string; content: string }>>>(
    async () => [],
  ),
  fetchOpenProposal: vi.fn<(id: string) => Promise<OpenProposal>>(),
  createConversation: vi.fn<() => Promise<Conversation>>(),
  cancelChat: vi.fn(),
  decideProposal: vi.fn(),
  notifyCalendarChanged: vi.fn(),
  buildProposalSummary: vi.fn(),
}));

vi.mock("../api", () => api);

const conversations: Conversation[] = [
  { id: "conv-1", title: "Climb", created_at: "", updated_at: "" },
  { id: "opened-chat", title: "Opened", created_at: "", updated_at: "" },
  { id: "created-from-new-chat", title: "Standing desk", created_at: "", updated_at: "" },
];

function emptyOpen(overrides: Partial<OpenProposal> = {}): OpenProposal {
  return {
    goal: null,
    proposed_sessions: [],
    proposal_summary: null,
    proposal_groups: [],
    clarification_questions: [],
    entered_answers: {},
    composer: "",
    ...overrides,
  };
}

function navValue(
  conversationId: string | null,
  setConversationId: (id: string | null) => void,
): ChatNavValue {
  return {
    conversations,
    folders: [],
    conversationId,
    search: "",
    setSearch: () => undefined,
    selectConversation: (id) => setConversationId(id),
    setConversationId,
    newChat: async () => conversations[2],
    renameChat: async () => undefined,
    deleteChat: async () => undefined,
    restoreChat: async () => undefined,
    moveChat: async () => undefined,
    placeChat: async () => undefined,
    addFolder: async () => ({
      id: "f",
      title: "F",
      sort_order: 0,
      created_at: "",
      updated_at: "",
    }),
    renameFolderTitle: async () => undefined,
    removeFolder: async () => undefined,
    refresh: async () => undefined,
  };
}

function Harness({ initialId }: { initialId: string | null }) {
  const [conversationId, setConversationId] = useState<string | null>(initialId);
  const value = useMemo(
    () => navValue(conversationId, setConversationId),
    [conversationId],
  );
  return (
    <ToastProvider>
      <GoalCompleteProvider>
        <ChatNavContext.Provider value={value}>
          <button type="button" data-testid="open-other" onClick={() => setConversationId("opened-chat")}>
            Open other
          </button>
          <button type="button" data-testid="open-climb" onClick={() => setConversationId("conv-1")}>
            Open climb
          </button>
          <ChatPage />
        </ChatNavContext.Provider>
      </GoalCompleteProvider>
    </ToastProvider>
  );
}

function lateResponse(): ChatResponse {
  return {
    conversation_id: "created-from-new-chat",
    reply: "LATE REPLY FROM NEW CHAT",
    goal: null,
    pending_question: null,
    activity: [],
    ai_available: true,
  };
}

describe("ChatPage full-cycle drafts and late replies", () => {
  afterEach(() => {
    cleanup();
  });

  beforeEach(() => {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: () => undefined,
    });
    localStorage.clear();
    vi.clearAllMocks();
    api.fetchOpenProposal.mockImplementation(async (id: string) => {
      if (id === "conv-1") {
        return emptyOpen({
          composer: "half a thought",
          clarification_questions: [
            {
              id: "grade",
              label: "What grade do you climb consistently?",
              help_text: "Your answer",
              answer_type: "short_text",
              required: false,
              options: [],
              suggested_answer: "V4",
            },
          ],
          entered_answers: { grade: "V3 indoors" },
        });
      }
      return emptyOpen({ composer: "from B" });
    });
    api.fetchMessages.mockResolvedValue([]);
    api.saveDraft.mockResolvedValue({ id: "ok" });
  });

  it("restores composer and answers, then saves empty clears", async () => {
    render(<Harness initialId="conv-1" />);
    const composer = await screen.findByLabelText("Message");
    await waitFor(() => expect(composer).toHaveValue("half a thought"));
    const answer = await screen.findByPlaceholderText("Your answer");
    expect(answer).toHaveValue("V3 indoors");

    fireEvent.change(answer, { target: { value: "" } });
    await waitFor(() =>
      expect(api.saveDraft).toHaveBeenCalledWith(
        "conv-1",
        expect.objectContaining({
          composer: "half a thought",
          clarification_answers: { grade: "" },
        }),
      ),
    );
  });

  it("flushes chat A drafts instead of writing them onto chat B", async () => {
    render(<Harness initialId="conv-1" />);
    const composer = await screen.findByLabelText("Message");
    await waitFor(() => expect(composer).toHaveValue("half a thought"));
    fireEvent.click(screen.getByTestId("open-other"));
    await waitFor(() =>
      expect(api.saveDraft).toHaveBeenCalledWith(
        "conv-1",
        expect.objectContaining({ composer: "half a thought" }),
      ),
    );
    await waitFor(() => expect(composer).toHaveValue("from B"));
    const leaked = api.saveDraft.mock.calls.some(([id, draft]) => {
      return id === "opened-chat" && draft?.composer === "half a thought";
    });
    expect(leaked).toBe(false);
  });

  it("keeps a late new-chat reply off the opened chat and offers Open", async () => {
    let release!: (value: ChatResponse) => void;
    api.sendChat.mockImplementation(
      () =>
        new Promise<ChatResponse>((resolve) => {
          release = resolve;
        }),
    );
    api.createConversation.mockResolvedValue({
      id: "created-from-new-chat",
      title: "Standing desk",
      created_at: "",
      updated_at: "",
    });
    api.fetchMessages.mockImplementation(async (id: string) => {
      if (id === "created-from-new-chat") {
        return [
          { id: "u", role: "user", content: "hello from blank" },
          { id: "a", role: "assistant", content: "LATE REPLY FROM NEW CHAT" },
        ];
      }
      return [{ id: "o", role: "user", content: "opened chat history" }];
    });

    render(<Harness initialId={null} />);
    const composer = screen.getByLabelText("Message");
    fireEvent.change(composer, { target: { value: "hello from blank" } });
    fireEvent.submit(composer.closest("form") as HTMLFormElement);
    await waitFor(() => expect(api.createConversation).toHaveBeenCalled());
    await waitFor(() => expect(api.sendChat).toHaveBeenCalled());
    fireEvent.click(screen.getByTestId("open-other"));
    await waitFor(() => screen.getByText("opened chat history"));
    release(lateResponse());
    await waitFor(() => screen.getByText("Buddy replied in Standing desk"));
    expect(screen.queryByText("LATE REPLY FROM NEW CHAT")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Open" }));
    await waitFor(() => screen.getByText("LATE REPLY FROM NEW CHAT"));
  });
});
