"""Local calendar categories — match session titles without Cloud AI."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any

from .owners import resolve_owner


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


DEFAULT_CATEGORIES = [
    {
        "name": "Climbing",
        "color": "#7eb8da",
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
        "color": "#e8c56b",
        "icon": "circle",
        "keywords": "",
    },
]


class CategoryStore:
    def __init__(self, conn):
        self.conn = conn

    def _oid(self, owner_user_id: str | None = None) -> str:
        return resolve_owner(self.conn, owner_user_id)

    def ensure_defaults(self, owner_user_id: str | None = None) -> None:
        from .auth import list_users

        if owner_user_id:
            owners = [owner_user_id]
        else:
            owners = [u["id"] for u in list_users(self.conn)]
            if not owners:
                owners = [self._oid()]
        for oid in owners:
            self._ensure_defaults_for(oid)

    def _ensure_defaults_for(self, owner_user_id: str) -> None:
        row = self.conn.execute(
            "SELECT COUNT(*) AS c FROM categories WHERE owner_user_id=?",
            (owner_user_id,),
        ).fetchone()
        if row and row["c"] > 0:
            return
        now = _now()
        for i, spec in enumerate(DEFAULT_CATEGORIES):
            self.conn.execute(
                """
                INSERT INTO categories (
                    id, name, color, icon, keywords, sort_order, created_at, updated_at, owner_user_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
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
                    owner_user_id,
                ),
            )
        self.conn.commit()

    def list(self, owner_user_id: str | None = None) -> list[dict[str, Any]]:
        oid = self._oid(owner_user_id)
        self.ensure_defaults(oid)
        self.dedupe_by_name(oid)
        rows = self.conn.execute(
            "SELECT * FROM categories WHERE owner_user_id=? ORDER BY sort_order ASC, name ASC",
            (oid,),
        ).fetchall()
        return [self._row(r) for r in rows]

    def dedupe_by_name(self, owner_user_id: str | None = None) -> None:
        """Keep one category per name (case-insensitive); remapping sessions to the keeper."""
        oid = self._oid(owner_user_id)
        rows = self.conn.execute(
            """
            SELECT id, name, sort_order, created_at FROM categories
            WHERE owner_user_id=?
            ORDER BY sort_order ASC, created_at ASC
            """,
            (oid,),
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
                "UPDATE sessions SET category_id=? WHERE category_id=? AND owner_user_id=?",
                (keeper, r["id"], oid),
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
        owner_user_id: str | None = None,
    ) -> dict[str, Any]:
        oid = self._oid(owner_user_id)
        self.ensure_defaults(oid)
        clean = name.strip()
        existing = self.conn.execute(
            "SELECT id FROM categories WHERE lower(name)=lower(?) AND owner_user_id=? LIMIT 1",
            (clean, oid),
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
        sort = self.conn.execute(
            "SELECT COALESCE(MAX(sort_order),0)+1 AS s FROM categories WHERE owner_user_id=?",
            (oid,),
        ).fetchone()["s"]
        self.conn.execute(
            """
            INSERT INTO categories (
                id, name, color, icon, keywords, sort_order, created_at, updated_at, owner_user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (cid, clean, color, icon, keywords.strip().lower(), sort, now, now, oid),
        )
        self.conn.commit()
        return self.get(cid, owner_user_id=oid)  # type: ignore[return-value]

    def get(self, category_id: str, owner_user_id: str | None = None) -> dict[str, Any] | None:
        row = self.conn.execute(
            "SELECT * FROM categories WHERE id = ? AND owner_user_id=?",
            (category_id, self._oid(owner_user_id)),
        ).fetchone()
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
            WHERE id=? AND owner_user_id=?
            """,
            (
                str(name).strip(),
                color,
                icon,
                str(keywords).strip().lower(),
                _now(),
                category_id,
                self._oid(),
            ),
        )
        self.conn.commit()
        return self.get(category_id)

    def delete(self, category_id: str, owner_user_id: str | None = None) -> bool:
        oid = self._oid(owner_user_id)
        row = self.get(category_id, owner_user_id=oid)
        if not row:
            return False
        self.conn.execute(
            "UPDATE sessions SET category_id=NULL WHERE category_id=? AND owner_user_id=?",
            (category_id, oid),
        )
        self.conn.execute(
            "DELETE FROM categories WHERE id=? AND owner_user_id=?",
            (category_id, oid),
        )
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
