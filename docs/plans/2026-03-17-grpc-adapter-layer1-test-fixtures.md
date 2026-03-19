# gRPC Adapter Layer 1 Test Fixtures — Implementation Plan

> **Execution:** Use the subagent-driven-development workflow to implement this plan.

> **WARNING — Spec Review Loop Exhausted:** The automated spec review loop for this task
> exhausted after 3 iterations before converging on approval. The final (3rd) iteration
> verdict was **APPROVED** — all 5 mock classes match the spec, all imports are present,
> the critical constraint (no `amplifier_core.testing` imports) is satisfied, and the
> acceptance criterion passes. However, the loop exhaustion means earlier iterations flagged
> issues that required multiple correction rounds. **Human reviewer: inspect the file
> manually to confirm no subtle regressions from the correction cycles remain.**
> Two extras not in the original spec were noted as benign:
> (1) Five `@pytest.fixture` wrapper functions for the mock classes.
> (2) A `_is_coroutine()` helper function (unused in the file itself).

**Goal:** Create `tests/test_grpc_adapter_services.py` with 5 self-contained mock fixture classes that satisfy the `amplifier_core.interfaces` protocols for use in downstream gRPC adapter service tests.

**Architecture:** All mock classes use structural subtyping (duck typing) to satisfy the `Tool` and `Provider` protocols from `amplifier_core.interfaces` — no inheritance from protocol classes. The file is entirely self-contained with zero imports from `amplifier_core.testing`. The gRPC protobuf imports (`pb2`/`pb2_grpc`) are wrapped in `try/except ImportError` to handle environments where protobuf dependencies aren't installed.

**Tech Stack:** Python 3.11+, pytest, pytest-asyncio (strict mode), unittest.mock (AsyncMock, MagicMock)

**Design document:** `docs/plans/2026-03-04-grpc-v2-debt-fix-design.md`

**Task dependencies:** task-1 (gRPC dependency declaration), task-2 (protobuf stub generation)

---

## Glossary (read this first)

| Term | What it means |
|------|---------------|
| **Protocol** | A Python `typing.Protocol` class — any object with matching method/property signatures satisfies it (structural subtyping, no inheritance) |
| **`Tool` protocol** | Defined in `python/amplifier_core/interfaces.py:134–158` — requires `name`, `description` properties and `async execute(input)` method |
| **`Provider` protocol** | Defined in `python/amplifier_core/interfaces.py:67–131` — requires `name` property, `get_info()`, `async list_models()`, `async complete()`, `parse_tool_calls()` |
| **`pb2` / `pb2_grpc`** | Generated protobuf Python stubs at `python/amplifier_core/_grpc_gen/amplifier_module_pb2.py` and `amplifier_module_pb2_grpc.py` |
| **`asyncio_mode = "strict"`** | pytest-asyncio configuration in `pyproject.toml` — async tests require explicit `@pytest.mark.asyncio` decorator |

## File Map

These are ALL the files this task touches:

| File | Action | Purpose |
|------|--------|---------|
| `tests/test_grpc_adapter_services.py` | **Create** | Self-contained mock fixture classes for gRPC adapter tests |

Reference files (read-only, do NOT modify):

| File | Why you need it |
|------|-----------------|
| `python/amplifier_core/interfaces.py` | Protocol definitions that mocks must satisfy |
| `python/amplifier_core/_grpc_gen/amplifier_module_pb2.py` | Import target for `pb2` alias |
| `python/amplifier_core/_grpc_gen/amplifier_module_pb2_grpc.py` | Import target for `pb2_grpc` alias |
| `pyproject.toml` | Confirms `asyncio_mode = "strict"` and test paths |

---

## Constraints

1. **NO imports from `amplifier_core.testing`** — this is the critical constraint. All fixtures must be self-contained.
2. **Structural subtyping only** — mock classes must NOT inherit from protocol classes. They satisfy protocols by having matching method/property signatures.
3. **No `conftest.py`** — this project has no shared conftest. All fixtures are local to the test file.
4. **No `from __future__ import annotations`** — existing tests in this project do not use it.

---

### Task 1: Create the test fixture file with module docstring and imports

**Files:**
- Create: `tests/test_grpc_adapter_services.py`

**Step 1: Write the file with docstring and all imports**

Create `tests/test_grpc_adapter_services.py` with this exact content:

```python
"""
Self-contained test fixtures for gRPC adapter service tests.

Provides mock implementations of Tool, Provider, and gRPC context
without any imports from amplifier_core.testing.

All fixtures satisfy the amplifier_core.interfaces protocols via
structural subtyping (no inheritance required).
"""

import asyncio
import json
from typing import Any
from unittest.mock import AsyncMock, MagicMock

import pytest

try:
    from amplifier_core._grpc_gen import amplifier_module_pb2 as pb2
    from amplifier_core._grpc_gen import amplifier_module_pb2_grpc as pb2_grpc
except ImportError:  # protobuf / google not installed in this env
    pb2 = None  # type: ignore[assignment]
    pb2_grpc = None  # type: ignore[assignment]
```

