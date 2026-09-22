# Buddy chat control plane

One path: Chat → `POST /chat` → `ControlPlane.handle_message` → one Cloud AI interpretation → deterministic operations → preview or commit.

## Stages

1. Receive the user message and persist a `turns` row keyed by `request_id`.
2. Load scoped context (history without the current line, calendar/sparks only when relevant).
3. One Groq JSON completion. A second call happens only if HTTP 200 JSON fails the BuddyTurn schema.
4. Expand the turn into ordered operations.
5. Classify each operation: commit, preview, or clarify.
6. Commit safe writes. Persist destructive calendar changes as a preview until Approve.
7. Store the `ChatResponse` on the turn for idempotent replay.

Timezone for “today / tomorrow / tonight / weekend” is `Europe/London` (including BST).

Stop cancels the in-flight Groq client when possible. If commit already started, the client is told changes were saved.

## Operations

`goal_create` does not pause other goals. Sparks are their own operations. Calendar delete/move/update waits for approval. Exact session complete/missed still commits immediately.

Unknown calendar ops and unknown `requested_action` types are validation failures, not silent `none`.
