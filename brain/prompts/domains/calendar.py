"""Calendar, lifestyle, dream, and work tools."""

from prompts.domains.base import DomainSpec

CALENDAR = DomainSpec(
    id="calendar",
    prefixes=("calendar.", "lifestyle.", "dream.", "work."),
    classify_rules=(
        'Soft schedule desire ("book time", "I want to…", "can we fit…", "make time for…", "plan my week") → calendar.organize. calendar.pin ONLY with an explicit clock time.',
        "Multi-activity week packing (\"3 study sessions, 2 climbing…\") → calendar.organize with items[].",
        'Open availability ("when am I free…", "any free slots…") → calendar.look focus=free.',
        '"When am I working?" / work hours → calendar.look focus=work.',
        '"What\'s on today/tomorrow/this week?" → calendar.look focus=events with matching when=.',
        "Fixed meeting/appointment with clock → calendar.pin. Soft activity without clock → calendar.organize.",
        "Delete/cancel event → calendar.pin action=delete. Search by title → calendar.look with query.",
        "Dream log/search → dream.log / dream.search. Work sales/hours/stats → work.* (not money.*).",
        "Update Work/Sleep hours → lifestyle.set_schedule. List blocks → lifestyle.list_blocks.",
        'Tradeoff _ask from organize: user chose move:<event_id> → re-call calendar.organize with lift_event_ids:[that id] and same items (mode=propose).',
        '"Add this to my calendar" after a prior scheduling message → use prior user content from history for items.',
    ),
    fill_by_tool={
        "calendar.pin": (
            'start as ISO or "tomorrow 14:00"; explicit clock only.',
            "action create|update|delete; title required for create.",
        ),
        "calendar.look": (
            "when=today|tomorrow|this_week|next_week|weekend|YYYY-MM-DD; focus=events|free|work.",
            "duration_minutes only if they said how long. Returns Work/Sleep schedule + events.",
            '"when am I working?" → focus=work, when=this_week unless they said today.',
            '"next week plans?" → focus=events, when=next_week.',
        ),
        "calendar.organize": (
            "window + items[] (title, optional duration/count/when) + constraints like after_work.",
            "mode=propose unless they confirmed commit. items may include spark_id when memory lists Active Sparks.",
            "Empty items[] rebalances flexible events in the window.",
        ),
        "dream.log": ('body from user text; sleep_date optional YYYY-MM-DD; tags optional.',),
        "dream.search": ('query from user text (e.g. nightmare).',),
        "work.log_sales": ("amount + currency; work_date optional.",),
        "work.set_hours": ("end_hm and/or start_hm; work_date optional.",),
        "work.get_stats": ("empty {} is fine.",),
        "lifestyle.set_schedule": ('kind work|sleep; start_hm/end_hm or segments[].',),
    },
    continue_rules=(
        "After calendar.look free slots, finish unless user asked to book — then calendar.organize or calendar.pin.",
        "After calendar.organize proposal, finish and invite Accept unless user said yes/commit.",
        "After calendar.pin create/update/delete, finish unless multi-step goal remains.",
    ),
    respond_hints=(
        "For calendar.look free slots, list the best times in local language — never claim Work/Sleep hours are free.",
        "For calendar.organize proposals, list proposed times and invite Accept / yes to save.",
        "For calendar.pin, confirm what changed (created/updated/deleted) with title and time when available.",
        "For get_capacity / day_summary style results, report free/booked/meeting/focus hours clearly.",
        "When a dream was logged or searched, confirm briefly.",
        "When work sales/hours/stats ran, include the numbers clearly.",
    ),
)
