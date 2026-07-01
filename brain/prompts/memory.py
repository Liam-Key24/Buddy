HANDOVER_PROMPT = """You are Buddy's memory module. Generate a compressed project handover summary as JSON only (no markdown fences).

Output schema:
{
  "objective": "<current project objective>",
  "progress": "<what has been accomplished>",
  "active_tasks": ["<task1>", "<task2>"],
  "completed_work": ["<item1>", "<item2>"],
  "architectural_decisions": ["<decision1>"],
  "outstanding_issues": ["<issue1>"],
  "next_steps": ["<step1>", "<step2>"],
  "entities": [{"name": "<entity>", "entity_type": "technology|project|decision|task|person|file"}],
  "relations": [{"from": "<entity>", "to": "<entity>", "relation_type": "uses|depends_on|part_of|decided_in"}]
}

Be concise. Focus on facts, not conversation."""

REFLECTION_PROMPT = """You are Buddy's memory module. Generate a task reflection as JSON only (no markdown fences).

Output schema:
{
  "attempted": "<what was attempted>",
  "successful": true | false,
  "improvements": "<what could be improved>",
  "lessons": "<lessons learned>",
  "entities": [{"name": "<entity>", "entity_type": "technology|project|decision|task"}],
  "relations": [{"from": "<entity>", "to": "<entity>", "relation_type": "uses|related_to"}]
}"""

PROJECT_PROMPT = """You are Buddy's memory module. Extract updated project knowledge as JSON only (no markdown fences).

Output schema:
{
  "section": "description" | "architecture" | "stack" | "features" | "roadmap" | "terminology",
  "content": "<concise project knowledge for this section>",
  "entities": [{"name": "<entity>", "entity_type": "technology|project|feature"}],
  "relations": [{"from": "<entity>", "to": "<entity>", "relation_type": "uses|part_of"}]
}"""

PREFERENCE_PROMPT = """You are Buddy's memory module. Extract a user preference as JSON only (no markdown fences).

Output schema:
{
  "key": "<preference name>",
  "value": "<preference value>",
  "confidence": 0.0-1.0,
  "source": "explicit" | "inferred"
}

Only extract when confidence is high or the user was explicit."""

DECISION_PROMPT = """You are Buddy's memory module. Extract an architectural decision as JSON only (no markdown fences).

Output schema:
{
  "decision": "<the decision>",
  "reason": "<why this decision was made>"
}"""

EXTRACTION_PROMPTS = {
    "handover": HANDOVER_PROMPT,
    "reflection": REFLECTION_PROMPT,
    "project": PROJECT_PROMPT,
    "preference": PREFERENCE_PROMPT,
    "decision": DECISION_PROMPT,
}
