# Buddy chat control plane

One path: Chat → `POST /chat` → `ControlPlane.handle_message` → one Cloud AI interpretation → deterministic operations → preview or commit.

## Stages

1. Receive the user message and persist a `turns` row keyed by `request_id`.
2. Load scoped context (history without the current line, calendar/sparks only when relevant). Open goals in the conversation are always included so follow-ups can target the intended goal.
3. One Groq JSON completion. A second call happens only if HTTP 200 JSON fails the BuddyTurn schema.
4. Parse a typed, versioned `operations[]` list (`schema_version` 2). Pre-v2 `goal_updates` / `requested_action` / `calendar_actions` are expanded once at parse time.
5. Classify each operation: commit, preview, or clarify.
6. Commit independent safe writes. Persist destructive calendar changes as a preview of exact session IDs and fingerprints until Approve.
7. Return `proposal_groups[]` (one card per goal/batch) and store the `ChatResponse` on the turn for idempotent replay.

Timezone for “today / tomorrow / tonight / weekend” is `Europe/London` (including BST).

Stop uses a dedicated HTTP client for that request. Stopping one conversation must not close a client another conversation is using. A stopped turn cannot enter commit. If commit already started, the client is told changes may have been applied. Usage counts physical provider requests only; Stop does not claim quota was saved.

## Operations

Each operation has `kind`, optional `target_type` / `target_id` / `target_ref`, `payload`, `assumptions`, `confidence`, and optional `disposition`. `goal_create` does not pause other goals. Sparks are their own operations. Calendar delete/move/update waits for approval and applies only reviewed IDs. Exact session complete/missed still commits immediately.

A turn may create three goals and propose a plan for only one of them. Follow-ups must match `open_goals` by title/domain; they must not assume the newest active goal is the target.

Unknown calendar ops and unknown `requested_action` types are validation failures, not silent `none`. Mixed success/failure is reported as a partial result, never as overall success.

Edit/resend uses `revision_of`. Before the new turn runs, later (and the original) turn `effects_json` are undone: created goals are removed, calendar rows are restored or deleted, and unapproved previews are cancelled. Edit is refused with HTTP 409 when a later session is already `completed` or `missed`, so the transcript cannot lie about calendar history.

`openai/gpt-oss-120b` cannot use Groq `json_object` mode (`json_validate_failed`). Completions are parsed from message text. Live three-goal dumps still often fail BuddyTurn validation after HTTP 200, so Chat is not ready to move onto the server.
