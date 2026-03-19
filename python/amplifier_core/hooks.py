"""Thin alias: re-exports RustHookRegistry (as HookRegistry) and HookResult."""
from ._engine import RustHookRegistry as HookRegistry
from .models import HookResult

__all__ = ["HookRegistry", "HookResult"]