**Step 2: Verify the imports work**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "import tests.test_grpc_adapter_services; print('OK')"
```
Expected: `OK` printed, exit code 0.

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add grpc adapter fixture file with imports"
```

---

### Task 2: Add MockTool class

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append MockTool class after the imports**

Add the following after the `pb2_grpc` import block:

```python


# ---------------------------------------------------------------------------
# MockTool
# ---------------------------------------------------------------------------


class MockTool:
    """Minimal Tool satisfying amplifier_core.interfaces.Tool protocol.

    Provides name, description, parameters_json properties and an async
    execute() that always returns success.
    """

    @property
    def name(self) -> str:
        return "mock_tool"

    @property
    def description(self) -> str:
        return "A mock tool for testing"

    @property
    def parameters_json(self) -> str:
        return json.dumps({"type": "object", "properties": {}})

    async def execute(self, input: dict[str, Any]) -> Any:
        return MagicMock(success=True, output="mock output", error=None)
```

**Step 2: Verify the file still imports cleanly**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "from tests.test_grpc_adapter_services import MockTool; print(MockTool().name)"
```
Expected: `mock_tool`

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add MockTool fixture class"
```

---

### Task 3: Add MockFailingTool class

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append MockFailingTool class after MockTool**

```python


# ---------------------------------------------------------------------------
# MockFailingTool
# ---------------------------------------------------------------------------


class MockFailingTool:
    """Tool that raises RuntimeError on execute.

    Used to test error-handling paths in the gRPC adapter.
    """

    @property
    def name(self) -> str:
        return "failing_tool"

    @property
    def description(self) -> str:
        return "A mock tool that always fails"

    @property
    def parameters_json(self) -> str:
        return json.dumps({"type": "object", "properties": {}})

    async def execute(self, input: dict[str, Any]) -> Any:
        raise RuntimeError("Tool execution failed")
```

**Step 2: Verify**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "
import asyncio
from tests.test_grpc_adapter_services import MockFailingTool
try:
    asyncio.run(MockFailingTool().execute({}))
    print('ERROR: should have raised')
except RuntimeError as e:
    print(f'OK: {e}')
"
```
Expected: `OK: Tool execution failed`

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add MockFailingTool fixture class"
```

---

### Task 4: Add MockSyncTool class

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append MockSyncTool class after MockFailingTool**

```python


# ---------------------------------------------------------------------------
# MockSyncTool
# ---------------------------------------------------------------------------


class MockSyncTool:
    """Tool with a synchronous (non-async) execute method.

    Used to verify that the gRPC adapter handles sync tools correctly.
    """

    @property
    def name(self) -> str:
        return "sync_tool"

    @property
    def description(self) -> str:
        return "A mock tool with a synchronous execute method"

    @property
    def parameters_json(self) -> str:
        return json.dumps({"type": "object", "properties": {}})

    def execute(self, input: dict[str, Any]) -> Any:  # intentionally synchronous
        return MagicMock(success=True, output="sync output", error=None)
```

**Step 2: Verify sync execute works and is NOT a coroutine**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "
import asyncio
from tests.test_grpc_adapter_services import MockSyncTool
tool = MockSyncTool()
result = tool.execute({})
print(f'sync={not asyncio.iscoroutinefunction(tool.execute)}, output={result.output}')
"
```
Expected: `sync=True, output=sync output`

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add MockSyncTool fixture class"
```

---

### Task 5: Add MockProvider class

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append MockProvider class after MockSyncTool**

```python


# ---------------------------------------------------------------------------
# MockProvider
# ---------------------------------------------------------------------------


class MockProvider:
    """Minimal Provider satisfying amplifier_core.interfaces.Provider protocol."""

    def __init__(self) -> None:
        self.list_models = AsyncMock(return_value=[])
        self.complete = AsyncMock(
            return_value=MagicMock(
                content="mock response",
                tool_calls=[],
                usage=MagicMock(
                    prompt_tokens=10,
                    completion_tokens=5,
                    total_tokens=15,
                ),
                finish_reason="stop",
            )
        )

    @property
    def name(self) -> str:
        return "mock_provider"

    def get_info(self) -> Any:
        return MagicMock(
            id="mock_provider",
            display_name="Mock Provider",
            credential_env_vars=[],
            capabilities=["chat"],
            defaults={},
            config_fields=[],
        )

    def parse_tool_calls(self, response: Any) -> list[Any]:
        return []
```

