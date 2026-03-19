# gRPC Adapter v2 + C ABI Layer — amplifier-core Implementation Plan

> **Execution:** Use the subagent-driven-development workflow to implement this plan.

**Goal:** Evolve amplifier-core's proto, PyO3 bindings, hooks, and FFI surface in a single coordinated release.
**Architecture:** Add typed `content_blocks` to ChatResponse proto with dual-write conversions, expose `conversions.rs` to Python via two new PyO3 functions, thin hooks.py to a 3-line alias (fixing emit_and_collect return type in Rust), and scaffold the C ABI crate with 25 extern "C" functions.
**Tech Stack:** Rust (prost/tonic, PyO3, cbindgen), Python (protobuf, pydantic), Protocol Buffers

**Design doc:** `docs/plans/2026-03-18-v2-adapter-and-c-abi-design.md`

---

## Block A: Proto Evolution (Tasks 1–3)

### Task 1: Add `content_blocks` Field to ChatResponse Proto

**Files:**
- Modify: `proto/amplifier_module.proto`

**Step 1: Add the new repeated field**

In `proto/amplifier_module.proto`, find the `ChatResponse` message (around line 337) and add field 7:

```protobuf
message ChatResponse {
  string                    content       = 1;  // DEPRECATED — keep for backward compat
  repeated ToolCallMessage  tool_calls    = 2;
  Usage                     usage         = 3;
  Degradation               degradation   = 4;
  // Completion stop reason: "stop", "tool_calls", "length", "content_filter".
  string                    finish_reason = 5;
  string                    metadata_json = 6;
  repeated ContentBlock     content_blocks = 7;  // NEW: typed, replaces JSON string in field 1
}
```

**Step 2: Verify proto syntax**

```bash
cd amplifier-core && protoc --proto_path=proto proto/amplifier_module.proto --descriptor_set_out=/dev/null
```

Expected: exits 0 with no output (clean parse).

**Step 3: Commit**

```bash
git add proto/amplifier_module.proto && git commit -m "proto: add repeated ContentBlock content_blocks = 7 to ChatResponse"
```

---

### Task 2: Regenerate Rust Proto Stubs

**Files:**
- Modified (by build): `crates/amplifier-core/src/generated/amplifier.module.rs`

**Step 1: Rebuild the crate to trigger tonic-build**

```bash
cd amplifier-core && cargo build -p amplifier-core 2>&1 | head -30
```

Expected: builds successfully. The `build.rs` at `crates/amplifier-core/build.rs` detects protoc and regenerates `src/generated/amplifier.module.rs` with the new `content_blocks` field on `ChatResponse`.

**Step 2: Verify the new field exists in generated code**

```bash
grep -n "content_blocks" crates/amplifier-core/src/generated/amplifier.module.rs | head -5
```

Expected: at least one match showing `pub content_blocks: ...` in the `ChatResponse` struct.

**Step 3: Run existing Rust tests to confirm nothing breaks**

```bash
cd amplifier-core && cargo test -p amplifier-core 2>&1 | tail -20
```

Expected: all existing tests pass. The new field is additive — existing code that doesn't reference `content_blocks` is unaffected (it defaults to an empty `Vec`).

**Step 4: Commit**

```bash
git add crates/amplifier-core/src/generated/amplifier.module.rs && git commit -m "chore: regenerate Rust proto stubs with content_blocks field"
```

---

### Task 3: Dual-Write `content_blocks` in `conversions.rs`

**Files:**
- Modify: `crates/amplifier-core/src/generated/conversions.rs`
- Test: inline `#[cfg(test)] mod tests` (existing test module in same file)

**Step 1: Write the failing test**

Add these tests to the existing `mod tests` block inside `conversions.rs` (after the existing `chat_response_empty_content_roundtrip` test, around line 1990):

```rust
    #[test]
    fn chat_response_content_blocks_populated_on_write() {
        use crate::messages::ChatResponse;

        let original = ChatResponse {
            content: vec![crate::messages::ContentBlock::Text {
                text: "Hello!".into(),
                visibility: None,
            }],
            tool_calls: None,
            usage: None,
            degradation: None,
            finish_reason: None,
            metadata: None,
            extensions: HashMap::new(),
        };

        let proto = super::native_chat_response_to_proto(&original);

        // Legacy field still populated (backward compat)
        assert!(!proto.content.is_empty(), "legacy content field must be populated");

        // New typed field also populated (dual-write)
        assert_eq!(proto.content_blocks.len(), 1, "content_blocks must have 1 block");

        // Verify it's a TextBlock
        let block = &proto.content_blocks[0];
        match &block.block {
            Some(super::super::amplifier_module::content_block::Block::TextBlock(tb)) => {
                assert_eq!(tb.text, "Hello!");
            }
            other => panic!("Expected TextBlock, got {:?}", other),
        }
    }

    #[test]
    fn chat_response_prefers_content_blocks_on_read() {
        // When content_blocks is non-empty, prefer it over the legacy JSON string
        let proto = super::super::amplifier_module::ChatResponse {
            content: "[]".to_string(), // empty legacy
            content_blocks: vec![super::super::amplifier_module::ContentBlock {
                block: Some(
                    super::super::amplifier_module::content_block::Block::TextBlock(
                        super::super::amplifier_module::TextBlock {
                            text: "From typed field".into(),
                        },
                    ),
                ),
                visibility: 0,
            }],
            tool_calls: vec![],
            usage: None,
            degradation: None,
            finish_reason: String::new(),
            metadata_json: String::new(),
        };

        let native = super::proto_chat_response_to_native(proto);
        assert_eq!(native.content.len(), 1);
        match &native.content[0] {
            crate::messages::ContentBlock::Text { text, .. } => {
                assert_eq!(text, "From typed field");
            }
            other => panic!("Expected Text block, got {:?}", other),
        }
    }
```

**Step 2: Run the new tests to verify they fail**

```bash
cd amplifier-core && cargo test -p amplifier-core chat_response_content_blocks_populated_on_write -- --nocapture 2>&1 | tail -10
```

Expected: FAIL — `content_blocks` is empty because `native_chat_response_to_proto` doesn't populate it yet.

**Step 3: Implement dual-write in `native_chat_response_to_proto`**

In `crates/amplifier-core/src/generated/conversions.rs`, find `native_chat_response_to_proto` (line ~874). The function currently returns a `ChatResponse` struct literal. Add `content_blocks` to the struct:

Replace:
```rust
pub fn native_chat_response_to_proto(
    response: &crate::messages::ChatResponse,
) -> super::amplifier_module::ChatResponse {
    super::amplifier_module::ChatResponse {
        content: to_json_or_warn(&response.content, "ChatResponse content"),
        tool_calls: response
            .tool_calls
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|tc| super::amplifier_module::ToolCallMessage {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments_json: to_json_or_warn(&tc.arguments, "ToolCall arguments"),
            })
            .collect(),
        usage: response.usage.clone().map(Into::into),
        degradation: response
            .degradation
            .as_ref()
            .map(|d| super::amplifier_module::Degradation {
                requested: d.requested.clone(),
                actual: d.actual.clone(),
                reason: d.reason.clone(),
            }),
        finish_reason: response.finish_reason.clone().unwrap_or_default(),
        metadata_json: response
            .metadata
            .as_ref()
            .map(|m| to_json_or_warn(m, "ChatResponse metadata"))
            .unwrap_or_default(),
    }
}
```

