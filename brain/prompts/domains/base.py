"""Domain plugin spec for extensible prompt rules."""

from __future__ import annotations

from dataclasses import dataclass, field


@dataclass(frozen=True)
class DomainSpec:
    """One capability area. Register new domains in domains/__init__.py."""

    id: str
    prefixes: tuple[str, ...]
    exact_tools: tuple[str, ...] = ()
    classify_rules: tuple[str, ...] = ()
    fill_by_tool: dict[str, tuple[str, ...]] = field(default_factory=dict)
    fill_default: tuple[str, ...] = ()
    continue_rules: tuple[str, ...] = ()
    respond_hints: tuple[str, ...] = ()

    def matches_tool(self, tool: str) -> bool:
        if not tool:
            return False
        if tool in self.exact_tools:
            return True
        return any(tool.startswith(p) for p in self.prefixes)

    def matches_catalog(self, tool_names: set[str]) -> bool:
        if not tool_names:
            return True  # empty catalog → include all domain guidance
        if any(t in tool_names for t in self.exact_tools):
            return True
        return any(any(t.startswith(p) for t in tool_names) for p in self.prefixes)

    def fill_hints_for(self, tool: str) -> tuple[str, ...]:
        if tool in self.fill_by_tool:
            return self.fill_by_tool[tool]
        for prefix, hints in self.fill_by_tool.items():
            if prefix.endswith(".") and tool.startswith(prefix):
                return hints
        return self.fill_default
