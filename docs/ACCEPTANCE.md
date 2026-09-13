# Buddy manual acceptance

Use a local backend with a mock or real Groq key. Prefer a disposable `BUDDY_DB_PATH` for the walkthrough if you do not want to touch `backend/data/buddy.db`.

## Setup

```bash
cd backend && source .venv/bin/activate
export BUDDY_DB_PATH=/tmp/buddy-acceptance.db
export GROQ_API_KEY=...   # or run automated tests which mock Groq
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787
cd ../frontend && npm run dev
```

Open http://127.0.0.1:5173

## Journey

1. **Chat** → “I want to climb V6 by the end of November.”
   - Expect one clarification about current grade. One goal created.
2. Reply “I’m climbing V4 consistently, twice a week.”
   - Same goal id. Baseline V4, frequency twice a week.
3. Agree to calendar → “Yes, look at the calendar.”
   - Proposal card appears. Nothing booked yet.
4. Click **Approve** (do not need to type Approve).
   - Sessions become scheduled. Calendar and Today update.
5. Click Approve again → no duplicates.
6. On Calendar, open a scheduled session → Mark completed / missed.
7. Today progress text updates (completed/missed counts).
8. New goal: “Finish Mevero by end of December, about 80% done.”
   - Climbing goal pauses. Chat follows Mevero.
9. Sparks → save an idea → Promote to Chat (starts goal talk, does not auto-schedule).
10. Quit backend → Chat explains Cloud AI unavailable; Today/Calendar/Sparks still load.

## Packaging smoke (after Stage 10)

1. Launch Buddy.app from Finder.
2. Confirm one localhost backend starts.
3. Quit app → backend process exits.
4. Confirm no `GROQ_API_KEY` in bundle or frontend network payloads.