With:
```rust
pub fn native_chat_response_to_proto(
    response: &crate::messages::ChatResponse,
) -> super::amplifier_module::ChatResponse {
    super::amplifier_module::ChatResponse {
        // Legacy field — dual-write for backward compatibility
        content: to_json_or_warn(&response.content, "ChatResponse content"),
        // NEW: typed content blocks (dual-write)
        content_blocks: response
            .content
            .iter()
            .map(|b| native_content_block_to_proto(b.clone()))
            .collect(),
        tool_calls: response
            .tool_calls
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|tc| super::amplifier_module::ToolCallMessage {
                id: tc.id.clone(),
                name: tc.name.clone(),
                arguments_json: to_json_or_warn(&tc.arguments, "ToolCall arguments"),
            })
            .collect(),
        usage: response.usage.clone().map(Into::into),
        degradation: response
            .degradation
            .as_ref()
            .map(|d| super::amplifier_module::Degradation {
                requested: d.requested.clone(),
                actual: d.actual.clone(),
                reason: d.reason.clone(),
            }),
        finish_reason: response.finish_reason.clone().unwrap_or_default(),
        metadata_json: response
            .metadata
            .as_ref()
            .map(|m| to_json_or_warn(m, "ChatResponse metadata"))
            .unwrap_or_default(),
    }
}
```

**Step 4: Implement prefer-`content_blocks` in `proto_chat_response_to_native`**

In the same file, find `proto_chat_response_to_native` (line ~916). Replace the `content` field logic:

Replace:
```rust
        content: if response.content.is_empty() {
            Vec::new()
        } else {
            from_json_or_default(&response.content, "ChatResponse content")
        },
```

With:
```rust
        // Prefer typed content_blocks if present; fall back to JSON-deserializing legacy content
        content: if !response.content_blocks.is_empty() {
            response
                .content_blocks
                .into_iter()
                .filter_map(|b| {
                    proto_content_block_to_native(b)
                        .map_err(|e| {
                            log::warn!("Skipping invalid content block in ChatResponse: {e}");
                            e
                        })
                        .ok()
                })
                .collect()
        } else if response.content.is_empty() {
            Vec::new()
        } else {
            from_json_or_default(&response.content, "ChatResponse content")
        },
```

**Step 5: Run all tests to verify they pass**

```bash
cd amplifier-core && cargo test -p amplifier-core 2>&1 | tail -20
```

Expected: ALL tests pass, including the two new ones and all existing roundtrip tests.

**Step 6: Commit**

```bash
git add crates/amplifier-core/src/generated/conversions.rs && git commit -m "feat: dual-write content_blocks in ChatResponse conversions"
```

---

## Block B: PyO3 Proto Bridge (Tasks 4–5)

### Task 4: Add `proto_chat_request_to_json` PyO3 Function

**Files:**
- Modify: `bindings/python/src/lib.rs`
- Test: `tests/test_pyo3_proto_bridge.py` (new)

**Step 1: Write the failing Python test**

Create `tests/test_pyo3_proto_bridge.py`:

```python
"""Tests for the PyO3 proto-to-JSON bridge functions."""

import json

import pytest

try:
    from amplifier_core._engine import proto_chat_request_to_json
except ImportError:
    pytest.skip("proto_chat_request_to_json not yet exposed", allow_module_level=True)

try:
    from amplifier_core._grpc_gen import amplifier_module_pb2 as pb2
except ImportError:
    pytest.skip("grpcio/protobuf not installed", allow_module_level=True)


class TestProtoChatRequestToJson:
    """Verify proto bytes → Rust → JSON string pipeline."""

    def test_minimal_request(self):
        """A ChatRequest with one user message round-trips through Rust to valid JSON."""
        msg = pb2.Message(
            role=pb2.ROLE_USER,
            text_content="Hello, world!",
        )
        request = pb2.ChatRequest(messages=[msg])
        proto_bytes = request.SerializeToString()

        json_str = proto_chat_request_to_json(proto_bytes)

        parsed = json.loads(json_str)
        assert "messages" in parsed
        assert len(parsed["messages"]) == 1

    def test_request_with_tools(self):
        """ChatRequest with tool specs converts to JSON with tools array."""
        msg = pb2.Message(role=pb2.ROLE_USER, text_content="help")
        tool = pb2.ToolSpecProto(
            name="search",
            description="Search the web",
            parameters_json='{"type": "object"}',
        )
        request = pb2.ChatRequest(messages=[msg], tools=[tool])
        proto_bytes = request.SerializeToString()

        json_str = proto_chat_request_to_json(proto_bytes)

        parsed = json.loads(json_str)
        assert parsed["tools"] is not None
        assert len(parsed["tools"]) == 1
        assert parsed["tools"][0]["name"] == "search"

    def test_empty_bytes_raises(self):
        """Empty bytes should produce a valid (empty) ChatRequest, not crash."""
        json_str = proto_chat_request_to_json(b"")
        parsed = json.loads(json_str)
        assert "messages" in parsed
```

**Step 2: Run the test to verify it fails**

```bash
cd amplifier-core && uv run pytest tests/test_pyo3_proto_bridge.py -v 2>&1 | tail -10
```

Expected: SKIP or ImportError — `proto_chat_request_to_json` doesn't exist yet.

**Step 3: Implement the PyO3 function**

In `bindings/python/src/lib.rs`, add a new function. First, add the necessary imports near the top of the file (after line 16):

```rust
use pyo3::types::PyBytes;
```

Then add the function before the `#[pymodule]` block (before line 59):

```rust
/// Convert serialized proto ChatRequest bytes to a JSON string via conversions.rs.
///
/// This is the Rust side of the PyO3 bridge: proto bytes come in from Python,
/// get decoded and converted using the same `conversions.rs` logic the kernel
/// uses, and return as a JSON string that Python can feed to Pydantic.
#[pyfunction]
fn proto_chat_request_to_json(proto_bytes: &[u8]) -> PyResult<String> {
    use amplifier_core::generated::amplifier_module::ChatRequest;
    use amplifier_core::generated::conversions::proto_chat_request_to_native;
    use pyo3::exceptions::PyValueError;

    let proto = ChatRequest::decode(proto_bytes)
        .map_err(|e| PyValueError::new_err(format!("Failed to decode ChatRequest proto: {e}")))?;
    let native = proto_chat_request_to_native(proto);
    serde_json::to_string(&native)
        .map_err(|e| PyValueError::new_err(format!("Failed to serialize ChatRequest to JSON: {e}")))
}
```

Note: You will also need to add a `use prost::Message as _;` import at the top of the file (for the `.decode()` method). Add it near line 16:

```rust
use prost::Message as ProstMessage;
```

And add `prost` to the `bindings/python/Cargo.toml` dependencies:

```toml
prost = "0.13"
```

Then register the function in the `#[pymodule]` block. Find the line `m.add_function(wrap_pyfunction!(resolve_module, m)?)?;` (line ~82) and add after it:

```rust
    m.add_function(wrap_pyfunction!(proto_chat_request_to_json, m)?)?;
```

**Step 4: Rebuild and run the test**

```bash
cd amplifier-core && uv run --reinstall-package amplifier-core pytest tests/test_pyo3_proto_bridge.py::TestProtoChatRequestToJson -v 2>&1 | tail -15
```

Expected: all 3 tests PASS.

**Step 5: Commit**

```bash
git add bindings/python/src/lib.rs bindings/python/Cargo.toml tests/test_pyo3_proto_bridge.py && git commit -m "feat: add proto_chat_request_to_json PyO3 bridge function"
```

---

### Task 5: Add `json_to_proto_chat_response` PyO3 Function

**Files:**
- Modify: `bindings/python/src/lib.rs`
- Test: `tests/test_pyo3_proto_bridge.py` (append)

**Step 1: Write the failing test**

Append to `tests/test_pyo3_proto_bridge.py`:

