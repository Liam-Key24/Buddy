"""Life domains: todos, fitness, money, study, socials, research."""

from prompts.domains.base import DomainSpec

LIFE = DomainSpec(
    id="life",
    prefixes=(
        "todo.",
        "fitness.",
        "money.",
        "study.",
        "socials.",
        "research.",
    ),
    classify_rules=(
        "Tasks/todos → todo.list / todo.add / todo.update. Log food/workout/weight/climb → fitness.*.",
        "Personal income/expense ledger → money.log (NOT work.log_sales). Savings pots → money.pot / money.pots.",
        "Study progress/sessions/syllabus → study.*. Look before upsert when reading existing rows.",
        "Social content planning → socials.*. Research sessions → research.*.",
        "Mixed life dumps: pick the first concrete loggable fact (food, expense, task) if no single dominant intent.",
        "fitness.log_food: estimate macros if omitted and note they are estimates.",
        "money.analyze gives suggestions only — never make financial decisions for the user.",
    ),
    fill_by_tool={
        "todo.add": (
            '{"title":"...","deadline?":"YYYY-MM-DD","priority?":"low|medium|high|critical"}.',
        ),
        "todo.list": ('{"status?":"not_started|in_progress|completed","category?":"..."}.',),
        "todo.update": (
            '{"id":"...","action":"update|complete|delete", ...fields}.',
            "Omit id if unknown — Clarification asks.",
        ),
        "fitness.log_food": (
            '{"name":"...","calories":N,"protein":N,"carbs":N,"fat":N,"meal_type":"breakfast|lunch|dinner|snack"}.',
            "calories must be > 0; date optional.",
        ),
        "fitness.look": (
            '{"what":"food|workouts|weight|climbs|prs|fridge|recipes|summary","date?":"YYYY-MM-DD"}.',
        ),
        "fitness.log_workout": ('{"name":"...","sets":[{"exercise":"...","reps":N,"weight":N}]}.',),
        "fitness.log_weight": ('{"kg":82.4,"date?":"YYYY-MM-DD"}.',),
        "money.log": (
            '{"kind":"expense|income","description":"...","amount":12.50,"category?":"...","date?":"YYYY-MM-DD"}.',
        ),
        "money.list": ('{"year?":2026,"month?":8,"kind?":"expense|income"}.',),
        "money.summary": ('{"year?":2026,"month?":8}.',),
        "study.log_session": (
            '{"duration_minutes":60,"topic_id?":"...","subject_id?":"...","notes?":"..."}.',
        ),
        "study.look": (
            '{"what":"sessions|subjects|topics|assignments|status","subject_id?":"..."}.',
        ),
        "socials.weekly_review": (
            '{"week_start?":"YYYY-MM-DD","last_week_notes?":"..."}.',
        ),
        "research.get": (
            '{"query?":"title or question","conversation_id?":"..."}.',
        ),
    },
    continue_rules=(
        "After study.look or fitness.look, upsert/log only if user asked to record something.",
        "After socials.weekly_review, finish — user reviews proposals in the Socials UI.",
        "After research.list, research.get if user needs one session's detail.",
    ),
    respond_hints=(
        "For fitness logs, confirm meal/workout/weight briefly with key numbers.",
        "For money logs, confirm amount and category.",
        "For todos, confirm title and status change.",
        "For study, confirm session duration or assignment updated.",
    ),
)
