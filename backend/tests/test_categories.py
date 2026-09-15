"""Category title matching."""

from __future__ import annotations

from app.categories import CategoryStore
from app.db import get_connection, init_db
from app.migrations import run_migrations


def test_category_match_climbing_vs_strength(tmp_path):
    conn = get_connection(tmp_path / "cat.db")
    init_db(conn)
    run_migrations(conn)
    store = CategoryStore(conn)
    store.ensure_defaults()

    climb = store.classify_session_title("Climb · Technique · Project V6")
    strength = store.classify_session_title("Climb strength · Hangboard + pull-ups")
    assert climb is not None and climb["name"] == "Climbing"
    assert strength is not None and strength["name"] == "Strength"

    # Custom category wins on a hangboard-only title (tighter keyword list)
    store.create(
        name="Hangboard",
        color="#f9a8d4",
        icon="timer",
        keywords="hangboard,fingerboard",
    )
    hang = store.classify_session_title("Hangboard · 7s hangs")
    assert hang is not None and hang["name"] == "Hangboard"
    conn.close()