```python
try:
    from amplifier_core._engine import json_to_proto_chat_response
except ImportError:
    pytest.skip("json_to_proto_chat_response not yet exposed", allow_module_level=True)


class TestJsonToProtoChatResponse:
    """Verify JSON string → Rust → proto bytes pipeline."""

    def test_minimal_response(self):
        """A minimal ChatResponse JSON round-trips through Rust to valid proto bytes."""
        response_json = json.dumps({
            "content": [{"type": "text", "text": "Hello!"}],
            "finish_reason": "stop",
        })

        proto_bytes = json_to_proto_chat_response(response_json)

        # Decode back with protobuf to verify
        response = pb2.ChatResponse()
        response.ParseFromString(bytes(proto_bytes))
        assert response.finish_reason == "stop"
        # Legacy content field should be populated (dual-write)
        assert response.content != ""
        # New content_blocks should also be populated
        assert len(response.content_blocks) == 1

    def test_response_with_tool_calls(self):
        """ChatResponse with tool_calls converts to proto with ToolCallMessage entries."""
        response_json = json.dumps({
            "content": [{"type": "text", "text": "Let me search."}],
            "tool_calls": [
                {"id": "tc1", "name": "search", "arguments": {"query": "rust"}}
            ],
            "finish_reason": "tool_calls",
        })

        proto_bytes = json_to_proto_chat_response(response_json)

        response = pb2.ChatResponse()
        response.ParseFromString(bytes(proto_bytes))
        assert len(response.tool_calls) == 1
        assert response.tool_calls[0].name == "search"

    def test_roundtrip_request_response(self):
        """Full round-trip: proto request → JSON → Pydantic → JSON → proto response."""
        # Build a proto request
        msg = pb2.Message(role=pb2.ROLE_USER, text_content="Hello!")
        request = pb2.ChatRequest(messages=[msg])

        # Proto bytes → JSON (request path)
        req_json = proto_chat_request_to_json(request.SerializeToString())
        parsed_req = json.loads(req_json)
        assert len(parsed_req["messages"]) == 1

        # Simulate provider response
        response_json = json.dumps({
            "content": [{"type": "text", "text": "Hi there!"}],
            "finish_reason": "stop",
        })

        # JSON → proto bytes (response path)
        resp_bytes = json_to_proto_chat_response(response_json)
        response = pb2.ChatResponse()
        response.ParseFromString(bytes(resp_bytes))
        assert response.finish_reason == "stop"
```

**Step 2: Run to verify it fails**

```bash
cd amplifier-core && uv run pytest tests/test_pyo3_proto_bridge.py::TestJsonToProtoChatResponse -v 2>&1 | tail -10
```

Expected: SKIP or ImportError.

**Step 3: Implement the function**

In `bindings/python/src/lib.rs`, add after the `proto_chat_request_to_json` function:

```rust
/// Convert a JSON ChatResponse string to serialized proto bytes via conversions.rs.
///
/// The reverse of `proto_chat_request_to_json`: takes a JSON string (from
/// Pydantic's `model_dump_json()`), deserializes to a native ChatResponse,
/// converts to proto using `conversions.rs`, and returns encoded proto bytes.
#[pyfunction]
fn json_to_proto_chat_response(json_str: &str) -> PyResult<Vec<u8>> {
    use amplifier_core::generated::conversions::native_chat_response_to_proto;
    use pyo3::exceptions::PyValueError;

    let native: amplifier_core::messages::ChatResponse = serde_json::from_str(json_str)
        .map_err(|e| PyValueError::new_err(format!("Failed to deserialize ChatResponse JSON: {e}")))?;
    let proto = native_chat_response_to_proto(&native);
    use prost::Message;
    Ok(proto.encode_to_vec())
}
```

Register it in the `#[pymodule]` block, right after the `proto_chat_request_to_json` registration:

```rust
    m.add_function(wrap_pyfunction!(json_to_proto_chat_response, m)?)?;
```

**Step 4: Update the `.pyi` stub**

In `python/amplifier_core/_engine.pyi`, add near the end of the file (before the last function definitions):

```python
def proto_chat_request_to_json(proto_bytes: bytes) -> str:
    """Convert serialized proto ChatRequest bytes to a JSON string via Rust conversions.rs.

    Raises:
        ValueError: If proto_bytes cannot be decoded or serialized.
    """
    ...

def json_to_proto_chat_response(json_str: str) -> bytes:
    """Convert a JSON ChatResponse string to serialized proto bytes via Rust conversions.rs.

    Raises:
        ValueError: If json_str cannot be deserialized or encoded.
    """
    ...
```

**Step 5: Rebuild and run all bridge tests**

```bash
cd amplifier-core && uv run --reinstall-package amplifier-core pytest tests/test_pyo3_proto_bridge.py -v 2>&1 | tail -20
```

Expected: all 6 tests PASS.

**Step 6: Commit**

```bash
git add bindings/python/src/lib.rs python/amplifier_core/_engine.pyi tests/test_pyo3_proto_bridge.py && git commit -m "feat: add json_to_proto_chat_response PyO3 bridge function"
```

---

## Block C: Hooks Thinning (Tasks 6–8)

### Task 6: Fix `emit_and_collect` to Return Dicts in Rust

**Files:**
- Modify: `bindings/python/src/hooks.rs`
- Test: `bindings/python/tests/test_switchover_hooks.py` (check existing)

**Step 1: Write the failing test**

Create `tests/test_emit_and_collect_returns_dicts.py`:

```python
"""Verify that RustHookRegistry.emit_and_collect returns list[dict], not list[str]."""

import pytest

from amplifier_core._engine import RustHookRegistry
from amplifier_core.models import HookResult


@pytest.fixture
def registry():
    return RustHookRegistry()


@pytest.mark.asyncio(loop_scope="function")
async def test_emit_and_collect_returns_dicts(registry):
    """emit_and_collect() must return list[dict], not list[str]."""

    async def handler(event, data):
        return HookResult(action="continue", data={"key": "value", "count": 42})

    registry.register("test:event", handler, name="dict-handler")

    results = await registry.emit_and_collect("test:event", {"input": "data"})

    assert len(results) == 1
    result = results[0]
    # This is the key assertion: result must be a dict, not a JSON string
    assert isinstance(result, dict), f"Expected dict, got {type(result).__name__}: {result!r}"
    assert result["key"] == "value"
    assert result["count"] == 42
```

**Step 2: Run to verify it fails**

```bash
cd amplifier-core && uv run pytest tests/test_emit_and_collect_returns_dicts.py -v 2>&1 | tail -10
```

Expected: FAIL — `isinstance(result, dict)` fails because `emit_and_collect` currently returns `list[str]` (JSON strings).

**Step 3: Fix `emit_and_collect` in Rust**

In `bindings/python/src/hooks.rs`, find the `emit_and_collect` method (line ~232). Replace the async block that converts results to JSON strings:

Replace:
```rust
        wrap_future_as_coroutine(
            py,
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                let results = inner.emit_and_collect(&event, value, timeout_dur).await;
                // Convert each HashMap<String, Value> to a JSON string.
                // Returns Vec<String> which becomes a Python list of strings.
                let json_strings: Vec<String> = results
                    .iter()
                    .map(|r| serde_json::to_string(r).unwrap_or_else(|e| {
                        log::warn!("Failed to serialize emit_and_collect result to JSON (using empty object): {e}");
                        "{}".to_string()
                    }))
                    .collect();
                Ok(json_strings)
            }),
        )
```

