"""Local calendar categories — match session titles without Cloud AI."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


DEFAULT_CATEGORIES = [
    {
        "name": "Climbing",
        "color": "#c5d9a0",
        "icon": "mountain",
        "keywords": "climb,boulder,technique,endurance,power,route",
    },
    {
        "name": "Strength",
        "color": "#9dde9a",
        "icon": "barbell",
        "keywords": "strength,hangboard,fingerboard,pull-up,workout,core,antagonist",
    },
    {
        "name": "Fixed",
        "color": "#8fb9a8",
        "icon": "lock",
        "keywords": "work,sleep,fixed,commitment",
    },
    {
        "name": "Other",
        "color": "#eaf6cb",
        "icon": "circle",
        "keywords": "",
    },
]


class CategoryStore:
    def __init__(self, conn):
        self.conn = conn

    def ensure_defaults(self) -> None:
        row = self.conn.execute("SELECT COUNT(*) AS c FROM categories").fetchone()
        if row and row["c"] > 0:
            return
        now = _now()
        for i, spec in enumerate(DEFAULT_CATEGORIES):
            self.conn.execute(
                """
                INSERT INTO categories (
                    id, name, color, icon, keywords, sort_order, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    _new_id(),
                    spec["name"],
                    spec["color"],
                    spec["icon"],
                    spec["keywords"],
                    i,
                    now,
                    now,
                ),
            )
        self.conn.commit()

    def list(self) -> list[dict[str, Any]]:
        self.ensure_defaults()
        self.dedupe_by_name()
        rows = self.conn.execute(
            "SELECT * FROM categories ORDER BY sort_order ASC, name ASC"
        ).fetchall()
        return [self._row(r) for r in rows]

    def dedupe_by_name(self) -> None:
        """Keep one category per name (case-insensitive); remapping sessions to the keeper."""
        rows = self.conn.execute(
            "SELECT id, name, sort_order, created_at FROM categories ORDER BY sort_order ASC, created_at ASC"
        ).fetchall()
        keep: dict[str, str] = {}
        removed = False
        for r in rows:
            key = (r["name"] or "").strip().lower()
            if not key:
                continue
            if key not in keep:
                keep[key] = r["id"]
                continue
            keeper = keep[key]
            self.conn.execute(
                "UPDATE sessions SET category_id=? WHERE category_id=?",
                (keeper, r["id"]),
            )
            self.conn.execute("DELETE FROM categories WHERE id=?", (r["id"],))
            removed = True
        if removed:
            self.conn.commit()

    def create(
        self,
        *,
        name: str,
        color: str = "#93c5fd",
        icon: str = "circle",
        keywords: str = "",
    ) -> dict[str, Any]:
        self.ensure_defaults()
        clean = name.strip()
        existing = self.conn.execute(
            "SELECT id FROM categories WHERE lower(name)=lower(?) LIMIT 1",
            (clean,),
        ).fetchone()
        if existing:
            updated = self.update(
                existing["id"],
                name=clean,
                color=color,
                icon=icon,
                keywords=keywords,
            )
            return updated  # type: ignore[return-value]
        cid = _new_id()
        now = _now()
        sort = self.conn.execute("SELECT COALESCE(MAX(sort_order),0)+1 AS s FROM categories").fetchone()[
            "s"
        ]
        self.conn.execute(
            """
            INSERT INTO categories (
                id, name, color, icon, keywords, sort_order, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (cid, clean, color, icon, keywords.strip().lower(), sort, now, now),
        )
        self.conn.commit()
        return self.get(cid)  # type: ignore[return-value]

    def get(self, category_id: str) -> dict[str, Any] | None:
        row = self.conn.execute("SELECT * FROM categories WHERE id = ?", (category_id,)).fetchone()
        return self._row(row) if row else None

    def update(self, category_id: str, **fields: Any) -> dict[str, Any] | None:
        row = self.get(category_id)
        if not row:
            return None
        name = fields.get("name", row["name"])
        color = fields.get("color", row["color"])
        icon = fields.get("icon", row["icon"])
        keywords = fields.get("keywords", row["keywords"])
        self.conn.execute(
            """
            UPDATE categories
            SET name=?, color=?, icon=?, keywords=?, updated_at=?
            WHERE id=?
            """,
            (str(name).strip(), color, icon, str(keywords).strip().lower(), _now(), category_id),
        )
        self.conn.commit()
        return self.get(category_id)

    def delete(self, category_id: str) -> bool:
        row = self.get(category_id)
        if not row:
            return False
        # Keep sessions; clear category_id
        self.conn.execute("UPDATE sessions SET category_id=NULL WHERE category_id=?", (category_id,))
        self.conn.execute("DELETE FROM categories WHERE id=?", (category_id,))
        self.conn.commit()
        return True

    def match_title(self, title: str) -> str | None:
        """Pick the best category id for a session title using keyword hits."""
        cats = self.list()
        lower = title.lower()
        best_id = None
        best_key: tuple[int, int, int] | None = None
        other_id = None
        for cat in cats:
            if cat["name"].lower() == "other":
                other_id = cat["id"]
            keywords = [k.strip() for k in (cat["keywords"] or "").split(",") if k.strip()]
            if not keywords:
                continue
            hits = [kw for kw in keywords if kw in lower]
            if not hits:
                continue
            # Prefer longer keyword hits, then more hits, then tighter keyword lists
            key = (max(len(h) for h in hits), len(hits), -len(keywords))
            if best_key is None or key > best_key:
                best_key = key
                best_id = cat["id"]
        return best_id or other_id

    def classify_session_title(self, title: str) -> dict[str, Any] | None:
        cid = self.match_title(title)
        return self.get(cid) if cid else None

    def _row(self, row: Any) -> dict[str, Any]:
        d = dict(row)
        return {
            "id": d["id"],
            "name": d["name"],
            "color": d["color"],
            "icon": d.get("icon") or "circle",
            "keywords": d.get("keywords") or "",
            "sort_order": d.get("sort_order") or 0,
        }
