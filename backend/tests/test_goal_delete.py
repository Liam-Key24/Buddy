"""Deleting a goal cascades to sessions, sparks, and approval history."""

from __future__ import annotations

import json
import uuid
from datetime import datetime, timezone
from pathlib import Path

import pytest

from app.control_plane import ControlPlane


@pytest.fixture()
def plane(tmp_path: Path):
    svc = ControlPlane(db_path=tmp_path / "test.db", ai=None)
    yield svc
    svc.close()


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def test_remove_goal_cascades_sessions_and_sparks(plane: ControlPlane):
    conv = plane.conversations.create()
    goal = plane.goals.create(
        conv["id"],
        title="Climb V6",
        domain="climbing",
        status="active",
    )
    goal_id = goal.id
    now = _now()
    batch_id = str(uuid.uuid4())

    statuses = ("proposed", "scheduled", "completed", "missed", "rejected")
    for i, status in enumerate(statuses):
        plane.conn.execute(
            """
            INSERT INTO sessions (
                id, goal_id, title, start_at, end_at, kind, status,
                proposal_batch_id, notes, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 'flexible', ?, ?, NULL, ?, ?)
            """,
            (
                str(uuid.uuid4()),
                goal_id,
                f"Session {status}",
                f"2026-10-{10 + i:02d}T17:00:00",
                f"2026-10-{10 + i:02d}T18:00:00",
                status,
                batch_id if status == "proposed" else None,
                now,
                now,
            ),
        )
    plane.conn.execute(
        """
        INSERT INTO approval_events (
            id, batch_id, goal_id, conversation_id, approved_at, undone_at, session_ids_json
        ) VALUES (?, ?, ?, ?, ?, NULL, ?)
        """,
        (str(uuid.uuid4()), batch_id, goal_id, conv["id"], now, json.dumps([])),
    )
    plane.conn.commit()

    spark = plane.sparks.capture("try board training notes")
    plane.sparks.promote(spark.id, goal_id)

    assert (
        plane.conn.execute(
            "SELECT COUNT(*) AS c FROM sessions WHERE goal_id=?", (goal_id,)
        ).fetchone()["c"]
        == len(statuses)
    )

    removed = plane.remove_goal(goal_id)
    assert removed is not None
    assert removed.id == goal_id
    assert plane.get_goal(goal_id) is None
    assert all(g.id != goal_id for g in plane.list_goals())

    assert (
        plane.conn.execute(
            "SELECT COUNT(*) AS c FROM sessions WHERE goal_id=?", (goal_id,)
        ).fetchone()["c"]
        == 0
    )
    assert (
        plane.conn.execute(
            "SELECT COUNT(*) AS c FROM approval_events WHERE goal_id=?", (goal_id,)
        ).fetchone()["c"]
        == 0
    )

    refreshed = plane.conn.execute(
        "SELECT status, promoted_goal_id FROM sparks WHERE id=?", (spark.id,)
    ).fetchone()
    assert refreshed["promoted_goal_id"] is None
    assert refreshed["status"] == "open"


def test_list_managed_keeps_completed_hides_deleted_chat_goals(plane: ControlPlane):
    live = plane.conversations.create()
    gone = plane.conversations.create()
    completed = plane.goals.create(live["id"], title="Reached V6", status="done")
    open_goal = plane.goals.create(live["id"], title="Keep climbing", status="active", pause_others=False)
    leftover = plane.goals.create(gone["id"], title="Abandoned", status="active")
    plane.delete_conversation(gone["id"])

    listed = {g.id: g for g in plane.list_goals()}
    assert completed.id in listed
    assert listed[completed.id].status == "done"
    assert open_goal.id in listed
    assert leftover.id not in listed

    plane.remove_goal(completed.id)
    assert all(g.id != completed.id for g in plane.list_goals())


def test_list_goals_includes_event_stats(plane: ControlPlane):
    conv = plane.conversations.create()
    goal = plane.goals.create(
        conv["id"],
        title="Climb V6",
        target="V6",
        status="done",
    )
    now = _now()
    for i, status in enumerate(("completed", "completed", "missed")):
        plane.conn.execute(
            """
            INSERT INTO sessions (
                id, goal_id, title, start_at, end_at, kind, status,
                proposal_batch_id, notes, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, 'flexible', ?, NULL, NULL, ?, ?)
            """,
            (
                str(uuid.uuid4()),
                goal.id,
                f"Climb {status}",
                f"2026-03-{10 + i:02d}T17:00:00",
                f"2026-03-{10 + i:02d}T18:30:00",
                status,
                now,
                now,
            ),
        )
    plane.conn.commit()

    listed = {g.id: g for g in plane.list_goals()}
    row = listed[goal.id]
    assert row.events_total == 3
    assert row.events_completed == 2
    assert row.events_missed == 1
    assert row.started_at and row.started_at.startswith("2026-03-10")
    assert row.ended_at and row.ended_at.startswith("2026-03-12")


def test_migration_purges_archived_done_goals(tmp_path: Path):
    from app.db import get_connection, init_db
    from app.migrations import SCHEMA_VERSION, run_migrations, set_schema_version

    conn = get_connection(tmp_path / "legacy-done.db")
    init_db(conn)
    now = _now()
    conv_id = str(uuid.uuid4())
    goal_id = str(uuid.uuid4())
    conn.execute(
        """
        INSERT INTO conversations (id, created_at, updated_at, title, deleted_at, draft_json, sort_order)
        VALUES (?, ?, ?, 'old chat', NULL, '{}', 0)
        """,
        (conv_id, now, now),
    )
    conn.execute(
        """
        INSERT INTO goals (
            id, conversation_id, title, domain, target, deadline,
            baseline, frequency, commitment, status, facts_json, created_at, updated_at
        ) VALUES (?, ?, 'Archived climb', NULL, NULL, NULL, NULL, NULL, NULL, 'done', '{}', ?, ?)
        """,
        (goal_id, conv_id, now, now),
    )
    conn.commit()
    set_schema_version(conn, 5)
    assert SCHEMA_VERSION >= 6
    run_migrations(conn)
    gone = conn.execute("SELECT id FROM goals WHERE id=?", (goal_id,)).fetchone()
    assert gone is None
    conn.close()