With:
```rust
        wrap_future_as_coroutine(
            py,
            pyo3_async_runtimes::tokio::future_into_py(py, async move {
                let results = inner.emit_and_collect(&event, value, timeout_dur).await;
                // Convert each HashMap<String, Value> to a Python dict.
                // This matches the Python HookRegistry.emit_and_collect() contract
                // which returns list[dict], not list[str].
                Python::try_attach(|py| -> PyResult<Py<PyAny>> {
                    let json_mod = py.import("json")?;
                    let py_list = pyo3::types::PyList::empty(py);
                    for r in &results {
                        let json_str = serde_json::to_string(r).unwrap_or_else(|e| {
                            log::warn!("Failed to serialize emit_and_collect result to JSON (using empty object): {e}");
                            "{}".to_string()
                        });
                        let dict = json_mod.call_method1("loads", (&json_str,))?;
                        py_list.append(dict)?;
                    }
                    Ok(py_list.into_any().unbind())
                })
                .ok_or_else(|| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                        "Failed to attach to Python runtime",
                    )
                })?
            }),
        )
```

You will also need to add `use pyo3::prelude::*;` if not already present (it is — check the top of hooks.rs).

**Step 4: Rebuild and run test**

```bash
cd amplifier-core && uv run --reinstall-package amplifier-core pytest tests/test_emit_and_collect_returns_dicts.py -v 2>&1 | tail -10
```

Expected: PASS.

**Step 5: Run existing hooks tests to verify no regressions**

```bash
cd amplifier-core && uv run pytest tests/test_hooks.py bindings/python/tests/test_switchover_hooks.py -v 2>&1 | tail -20
```

Expected: all PASS.

**Step 6: Commit**

```bash
git add bindings/python/src/hooks.rs tests/test_emit_and_collect_returns_dicts.py && git commit -m "fix: emit_and_collect returns list[dict] instead of list[str]"
```

---

### Task 7: Thin `hooks.py` to 3-Line Alias

**Files:**
- Modify: `python/amplifier_core/hooks.py`
- Test: `tests/test_hooks_alias.py` (new)

**Step 1: Write the test**

Create `tests/test_hooks_alias.py`:

```python
"""Verify the hooks.py 3-line alias works for all ecosystem import patterns."""

import pytest


def test_import_hook_registry():
    """from amplifier_core.hooks import HookRegistry works."""
    from amplifier_core.hooks import HookRegistry

    assert HookRegistry is not None
    # It should be the Rust class
    assert HookRegistry.__name__ == "RustHookRegistry"


def test_import_hook_result():
    """from amplifier_core.hooks import HookResult works."""
    from amplifier_core.hooks import HookResult

    assert HookResult is not None


def test_hook_registry_has_class_constants():
    """HookRegistry.SESSION_START etc. are accessible (from Rust #[classattr])."""
    from amplifier_core.hooks import HookRegistry

    assert HookRegistry.SESSION_START == "session:start"
    assert HookRegistry.SESSION_END == "session:end"
    assert HookRegistry.PROMPT_SUBMIT == "prompt:submit"
    assert HookRegistry.TOOL_PRE == "tool:pre"
    assert HookRegistry.TOOL_POST == "tool:post"
    assert HookRegistry.CONTEXT_PRE_COMPACT == "context:pre_compact"
    assert HookRegistry.ORCHESTRATOR_COMPLETE == "orchestrator:complete"
    assert HookRegistry.USER_NOTIFICATION == "user:notification"


def test_hook_registry_instantiation():
    """HookRegistry() creates a working instance."""
    from amplifier_core.hooks import HookRegistry

    registry = HookRegistry()
    handlers = registry.list_handlers()
    assert isinstance(handlers, dict)


def test_all_exports():
    """__all__ contains exactly HookRegistry and HookResult."""
    from amplifier_core import hooks

    assert set(hooks.__all__) == {"HookRegistry", "HookResult"}
```

**Step 2: Replace `hooks.py`**

Replace the entire contents of `python/amplifier_core/hooks.py` (all 348 lines) with:

```python
"""Hook system — thin re-export of the Rust HookRegistry."""

from amplifier_core._engine import RustHookRegistry as HookRegistry
from amplifier_core.models import HookResult

__all__ = ["HookRegistry", "HookResult"]
```

**Step 3: Run the alias tests**

```bash
cd amplifier-core && uv run pytest tests/test_hooks_alias.py -v 2>&1 | tail -15
```

Expected: all 5 tests PASS.

**Step 4: Run ALL hooks-related tests to verify no regressions**

```bash
cd amplifier-core && uv run pytest tests/test_hooks.py tests/test_hooks_timestamp.py tests/test_hook_validation_debug.py tests/test_emit_and_collect_returns_dicts.py bindings/python/tests/test_switchover_hooks.py -v 2>&1 | tail -20
```

Expected: all PASS. Any test that was testing the pure-Python `HookHandler` dataclass internals will have been removed with the file — that's expected.

**Step 5: Commit**

```bash
git add python/amplifier_core/hooks.py tests/test_hooks_alias.py && git commit -m "refactor: thin hooks.py to 3-line Rust alias (348 lines → 5 lines)"
```

---

### Task 8: Sync `_engine.pyi` Stub Constants

**Files:**
- Modify: `python/amplifier_core/_engine.pyi`

**Step 1: Fix the stub**

In `python/amplifier_core/_engine.pyi`, find the `RustHookRegistry` class (around line 64). Replace the event constants block (lines ~72-87) which currently lists 16 phantom constants with the actual 8 `#[classattr]` entries from `bindings/python/src/hooks.rs`:

Replace:
```python
    # Event constants
    SESSION_START: str
    SESSION_END: str
    SESSION_ERROR: str
    SESSION_RESUME: str
    SESSION_FORK: str
    TURN_START: str
    TURN_END: str
    TURN_ERROR: str
    PROVIDER_REQUEST: str
    PROVIDER_RESPONSE: str
    PROVIDER_ERROR: str
    TOOL_CALL: str
    TOOL_RESULT: str
    TOOL_ERROR: str
    CANCEL_REQUESTED: str
    CANCEL_COMPLETED: str
```

With:
```python
    # Event constants (8 #[classattr] entries in hooks.rs)
    SESSION_START: str
    SESSION_END: str
    PROMPT_SUBMIT: str
    TOOL_PRE: str
    TOOL_POST: str
    CONTEXT_PRE_COMPACT: str
    ORCHESTRATOR_COMPLETE: str
    USER_NOTIFICATION: str
```

**Step 2: Run the stub validation test**

```bash
cd amplifier-core && uv run pytest bindings/python/tests/test_python_stubs.py bindings/python/tests/test_stub_validation.py -v 2>&1 | tail -15
```

Expected: PASS (or skip if those tests validate different things).

**Step 3: Commit**

```bash
git add python/amplifier_core/_engine.pyi && git commit -m "fix: sync _engine.pyi stub constants with actual Rust #[classattr] entries"
```

---

## Block D: Regenerate Python Proto Stubs (Task 9)

### Task 9: Regenerate Python gRPC Stubs

**Files:**
- Modified (by codegen): `python/amplifier_core/_grpc_gen/amplifier_module_pb2.py`
- Modified (by codegen): `python/amplifier_core/_grpc_gen/amplifier_module_pb2_grpc.py`

**Step 1: Regenerate the stubs**

```bash
cd amplifier-core && python -m grpc_tools.protoc -I proto \
    --python_out=python/amplifier_core/_grpc_gen \
    --grpc_python_out=python/amplifier_core/_grpc_gen \
    proto/amplifier_module.proto
```

Expected: exits 0. The `amplifier_module_pb2.py` now includes the `content_blocks` field on `ChatResponse`.

**Step 2: Verify the new field is in the generated Python stub**

