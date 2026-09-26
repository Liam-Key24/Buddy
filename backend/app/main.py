"""Buddy backend — localhost FastAPI, SQLite, Cloud AI."""

from __future__ import annotations

from typing import Any

from fastapi import APIRouter, Depends, FastAPI, HTTPException, Request, Response
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import FileResponse, JSONResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, Field

from .auth import (
    SESSION_COOKIE,
    authenticate,
    bind_request_owner,
    cookie_kwargs,
    create_session as create_auth_session,
    lookup_session,
    revoke_session,
    unbind_request_owner,
)
from .config import load_settings
from .control_plane import ControlPlane, RevertBlocked
from . import user_settings as user_settings_mod
from .schemas import (
    ChatCancelRequest,
    ChatRequest,
    ChatResponse,
    OutcomeRequest,
    ProposalDecision,
    SessionUpdate,
    SparkCreate,
    SparkPromote,
    TodayResponse,
)

settings = load_settings()
app = FastAPI(title="Buddy", version="0.4.0")
public = APIRouter(prefix="/api")
api = APIRouter(prefix="/api")
app.add_middleware(
    CORSMiddleware,
    allow_origins=list(settings.cors_origins),
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

plane = ControlPlane(settings=settings)


_PUBLIC_API = {"/api/health", "/api/auth/login", "/api/auth/logout"}


@app.middleware("http")
async def auth_middleware(request: Request, call_next):
    if request.method == "OPTIONS":
        return await call_next(request)
    path = request.url.path
    if path.startswith("/api") and path not in _PUBLIC_API:
        user = lookup_session(plane.conn, request.cookies.get(SESSION_COOKIE))
        if not user:
            return JSONResponse({"detail": "Not authenticated"}, status_code=401)
        request.state.user = user
        token = bind_request_owner(user["id"])
        try:
            return await call_next(request)
        finally:
            unbind_request_owner(token)
    return await call_next(request)


def require_user(request: Request) -> dict:
    user = getattr(request.state, "user", None)
    if not user:
        user = lookup_session(plane.conn, request.cookies.get(SESSION_COOKIE))
    if not user:
        raise HTTPException(status_code=401, detail="Not authenticated")
    return user


def _owner_id(request: Request) -> str:
    return request.state.user["id"]


@app.on_event("shutdown")
def _shutdown():
    plane.close()


class LoginBody(BaseModel):
    username: str
    password: str


@public.get("/health")
def health():
    """Sanitised health — no paths, hosts, or model account detail."""
    db_ok = True
    try:
        plane.conn.execute("SELECT 1").fetchone()
    except Exception:
        db_ok = False
    return {
        "ok": db_ok,
        "product": "Buddy",
        "database": "available" if db_ok else "unavailable",
        "ai": "configured" if settings.groq_configured else "unconfigured",
        "privacy": "Local data · Cloud reasoning",
    }


@public.post("/auth/login")
def login(body: LoginBody, response: Response):
    user = authenticate(plane.conn, body.username, body.password)
    if not user:
        raise HTTPException(status_code=401, detail="Invalid username or password")
    raw = create_auth_session(plane.conn, user["id"])
    response.set_cookie(value=raw, **cookie_kwargs())
    return {"id": user["id"], "username": user["username"]}


@public.post("/auth/logout")
def logout(request: Request, response: Response):
    revoke_session(plane.conn, request.cookies.get(SESSION_COOKIE))
    response.delete_cookie(SESSION_COOKIE, path="/")
    return {"ok": True}


@api.get("/auth/me")
def me(user: dict = Depends(require_user)):
    return {"id": user["id"], "username": user["username"]}


class WorkSettingsPatch(BaseModel):
    enabled: bool | None = None
    mode: str | None = None
    start: str | None = None
    end: str | None = None
    days: list[bool] | None = None
    shifts: list[dict[str, Any]] | None = None


class SettingsPatch(BaseModel):
    show_avatar: bool | None = None
    compact_sidebar: bool | None = None
    confirm_deletes: bool | None = None
    sleep_enabled: bool | None = None
    skip_weekends: bool | None = None
    prefer_after: str | None = None
    work: WorkSettingsPatch | None = None


@api.get("/settings")
def get_user_settings(request: Request):
    return user_settings_mod.get_settings(plane.conn, _owner_id(request))


@api.put("/settings")
def put_user_settings(body: SettingsPatch, request: Request):
    patch = body.model_dump(exclude_unset=True)
    if "work" in patch and patch["work"] is not None:
        patch["work"] = {
            k: v for k, v in patch["work"].items() if v is not None
        }
    try:
        return user_settings_mod.put_settings(plane.conn, _owner_id(request), patch)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@api.post("/chat", response_model=ChatResponse)
def chat(req: ChatRequest, request: Request) -> ChatResponse:
    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Message required")
    try:
        return plane.handle_message(
            req.message,
            req.conversation_id,
            request_id=req.request_id,
            clarification_answers=req.clarification_answers,
            revision_of=req.revision_of,
            owner_user_id=_owner_id(request),
        )
    except RevertBlocked as exc:
        raise HTTPException(status_code=409, detail=exc.detail) from exc


@api.post("/chat/cancel")
def chat_cancel(body: ChatCancelRequest, request: Request):
    if not body.request_id.strip():
        raise HTTPException(status_code=400, detail="request_id required")
    return plane.cancel_request(body.request_id.strip(), owner_user_id=_owner_id(request))


@api.get("/today", response_model=TodayResponse)
def today(request: Request) -> TodayResponse:
    return plane.get_today(owner_user_id=_owner_id(request))


@api.get("/calendar/sessions")
def calendar_sessions(start: str | None = None, end: str | None = None):
    return plane.calendar.list_sessions(start=start, end=end)


@api.get("/calendar/fixed")
def calendar_fixed():
    return plane.calendar.list_fixed_blocks()


class FixedBlockCreate(BaseModel):
    title: str
    weekday: int
    start_minute: int
    end_minute: int


class FixedBlockUpdate(BaseModel):
    title: str | None = None
    weekday: int | None = None
    start_minute: int | None = None
    end_minute: int | None = None


@api.post("/calendar/fixed")
def create_fixed(body: FixedBlockCreate):
    try:
        return plane.calendar.create_fixed_block(
            title=body.title,
            weekday=body.weekday,
            start_minute=body.start_minute,
            end_minute=body.end_minute,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@api.patch("/calendar/fixed/{block_id}")
def update_fixed(block_id: str, body: FixedBlockUpdate):
    try:
        updated = plane.calendar.update_fixed_block(
            block_id,
            title=body.title,
            weekday=body.weekday,
            start_minute=body.start_minute,
            end_minute=body.end_minute,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    if not updated:
        raise HTTPException(status_code=404, detail="Fixed block not found")
    return updated


@api.delete("/calendar/fixed/{block_id}")
def delete_fixed(block_id: str):
    if not plane.calendar.delete_fixed_block(block_id):
        raise HTTPException(status_code=404, detail="Fixed block not found")
    return {"ok": True}


@api.post("/calendar/proposals/decide")
def decide_proposal(body: ProposalDecision, request: Request):
    return plane.decide_proposal(
        body.batch_id,
        body.decision,
        conversation_id=body.conversation_id,
        owner_user_id=_owner_id(request),
    )


@api.post("/calendar/sessions/outcome")
def session_outcome(body: OutcomeRequest, request: Request):
    updated = plane.mark_outcome(
        body.session_id, body.outcome, body.notes, owner_user_id=_owner_id(request)
    )
    if not updated:
        raise HTTPException(status_code=404, detail="Session not found")
    return updated


class SessionCreate(BaseModel):
    title: str
    start_at: str
    end_at: str
    category_id: str | None = None


@api.post("/calendar/sessions")
def create_session(body: SessionCreate):
    try:
        return plane.calendar.create_manual_session(
            title=body.title,
            start_at=body.start_at,
            end_at=body.end_at,
            category_id=body.category_id,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@api.delete("/calendar/sessions/{session_id}")
def delete_session(session_id: str):
    if not plane.calendar.delete_session(session_id):
        raise HTTPException(status_code=404, detail="Session not found")
    return {"ok": True}


@api.patch("/calendar/sessions/{session_id}")
def update_session(session_id: str, body: SessionUpdate):
    try:
        data = body.model_dump(exclude_unset=True)
        updated = plane.calendar.update_session(session_id, **data)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    if not updated:
        raise HTTPException(status_code=404, detail="Session not found")
    return updated


class CategoryCreate(BaseModel):
    name: str
    color: str = "#93c5fd"
    icon: str = "circle"
    keywords: str = ""


class CategoryUpdate(BaseModel):
    name: str | None = None
    color: str | None = None
    icon: str | None = None
    keywords: str | None = None


@api.get("/categories")
def list_categories():
    return plane.categories.list()


@api.post("/categories")
def create_category(body: CategoryCreate):
    if not body.name.strip():
        raise HTTPException(status_code=400, detail="Name required")
    cat = plane.categories.create(
        name=body.name,
        color=body.color,
        icon=body.icon,
        keywords=body.keywords,
    )
    plane.calendar.reclassify_all()
    return cat


@api.patch("/categories/{category_id}")
def update_category(category_id: str, body: CategoryUpdate):
    fields = {k: v for k, v in body.model_dump().items() if v is not None}
    cat = plane.categories.update(category_id, **fields)
    if not cat:
        raise HTTPException(status_code=404, detail="Category not found")
    plane.calendar.reclassify_all()
    return cat


@api.delete("/categories/{category_id}")
def delete_category(category_id: str):
    if not plane.categories.delete(category_id):
        raise HTTPException(status_code=404, detail="Category not found")
    plane.calendar.reclassify_all()
    return {"ok": True}


@api.post("/categories/reclassify")
def reclassify_sessions():
    return {"updated": plane.calendar.reclassify_all()}


@api.get("/sparks")
def list_sparks(open_only: bool = True):
    return plane.sparks.list_open() if open_only else plane.sparks.list_all()


@api.post("/sparks")
def create_spark(body: SparkCreate):
    if not body.content.strip():
        raise HTTPException(status_code=400, detail="Content required")
    return plane.sparks.capture(body.content)


@api.post("/sparks/{spark_id}/promote")
def promote_spark(spark_id: str, body: SparkPromote):
    spark_row = plane.sparks.get(spark_id)
    if not spark_row:
        raise HTTPException(status_code=404, detail="Spark not found")
    if body.goal_id:
        return plane.sparks.promote(spark_id, body.goal_id)
    seed = body.message or spark_row.content
    result = plane.handle_message(f"I want to work on this idea: {seed}")
    if result.goal:
        plane.sparks.promote(spark_id, result.goal.id)
    return {"chat": result, "spark_id": spark_id}


@api.post("/sparks/{spark_id}/dismiss")
def dismiss_spark(spark_id: str):
    spark = plane.sparks.dismiss(spark_id)
    if not spark:
        raise HTTPException(status_code=404, detail="Spark not found")
    return spark


class ConversationPatch(BaseModel):
    title: str | None = None
    folder_id: str | None = None


class ConversationPlace(BaseModel):
    folder_id: str | None = None
    before_id: str | None = None


class FolderCreate(BaseModel):
    title: str


class FolderRename(BaseModel):
    title: str


class DraftBody(BaseModel):
    draft: dict[str, Any] = Field(default_factory=dict)


@api.get("/conversations")
def list_conversations(request: Request):
    return plane.list_conversations(owner_user_id=_owner_id(request))


@api.post("/conversations")
def create_conversation(request: Request):
    return plane.create_conversation(owner_user_id=_owner_id(request))


@api.patch("/conversations/{conversation_id}")
def patch_conversation(conversation_id: str, body: ConversationPatch):
    row = None
    if body.title is not None:
        row = plane.rename_conversation(conversation_id, body.title)
        if not row:
            raise HTTPException(status_code=404, detail="Conversation not found")
    if "folder_id" in body.model_fields_set:
        row = plane.move_conversation(conversation_id, body.folder_id)
        if not row:
            raise HTTPException(status_code=404, detail="Conversation or folder not found")
    if row is None:
        raise HTTPException(status_code=400, detail="Nothing to update")
    return row


@api.post("/conversations/{conversation_id}/place")
def place_conversation(conversation_id: str, body: ConversationPlace):
    row = plane.place_conversation(conversation_id, body.folder_id, body.before_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation or folder not found")
    return row


@api.get("/folders")
def list_folders():
    return plane.list_folders()


@api.post("/folders")
def create_folder(body: FolderCreate):
    if not body.title.strip():
        raise HTTPException(status_code=400, detail="Title required")
    return plane.create_folder(body.title)


@api.patch("/folders/{folder_id}")
def rename_folder(folder_id: str, body: FolderRename):
    row = plane.rename_folder(folder_id, body.title)
    if not row:
        raise HTTPException(status_code=404, detail="Folder not found")
    return row


@api.delete("/folders/{folder_id}")
def delete_folder(folder_id: str):
    row = plane.delete_folder(folder_id)
    if not row:
        raise HTTPException(status_code=404, detail="Folder not found")
    return row


@api.delete("/conversations/{conversation_id}")
def delete_conversation(conversation_id: str):
    row = plane.delete_conversation(conversation_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@api.post("/conversations/{conversation_id}/restore")
def restore_conversation(conversation_id: str):
    row = plane.restore_conversation(conversation_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@api.put("/conversations/{conversation_id}/draft")
def save_draft(conversation_id: str, body: DraftBody):
    row = plane.save_draft(conversation_id, body.draft)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@api.post("/conversations/{conversation_id}/messages/{message_id}/revert")
def revert_message(conversation_id: str, message_id: str):
    result = plane.revert_to(conversation_id, message_id)
    if result.get("blocked"):
        raise HTTPException(status_code=409, detail=result.get("detail") or "Can't revert that message.")
    if not result.get("ok"):
        raise HTTPException(status_code=404, detail="Message not found")
    return result


@api.get("/conversations/{conversation_id}/messages")
def messages(conversation_id: str, request: Request):
    if not plane.conversations.get(conversation_id):
        raise HTTPException(status_code=404, detail="Conversation not found")
    return plane.list_messages(conversation_id, owner_user_id=_owner_id(request))


@api.get("/conversations/{conversation_id}/open-proposal")
def open_proposal(conversation_id: str, request: Request):
    if not plane.conversations.get(conversation_id):
        raise HTTPException(status_code=404, detail="Conversation not found")
    return plane.get_open_proposal(conversation_id, owner_user_id=_owner_id(request))


@api.get("/ai/usage")
def ai_usage(request: Request):
    return plane.usage_today(owner_user_id=_owner_id(request))


@api.get("/goals")
def list_goals():
    return plane.list_goals()


@api.get("/goals/{goal_id}")
def get_goal(goal_id: str):
    goal = plane.get_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal


@api.delete("/goals/{goal_id}")
def delete_goal(goal_id: str):
    goal = plane.remove_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal


app.include_router(public)
app.include_router(api)


def _mount_static() -> None:
    static = settings.static_dir
    if static is None or not static.is_dir():
        return
    assets = static / "assets"
    if assets.is_dir():
        app.mount("/assets", StaticFiles(directory=str(assets)), name="assets")
    index = static / "index.html"
    if not index.is_file():
        return

    @app.get("/")
    def spa_index():
        return FileResponse(index)

    @app.get("/{full_path:path}")
    def spa_fallback(full_path: str):
        # Never steal API routes (already registered) or missing asset paths.
        candidate = static / full_path
        if full_path and candidate.is_file():
            return FileResponse(candidate)
        return FileResponse(index)


_mount_static()
