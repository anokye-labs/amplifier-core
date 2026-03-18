"""Test that emit_and_collect returns list[dict] instead of list[str].

This tests the fix for the RustHookRegistry.emit_and_collect method
which previously returned JSON strings instead of Python dicts.
"""

import pytest
from amplifier_core._engine import RustHookRegistry
from amplifier_core.models import HookResult


@pytest.mark.asyncio
async def test_emit_and_collect_returns_dicts():
    """emit_and_collect returns list[dict] not list[str].

    Registers a handler returning HookResult with data, calls emit_and_collect,
    and asserts results are dicts with correct values — not JSON strings.
    """
    registry = RustHookRegistry()

    async def handler(event, data):
        return HookResult(action="continue", data={"key": "value", "count": 42})

    registry.register("test:event", handler, 0, name="test-handler")

    result = await registry.emit_and_collect("test:event", {"trigger": True})

    # Result should be a non-empty list
    assert isinstance(result, list)
    assert len(result) == 1, f"Expected 1 result, got {len(result)}"

    item = result[0]

    # Each item must be a dict, not a str
    assert isinstance(item, dict), (
        f"emit_and_collect must return list[dict], "
        f"but got list[{type(item).__name__}]: {item!r}"
    )
    assert not isinstance(item, str), "emit_and_collect must not return list[str]"

    # Verify dict values are correct
    assert item["key"] == "value", f"Expected key='value', got {item['key']!r}"
    assert item["count"] == 42, f"Expected count=42, got {item['count']!r}"