```bash
grep -n "content_blocks" python/amplifier_core/_grpc_gen/amplifier_module_pb2.py | head -5
```

Expected: at least one match.

**Step 3: Run the proto compilation test**

```bash
cd amplifier-core && uv run pytest tests/test_proto_compilation.py tests/test_grpc_stubs_regenerated.py -v 2>&1 | tail -10
```

Expected: PASS.

**Step 4: Commit**

```bash
git add python/amplifier_core/_grpc_gen/ && git commit -m "chore: regenerate Python gRPC stubs with content_blocks field"
```

---

## Block E: C ABI Scaffold (Tasks 10–15)

### Task 10: Create FFI Crate Skeleton

**Files:**
- Create: `crates/amplifier-ffi/Cargo.toml`
- Create: `crates/amplifier-ffi/src/lib.rs`
- Create: `crates/amplifier-ffi/src/handles.rs`
- Create: `crates/amplifier-ffi/src/memory.rs`
- Create: `crates/amplifier-ffi/cbindgen.toml`
- Modify: `Cargo.toml` (workspace)

**Step 1: Add crate to workspace**

In the root `Cargo.toml`, add `"crates/amplifier-ffi"` to the workspace members:

```toml
[workspace]
members = [
    "crates/amplifier-core",
    "crates/amplifier-guest",
    "crates/amplifier-ffi",
    "bindings/python",
    "bindings/node",
]
resolver = "2"
```

**Step 2: Create `crates/amplifier-ffi/Cargo.toml`**

```toml
[package]
name = "amplifier-ffi"
version = "0.1.0"
edition = "2021"
description = "C ABI layer for the Amplifier Rust kernel"
license = "MIT"
publish = false

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
amplifier-core = { path = "../amplifier-core" }
tokio = { version = "1", features = ["rt-multi-thread", "sync", "time"] }
serde_json = "1"
log = "0.4"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

**Step 3: Create `crates/amplifier-ffi/cbindgen.toml`**

```toml
language = "C"
header = "/* Generated by cbindgen — do not edit manually. */"
include_guard = "AMPLIFIER_FFI_H"
autogen_warning = "/* Warning: this file was auto-generated by cbindgen. Do not modify this manually. */"

[export]
prefix = "Amplifier"

[export.rename]
"AmplifierResult" = "AmplifierResult"
"AmplifierHandle" = "AmplifierHandle"
```

**Step 4: Create `crates/amplifier-ffi/src/handles.rs`**

```rust
//! Opaque handle management for C ABI consumers.
//!
//! Every kernel object exposed over the FFI boundary is an opaque `*mut c_void`.
//! Internally we store the real Rust object in an `Arc` to allow safe sharing.

use std::sync::Arc;

/// Opaque handle type exposed to C callers.
pub type AmplifierHandle = *mut std::ffi::c_void;

/// Result code returned by every FFI function. 0 = success.
pub type AmplifierResult = i32;

pub const AMPLIFIER_OK: AmplifierResult = 0;
pub const AMPLIFIER_ERR_NULL_HANDLE: AmplifierResult = -1;
pub const AMPLIFIER_ERR_INVALID_JSON: AmplifierResult = -2;
pub const AMPLIFIER_ERR_RUNTIME: AmplifierResult = -3;
pub const AMPLIFIER_ERR_SESSION: AmplifierResult = -4;
pub const AMPLIFIER_ERR_INTERNAL: AmplifierResult = -99;

/// Box an Arc'd value into an opaque handle.
pub fn arc_to_handle<T: Send + Sync + 'static>(val: Arc<T>) -> AmplifierHandle {
    Arc::into_raw(val) as AmplifierHandle
}

/// Recover an Arc from an opaque handle WITHOUT consuming it.
///
/// # Safety
/// The handle must have been created by `arc_to_handle` for the same type T.
pub unsafe fn handle_to_arc_ref<T: Send + Sync + 'static>(handle: AmplifierHandle) -> Arc<T> {
    let ptr = handle as *const T;
    // Increment refcount (we don't want to drop the original)
    Arc::increment_strong_count(ptr);
    Arc::from_raw(ptr)
}

/// Recover an Arc from an opaque handle, consuming it (for destroy).
///
/// # Safety
/// The handle must have been created by `arc_to_handle` for the same type T.
/// After this call, the handle is invalid.
pub unsafe fn handle_to_arc_owned<T: Send + Sync + 'static>(handle: AmplifierHandle) -> Arc<T> {
    Arc::from_raw(handle as *const T)
}
```

**Step 5: Create `crates/amplifier-ffi/src/memory.rs`**

```rust
//! Memory management and error reporting for C ABI consumers.

use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Store an error message for retrieval by `amplifier_last_error`.
pub(crate) fn set_last_error(msg: &str) {
    let c_str = CString::new(msg).unwrap_or_else(|_| CString::new("(error contained null byte)").unwrap());
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(c_str);
    });
}

/// Retrieve the last error message (thread-local).
///
/// Returns a pointer to a null-terminated string. The pointer is valid until
/// the next FFI call on the same thread. Returns a pointer to an empty string
/// if no error has been set.
#[no_mangle]
pub extern "C" fn amplifier_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| {
        match cell.borrow().as_ref() {
            Some(msg) => msg.as_ptr(),
            None => c"".as_ptr(),
        }
    })
}

/// Free a string that was allocated by the library (e.g., from `amplifier_session_execute`).
///
/// # Safety
/// `ptr` must be a pointer returned by a previous FFI call, or null (no-op).
/// Double-free is safe (we check for null).
#[no_mangle]
pub unsafe extern "C" fn amplifier_string_free(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Allocate a CString from a Rust String and return its raw pointer.
///
/// The caller is responsible for freeing with `amplifier_string_free`.
pub(crate) fn string_to_c(s: String) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => {
            set_last_error("String contained null byte");
            std::ptr::null_mut()
        }
    }
}
```

**Step 6: Create `crates/amplifier-ffi/src/lib.rs`**

```rust
//! C ABI layer for the Amplifier Rust kernel.
//!
//! Exposes the kernel as `extern "C"` functions for Go (CGo), C# (P/Invoke),
//! and C++ consumers. All complex types cross the boundary as JSON strings.
//!
//! # Usage pattern
//!
//! 1. `amplifier_runtime_create` — create a Tokio runtime
//! 2. `amplifier_session_create` — create a session with JSON config
//! 3. `amplifier_session_mount_*` — mount providers/tools
//! 4. `amplifier_session_initialize` — initialize the session
//! 5. `amplifier_session_execute` — run a prompt (blocking)
//! 6. `amplifier_session_cleanup` / `amplifier_session_destroy` — teardown
//! 7. `amplifier_runtime_destroy` — destroy the Tokio runtime

pub mod handles;
pub mod memory;
pub mod runtime;
pub mod session;
pub mod coordinator;
pub mod transport;
pub mod kernel_service;
pub mod capabilities;
```

**Step 7: Verify it compiles**

```bash
cd amplifier-core && cargo check -p amplifier-ffi 2>&1 | tail -10
```

Expected: compiles (with warnings about unused modules — that's fine, we'll fill them in next tasks).

**Step 8: Commit**

```bash
git add Cargo.toml crates/amplifier-ffi/ && git commit -m "feat: scaffold amplifier-ffi crate with handle types and memory management"
```

---

### Task 11: Implement Group 1 — Runtime Create/Destroy

**Files:**
- Create: `crates/amplifier-ffi/src/runtime.rs`

**Step 1: Write the Rust test**

At the bottom of `runtime.rs` (we'll create the whole file):

```rust
//! Group 1: Tokio runtime lifecycle.

