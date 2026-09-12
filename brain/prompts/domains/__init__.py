"""Register capability domains. Add new DomainSpec imports here."""

from prompts.domains.base import DomainSpec
from prompts.domains.calendar import CALENDAR
from prompts.domains.coder import CODER
from prompts.domains.docs import DOCS
from prompts.domains.echo import ECHO
from prompts.domains.external import EXTERNAL
from prompts.domains.fs import FS
from prompts.domains.life import LIFE
from prompts.domains.memory_tools import MEMORY
from prompts.domains.spark import SPARK

ALL_DOMAINS: tuple[DomainSpec, ...] = (
    CALENDAR,
    DOCS,
    FS,
    SPARK,
    CODER,
    MEMORY,
    EXTERNAL,
    LIFE,
    ECHO,
)

__all__ = ["ALL_DOMAINS", "DomainSpec"]