**Step 2: Verify MockProvider instantiates and get_info returns expected fields**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "
from tests.test_grpc_adapter_services import MockProvider
p = MockProvider()
info = p.get_info()
print(f'name={p.name}, id={info.id}, display={info.display_name}, caps={info.capabilities}')
"
```
Expected: `name=mock_provider, id=mock_provider, display=Mock Provider, caps=['chat']`

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add MockProvider fixture class"
```

---

### Task 6: Add MockContext class

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append MockContext class after MockProvider**

```python


# ---------------------------------------------------------------------------
# MockContext  (gRPC servicer context)
# ---------------------------------------------------------------------------


class MockContext:
    """Mock gRPC servicer context.

    Provides the minimal interface used by gRPC servicer methods:
    set_code(), set_details(), and async abort().
    """

    def __init__(self) -> None:
        self.code: Any = None
        self.details: str = ""
        self._aborted: bool = False

    def set_code(self, code: Any) -> None:
        self.code = code

    def set_details(self, details: str) -> None:
        self.details = details

    async def abort(self, code: Any, details: str) -> None:
        self.code = code
        self.details = details
        self._aborted = True
```

**Step 2: Verify MockContext abort is async and state tracks correctly**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "
import asyncio
from tests.test_grpc_adapter_services import MockContext
ctx = MockContext()
ctx.set_code('NOT_FOUND')
ctx.set_details('missing')
asyncio.run(ctx.abort('INTERNAL', 'error'))
print(f'code={ctx.code}, details={ctx.details}, aborted={ctx._aborted}')
"
```
Expected: `code=INTERNAL, details=error, aborted=True`

**Step 3: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add MockContext fixture class"
```

---

### Task 7: Add pytest fixture wrappers and utility helper

**Files:**
- Modify: `tests/test_grpc_adapter_services.py`

**Step 1: Append pytest fixture wrappers and _is_coroutine helper after MockContext**

```python


# ---------------------------------------------------------------------------
# Pytest fixtures — convenient @pytest.fixture wrappers for the mock classes
# ---------------------------------------------------------------------------


@pytest.fixture
def mock_tool() -> MockTool:
    """Return a fresh MockTool instance."""
    return MockTool()


@pytest.fixture
def mock_failing_tool() -> MockFailingTool:
    """Return a fresh MockFailingTool instance."""
    return MockFailingTool()


@pytest.fixture
def mock_sync_tool() -> MockSyncTool:
    """Return a fresh MockSyncTool instance."""
    return MockSyncTool()


@pytest.fixture
def mock_provider() -> MockProvider:
    """Return a fresh MockProvider instance."""
    return MockProvider()


@pytest.fixture
def mock_context() -> MockContext:
    """Return a fresh MockContext instance."""
    return MockContext()


# ---------------------------------------------------------------------------
# Sanity-check: verify async mock helpers work in the current event loop
# ---------------------------------------------------------------------------


def _is_coroutine(obj: Any) -> bool:
    """Return True if *obj* is a coroutine function (uses asyncio)."""
    return asyncio.iscoroutinefunction(obj)
```

**Step 2: Run full acceptance criterion**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "import tests.test_grpc_adapter_services; print('OK')"
```
Expected: `OK`

**Step 3: Run python_check for code quality**

Run:
```bash
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run ruff check tests/test_grpc_adapter_services.py && uv run ruff format --check tests/test_grpc_adapter_services.py
```
Expected: No errors, no formatting issues.

**Step 4: Verify no imports from amplifier_core.testing**

Run:
```bash
grep -n "amplifier_core.testing" /home/bkrabach/dev/rust-devrust-core/amplifier-core/tests/test_grpc_adapter_services.py
```
Expected: No output (no matches).

**Step 5: Commit**
```bash
git add tests/test_grpc_adapter_services.py && git commit -m "test: add pytest fixture wrappers and coroutine helper"
```

---

## Final Verification Checklist

After all tasks are complete, run these commands to confirm acceptance criteria:

```bash
# 1. File exists and imports cleanly
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "import tests.test_grpc_adapter_services; print('OK')"

# 2. All 5 mock classes are importable
cd /home/bkrabach/dev/rust-devrust-core/amplifier-core && uv run python -c "
from tests.test_grpc_adapter_services import MockTool, MockFailingTool, MockSyncTool, MockProvider, MockContext
print(f'MockTool={MockTool().name}')
print(f'MockFailingTool={MockFailingTool().name}')
print(f'MockSyncTool={MockSyncTool().name}')
print(f'MockProvider={MockProvider().name}')
print(f'MockContext aborted={MockContext()._aborted}')
print('ALL OK')
"

# 3. No amplifier_core.testing imports
grep -c "amplifier_core.testing" tests/test_grpc_adapter_services.py && echo "FAIL: found forbidden import" || echo "PASS: no forbidden imports"

# 4. Code quality
uv run ruff check tests/test_grpc_adapter_services.py
uv run ruff format --check tests/test_grpc_adapter_services.py
```

All commands should succeed with exit code 0.