use std::sync::Arc;

use crate::handles::*;
use crate::memory::set_last_error;

/// Wrapper around a Tokio multi-thread runtime.
pub struct FfiRuntime {
    pub(crate) rt: tokio::runtime::Runtime,
}

/// Create a new Tokio multi-thread runtime.
///
/// On success, writes the runtime handle to `*out` and returns 0.
/// On failure, returns a non-zero error code and sets the thread-local error message.
///
/// # Safety
/// `out` must be a valid pointer to an `AmplifierHandle`.
#[no_mangle]
pub unsafe extern "C" fn amplifier_runtime_create(out: *mut AmplifierHandle) -> AmplifierResult {
    if out.is_null() {
        set_last_error("amplifier_runtime_create: out pointer is null");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    match tokio::runtime::Runtime::new() {
        Ok(rt) => {
            let wrapper = Arc::new(FfiRuntime { rt });
            *out = arc_to_handle(wrapper);
            AMPLIFIER_OK
        }
        Err(e) => {
            set_last_error(&format!("Failed to create Tokio runtime: {e}"));
            AMPLIFIER_ERR_RUNTIME
        }
    }
}

/// Destroy a Tokio runtime, shutting down all spawned tasks.
///
/// # Safety
/// `runtime` must be a handle returned by `amplifier_runtime_create`.
/// After this call, the handle is invalid.
#[no_mangle]
pub unsafe extern "C" fn amplifier_runtime_destroy(runtime: AmplifierHandle) -> AmplifierResult {
    if runtime.is_null() {
        set_last_error("amplifier_runtime_destroy: handle is null");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    // Consume the Arc — when refcount hits 0, Runtime::drop shuts down
    let _arc: Arc<FfiRuntime> = handle_to_arc_owned(runtime);
    AMPLIFIER_OK
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_create_destroy_roundtrip() {
        let mut handle: AmplifierHandle = std::ptr::null_mut();
        unsafe {
            let rc = amplifier_runtime_create(&mut handle);
            assert_eq!(rc, AMPLIFIER_OK);
            assert!(!handle.is_null());

            let rc = amplifier_runtime_destroy(handle);
            assert_eq!(rc, AMPLIFIER_OK);
        }
    }

    #[test]
    fn runtime_null_out_returns_error() {
        unsafe {
            let rc = amplifier_runtime_create(std::ptr::null_mut());
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }

    #[test]
    fn runtime_destroy_null_returns_error() {
        unsafe {
            let rc = amplifier_runtime_destroy(std::ptr::null_mut());
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

**Step 2: Run the tests**

```bash
cd amplifier-core && cargo test -p amplifier-ffi 2>&1 | tail -15
```

Expected: 3 tests PASS.

**Step 3: Commit**

```bash
git add crates/amplifier-ffi/src/runtime.rs && git commit -m "feat(ffi): implement Group 1 — runtime create/destroy"
```

---

### Task 12: Implement Group 2 — Session Create/Execute/Destroy

**Files:**
- Create: `crates/amplifier-ffi/src/session.rs`

**Step 1: Implement session functions**

```rust
//! Group 2: Session lifecycle — create, initialize, execute, cleanup, destroy.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::{Arc, Mutex};

use crate::handles::*;
use crate::memory::{set_last_error, string_to_c};
use crate::runtime::FfiRuntime;

/// Wrapper around a kernel Session, protected by a Mutex for thread safety.
pub struct FfiSession {
    pub(crate) runtime: Arc<FfiRuntime>,
    pub(crate) session: Mutex<amplifier_core::Session>,
}

/// Create a new session from a JSON config string.
///
/// # Safety
/// - `runtime` must be a valid handle from `amplifier_runtime_create`.
/// - `config_json` must be a valid null-terminated UTF-8 string.
/// - `out` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_create(
    runtime: AmplifierHandle,
    config_json: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || config_json.is_null() || out.is_null() {
        set_last_error("amplifier_session_create: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let rt_arc: Arc<FfiRuntime> = handle_to_arc_ref(runtime);

    let config_str = match CStr::from_ptr(config_json).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in config_json: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    let config: std::collections::HashMap<String, serde_json::Value> =
        match serde_json::from_str(config_str) {
            Ok(c) => c,
            Err(e) => {
                set_last_error(&format!("Invalid JSON config: {e}"));
                return AMPLIFIER_ERR_INVALID_JSON;
            }
        };

    // Convert Value map to String map (Session expects HashMap<String, String>)
    let string_config: std::collections::HashMap<String, String> = config
        .into_iter()
        .map(|(k, v)| (k, v.as_str().unwrap_or(&v.to_string()).to_string()))
        .collect();

    let session = amplifier_core::Session::new(string_config);
    let wrapper = Arc::new(FfiSession {
        runtime: rt_arc,
        session: Mutex::new(session),
    });
    *out = arc_to_handle(wrapper);
    AMPLIFIER_OK
}

/// Destroy a session handle.
///
/// # Safety
/// `session` must be a valid handle from `amplifier_session_create`.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_destroy(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_destroy: handle is null");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    let _arc: Arc<FfiSession> = handle_to_arc_owned(session);
    AMPLIFIER_OK
}

/// Initialize the session (calls mount on all registered modules).
///
/// Blocks the calling thread.
///
/// # Safety
/// `session` must be a valid session handle.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_initialize(
    session: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_initialize: handle is null");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    let sess_arc: Arc<FfiSession> = handle_to_arc_ref(session);
    let guard = match sess_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!("Session lock poisoned: {e}"));
            return AMPLIFIER_ERR_INTERNAL;
        }
    };
    match sess_arc.runtime.rt.block_on(guard.initialize()) {
        Ok(()) => AMPLIFIER_OK,
        Err(e) => {
            set_last_error(&format!("Session initialize failed: {e}"));
            AMPLIFIER_ERR_SESSION
        }
    }
}

/// Execute a prompt and return the response as a JSON string.
///
/// Blocks the calling thread. The caller must free `*out_json` with `amplifier_string_free`.
///
/// # Safety
/// - `session` must be a valid session handle.
/// - `prompt` must be a valid null-terminated UTF-8 string.
/// - `out_json` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_execute(
    session: AmplifierHandle,
    prompt: *const c_char,
    out_json: *mut *mut c_char,
) -> AmplifierResult {
    if session.is_null() || prompt.is_null() || out_json.is_null() {
        set_last_error("amplifier_session_execute: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let sess_arc: Arc<FfiSession> = handle_to_arc_ref(session);

    let prompt_str = match CStr::from_ptr(prompt).to_str() {
        Ok(s) => s.to_string(),
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in prompt: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    let guard = match sess_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!("Session lock poisoned: {e}"));
            return AMPLIFIER_ERR_INTERNAL;
        }
    };

    match sess_arc.runtime.rt.block_on(guard.execute(&prompt_str)) {
        Ok(response) => {
            // The response is a String from the kernel
            *out_json = string_to_c(response);
            AMPLIFIER_OK
        }
        Err(e) => {
            set_last_error(&format!("Session execute failed: {e}"));
            AMPLIFIER_ERR_SESSION
        }
    }
}

/// Cleanup the session (call cleanup hooks, unmount modules).
///
/// Blocks the calling thread.
///
/// # Safety
/// `session` must be a valid session handle.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_cleanup(session: AmplifierHandle) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_session_cleanup: handle is null");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    let sess_arc: Arc<FfiSession> = handle_to_arc_ref(session);
    let guard = match sess_arc.session.lock() {
        Ok(g) => g,
        Err(e) => {
            set_last_error(&format!("Session lock poisoned: {e}"));
            return AMPLIFIER_ERR_INTERNAL;
        }
    };
    match sess_arc.runtime.rt.block_on(guard.cleanup()) {
        Ok(()) => AMPLIFIER_OK,
        Err(e) => {
            set_last_error(&format!("Session cleanup failed: {e}"));
            AMPLIFIER_ERR_SESSION
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::amplifier_runtime_create;

    #[test]
    fn session_create_destroy_roundtrip() {
        unsafe {
            let mut rt_handle: AmplifierHandle = std::ptr::null_mut();
            let rc = amplifier_runtime_create(&mut rt_handle);
            assert_eq!(rc, AMPLIFIER_OK);

            let config = std::ffi::CString::new("{}").unwrap();
            let mut sess_handle: AmplifierHandle = std::ptr::null_mut();
            let rc = amplifier_session_create(rt_handle, config.as_ptr(), &mut sess_handle);
            assert_eq!(rc, AMPLIFIER_OK);
            assert!(!sess_handle.is_null());

            let rc = amplifier_session_destroy(sess_handle);
            assert_eq!(rc, AMPLIFIER_OK);

            let rc = crate::runtime::amplifier_runtime_destroy(rt_handle);
            assert_eq!(rc, AMPLIFIER_OK);
        }
    }

    #[test]
    fn session_null_args_return_error() {
        unsafe {
            let rc = amplifier_session_create(
                std::ptr::null_mut(),
                std::ptr::null(),
                std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

> **Note to implementer:** The exact `Session::new()`, `session.initialize()`, `session.execute()`, and `session.cleanup()` APIs depend on the kernel's public Rust API. Check `crates/amplifier-core/src/session.rs` for the actual constructor signature — it may take a `HashMap<String, String>` or a config struct. Adapt the `amplifier_session_create` function accordingly.

**Step 2: Run tests**

```bash
cd amplifier-core && cargo test -p amplifier-ffi 2>&1 | tail -15
```

Expected: all session + runtime tests pass.

**Step 3: Commit**

```bash
git add crates/amplifier-ffi/src/session.rs && git commit -m "feat(ffi): implement Group 2 — session create/execute/destroy"
```

---

### Task 13: Implement Group 3 — Coordinator Mount Points

**Files:**
- Create: `crates/amplifier-ffi/src/coordinator.rs`

**Step 1: Implement mount functions**

```rust
//! Group 3: Coordinator mount points — mount providers, tools, orchestrators, context.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Arc;

use crate::handles::*;
use crate::memory::set_last_error;
use crate::session::FfiSession;

/// Mount a provider into a session.
///
/// # Safety
/// - `session` must be a valid session handle.
/// - `provider` must be a valid provider handle (from `amplifier_load_grpc_provider`).
/// - `name` must be a valid null-terminated UTF-8 string.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_mount_provider(
    session: AmplifierHandle,
    provider: AmplifierHandle,
    name: *const c_char,
) -> AmplifierResult {
    if session.is_null() || provider.is_null() || name.is_null() {
        set_last_error("amplifier_session_mount_provider: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let _name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in name: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    // TODO: Implement actual provider mounting via Session/Coordinator API.
    // This requires the kernel's public API for mounting trait objects.
    // For now, this is a scaffold that validates arguments.
    set_last_error("amplifier_session_mount_provider: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Mount a tool into a session.
///
/// # Safety
/// Same as `amplifier_session_mount_provider`.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_mount_tool(
    session: AmplifierHandle,
    tool: AmplifierHandle,
    name: *const c_char,
) -> AmplifierResult {
    if session.is_null() || tool.is_null() || name.is_null() {
        set_last_error("amplifier_session_mount_tool: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let _name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in name: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    // TODO: Implement actual tool mounting
    set_last_error("amplifier_session_mount_tool: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Set the orchestrator for a session.
///
/// # Safety
/// Same as `amplifier_session_mount_provider` (minus `name`).
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_set_orchestrator(
    session: AmplifierHandle,
    orchestrator: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() || orchestrator.is_null() {
        set_last_error("amplifier_session_set_orchestrator: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    // TODO: Implement actual orchestrator setting
    set_last_error("amplifier_session_set_orchestrator: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Set the context module for a session.
///
/// # Safety
/// Same as `amplifier_session_set_orchestrator`.
#[no_mangle]
pub unsafe extern "C" fn amplifier_session_set_context(
    session: AmplifierHandle,
    context: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() || context.is_null() {
        set_last_error("amplifier_session_set_context: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    // TODO: Implement actual context module setting
    set_last_error("amplifier_session_set_context: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mount_null_args_return_error() {
        unsafe {
            let rc = amplifier_session_mount_provider(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);

            let rc = amplifier_session_mount_tool(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);

            let rc = amplifier_session_set_orchestrator(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);

            let rc = amplifier_session_set_context(
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

> **Note:** The mount functions are scaffolded with argument validation and TODOs. Filling in the actual kernel Coordinator API calls depends on what `Session`/`Coordinator` exposes publicly. Check `crates/amplifier-core/src/coordinator.rs` and `crates/amplifier-core/src/session.rs` for the actual mount methods. The scaffolds compile and validate all arguments; the kernel integration is the next layer.

**Step 2: Run tests**

```bash
cd amplifier-core && cargo test -p amplifier-ffi 2>&1 | tail -15
```

Expected: all tests pass.

**Step 3: Commit**

```bash
git add crates/amplifier-ffi/src/coordinator.rs && git commit -m "feat(ffi): scaffold Group 3 — coordinator mount points"
```

---

### Task 14: Implement Groups 4–6 — Transport, KernelService, Capabilities

**Files:**
- Create: `crates/amplifier-ffi/src/transport.rs`
- Create: `crates/amplifier-ffi/src/kernel_service.rs`
- Create: `crates/amplifier-ffi/src/capabilities.rs`

**Step 1: Create `crates/amplifier-ffi/src/transport.rs`**

```rust
//! Group 4: gRPC transport loaders.
//!
//! Each function connects to a remote gRPC module and returns an opaque handle
//! that can be mounted via the Group 3 functions.

use std::ffi::CStr;
use std::os::raw::c_char;

use crate::handles::*;
use crate::memory::set_last_error;

/// Load a gRPC provider from an endpoint.
///
/// # Safety
/// - `runtime` must be a valid runtime handle.
/// - `endpoint` must be a valid null-terminated UTF-8 string (e.g., "127.0.0.1:50051").
/// - `out` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_provider(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_provider: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    let _endpoint_str = match CStr::from_ptr(endpoint).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in endpoint: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };
    // TODO: Use GrpcProviderBridge to connect to the endpoint
    set_last_error("amplifier_load_grpc_provider: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Load a gRPC tool from an endpoint.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_tool(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_tool: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    set_last_error("amplifier_load_grpc_tool: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Load a gRPC orchestrator from an endpoint.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_orchestrator(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    session_id: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || session_id.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_orchestrator: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    set_last_error("amplifier_load_grpc_orchestrator: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Load a gRPC hook module from an endpoint.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_hook(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_hook: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    set_last_error("amplifier_load_grpc_hook: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Load a gRPC context module from an endpoint.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_context(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_context: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    set_last_error("amplifier_load_grpc_context: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Load a gRPC approval module from an endpoint.
#[no_mangle]
pub unsafe extern "C" fn amplifier_load_grpc_approval(
    runtime: AmplifierHandle,
    endpoint: *const c_char,
    out: *mut AmplifierHandle,
) -> AmplifierResult {
    if runtime.is_null() || endpoint.is_null() || out.is_null() {
        set_last_error("amplifier_load_grpc_approval: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    set_last_error("amplifier_load_grpc_approval: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_null_args_return_error() {
        unsafe {
            let rc = amplifier_load_grpc_provider(
                std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

**Step 2: Create `crates/amplifier-ffi/src/kernel_service.rs`**

```rust
//! Group 5: KernelService — start/stop the gRPC callback server.

use std::os::raw::c_char;

use crate::handles::*;
use crate::memory::set_last_error;

/// Start the KernelService gRPC server for a session.
///
/// Returns an auth token via `*out_token` that out-of-process modules use
/// for callback RPCs. The caller must free `*out_token` with `amplifier_string_free`.
///
/// # Safety
/// - `session` must be a valid session handle.
/// - `out_token` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn amplifier_kernel_service_start(
    session: AmplifierHandle,
    port: u16,
    out_token: *mut *mut c_char,
) -> AmplifierResult {
    if session.is_null() || out_token.is_null() {
        set_last_error("amplifier_kernel_service_start: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    // TODO: Start the gRPC KernelService server via grpc_server.rs
    set_last_error("amplifier_kernel_service_start: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Stop the KernelService gRPC server for a session.
///
/// # Safety
/// `session` must be a valid session handle.
#[no_mangle]
pub unsafe extern "C" fn amplifier_kernel_service_stop(
    session: AmplifierHandle,
) -> AmplifierResult {
    if session.is_null() {
        set_last_error("amplifier_kernel_service_stop: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }
    // TODO: Stop the gRPC server
    set_last_error("amplifier_kernel_service_stop: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_service_null_args_return_error() {
        unsafe {
            let rc = amplifier_kernel_service_start(
                std::ptr::null_mut(), 0, std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

**Step 3: Create `crates/amplifier-ffi/src/capabilities.rs`**

```rust
//! Group 6: Capabilities — register and get capabilities.

use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::Arc;

use crate::handles::*;
use crate::memory::{set_last_error, string_to_c};
use crate::session::FfiSession;

/// Register a capability (JSON value) with the session's coordinator.
///
/// # Safety
/// - `session` must be a valid session handle.
/// - `name` and `value_json` must be valid null-terminated UTF-8 strings.
#[no_mangle]
pub unsafe extern "C" fn amplifier_register_capability(
    session: AmplifierHandle,
    name: *const c_char,
    value_json: *const c_char,
) -> AmplifierResult {
    if session.is_null() || name.is_null() || value_json.is_null() {
        set_last_error("amplifier_register_capability: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let _name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in name: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    let _value_str = match CStr::from_ptr(value_json).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in value_json: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    // TODO: Call coordinator.register_capability(name, value)
    set_last_error("amplifier_register_capability: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

/// Get a capability value (as JSON string) from the session's coordinator.
///
/// The caller must free `*out_json` with `amplifier_string_free`.
///
/// # Safety
/// - `session` must be a valid session handle.
/// - `name` must be a valid null-terminated UTF-8 string.
/// - `out_json` must be a valid pointer.
#[no_mangle]
pub unsafe extern "C" fn amplifier_get_capability(
    session: AmplifierHandle,
    name: *const c_char,
    out_json: *mut *mut c_char,
) -> AmplifierResult {
    if session.is_null() || name.is_null() || out_json.is_null() {
        set_last_error("amplifier_get_capability: null argument");
        return AMPLIFIER_ERR_NULL_HANDLE;
    }

    let _name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(e) => {
            set_last_error(&format!("Invalid UTF-8 in name: {e}"));
            return AMPLIFIER_ERR_INVALID_JSON;
        }
    };

    // TODO: Call coordinator.get_capability(name) and serialize to JSON
    set_last_error("amplifier_get_capability: not yet fully implemented");
    AMPLIFIER_ERR_INTERNAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_null_args_return_error() {
        unsafe {
            let rc = amplifier_register_capability(
                std::ptr::null_mut(), std::ptr::null(), std::ptr::null(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);

            let rc = amplifier_get_capability(
                std::ptr::null_mut(), std::ptr::null(), std::ptr::null_mut(),
            );
            assert_eq!(rc, AMPLIFIER_ERR_NULL_HANDLE);
        }
    }
}
```

**Step 4: Run all FFI tests**

```bash
cd amplifier-core && cargo test -p amplifier-ffi 2>&1 | tail -15
```

Expected: all tests pass (runtime + session + coordinator + transport + kernel_service + capabilities).

**Step 5: Commit**

```bash
git add crates/amplifier-ffi/src/transport.rs crates/amplifier-ffi/src/kernel_service.rs crates/amplifier-ffi/src/capabilities.rs && git commit -m "feat(ffi): scaffold Groups 4-6 — transport, kernel service, capabilities"
```

---

### Task 15: Generate C Header and Verify Compilation

**Files:**
- Generated: `include/amplifier_ffi.h` (new directory)

**Step 1: Install cbindgen if not present**

```bash
cargo install cbindgen 2>&1 | tail -5
```

**Step 2: Generate the C header**

```bash
cd amplifier-core && mkdir -p include && cbindgen --config crates/amplifier-ffi/cbindgen.toml --crate amplifier-ffi --output include/amplifier_ffi.h 2>&1
```

Expected: generates `include/amplifier_ffi.h` with all 25 function declarations.

**Step 3: Verify the header contains all function groups**

```bash
grep -c "amplifier_" include/amplifier_ffi.h
```

Expected: at least 25 matches (one per function plus extras for types).

```bash
grep "amplifier_runtime_create\|amplifier_session_create\|amplifier_session_execute\|amplifier_string_free\|amplifier_last_error" include/amplifier_ffi.h
```

Expected: all 5 key functions present.

**Step 4: Verify the cdylib builds**

```bash
cd amplifier-core && cargo build -p amplifier-ffi --release 2>&1 | tail -10
```

Expected: builds successfully, producing `target/release/libamplifier_ffi.so` (Linux) or `target/release/libamplifier_ffi.dylib` (macOS).

```bash
ls -la target/release/libamplifier_ffi.* 2>/dev/null || ls -la target/release/libamplifier_ffi* 2>/dev/null
```

Expected: shows the shared library file.

**Step 5: Commit**

```bash
git add include/amplifier_ffi.h crates/amplifier-ffi/cbindgen.toml && git commit -m "feat(ffi): generate C header via cbindgen — 25 extern C functions"
```

---

## Block F: Version + Release (Task 16)

### Task 16: Full Test Suite and Version Bump

**Files:**
- Modify: `crates/amplifier-core/Cargo.toml` (version)
- Modify: `bindings/python/Cargo.toml` (version)
- Modify: `pyproject.toml` (version)

**Step 1: Run all Rust tests**

```bash
cd amplifier-core && cargo test 2>&1 | tail -30
```

Expected: all tests pass across all crates (amplifier-core, amplifier-core-py, amplifier-ffi).

**Step 2: Run all Python tests**

```bash
cd amplifier-core && uv run pytest tests/ bindings/python/tests/ -q 2>&1 | tail -20
```

Expected: all tests pass.

**Step 3: Bump version**

In `crates/amplifier-core/Cargo.toml`, change `version = "1.2.6"` to `version = "1.3.0"`.
In `bindings/python/Cargo.toml`, change `version = "1.2.6"` to `version = "1.3.0"`.
In `pyproject.toml`, change `version = "1.2.6"` to `version = "1.3.0"`.

**Step 4: Final commit**

```bash
cd amplifier-core && git add -A && git commit -m "release: v1.3.0 — proto content_blocks, PyO3 bridge, hooks thinning, C ABI scaffold"
```

**Step 5: Tag**

```bash
cd amplifier-core && git tag v1.3.0
```
