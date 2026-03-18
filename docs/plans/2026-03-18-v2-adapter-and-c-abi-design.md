# gRPC Adapter v2 + C ABI Layer (Phase 5A) Design

## Goal

Two coordinated objectives sharing a single amplifier-core release:

1. **Fix gRPC adapter v1 limitations** — proto evolution, proper type translation, CompleteStreaming, coordinator access for out-of-process modules, hooks.py thinning
2. **Enable Go/C#/C++ hosts** — C ABI layer (`crates/amplifier-ffi/`) exposing the Rust kernel via `extern "C"` functions

## Background

The gRPC adapter (v1) shipped with several known limitations that restrict out-of-process Python modules. `ChatResponse` uses a flat JSON string for content instead of the typed `ContentBlock` messages the rest of the proto uses. Proto-to-Pydantic translation is done manually in Python, duplicating 2,377 lines of Rust `conversions.rs` logic. `CompleteStreaming` returns `UNIMPLEMENTED`. Out-of-process modules have no way to call back into the kernel for hooks or capabilities. And the Python `HookRegistry` (348 lines) is a near-complete duplicate of the Rust `RustHookRegistry`.

Separately, Go and C# hosts currently have no way to embed the Rust kernel. A C ABI layer would unlock these languages without requiring each to maintain gRPC adapters.

Both sub-projects require amplifier-core changes and benefit from a single coordinated release.

### Ecosystem Investigation

Before designing, we investigated 13 repos (amplifier-app-cli, amplifier-bundle-distro, all 7 provider modules, all 3 orchestrators, 6 hook modules). Key findings that shaped the design:

- 5 of 7 providers subclass `ChatResponse` and define `content_blocks` as a field using streaming dataclass types (`TextContent`, `ThinkingContent`) — a different type hierarchy from the base `ChatResponse.content` (Pydantic `ContentBlockUnion`). We cannot add `content_blocks` to the base Pydantic model.
- All providers call `coordinator.mount("providers", self)` during `mount()` — this is self-registration that must be handled kernel-side for out-of-process modules.
- Python `HookRegistry` (348 lines) duplicates `RustHookRegistry` — only one real API gap: `emit_and_collect()` returns JSON strings from Rust vs dicts from Python.
- `resolve_module()` already returns `module_type` from Rust, but hardcodes `ModuleType::Tool` for Python modules — the optimization to extend it is less valuable than initially thought.
- `PreparedBundle` refactor must maintain `from amplifier_foundation.bundle import PreparedBundle` as a valid import (12 source files in app-cli depend on it).

## Approach

**Sub-project A (gRPC adapter v2):** Rather than building Python translation layers or proxy objects, we expose existing Rust logic to Python via PyO3 and use the kernel's established patterns for cross-process communication. This follows the kernel's §5 principle: "Logic goes in Rust, not in bindings. Don't duplicate logic across languages."

**Sub-project B (C ABI):** Opaque handles + JSON for complex types + blocking calls for v1. This is the simplest possible FFI surface that lets Go/C#/C++ create sessions, mount modules, and execute prompts without understanding Rust internals.

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                    amplifier-core                         │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌───────────────┐  │
│  │ conversions.rs│  │   hooks.rs   │  │amplifier-ffi/ │  │
│  │  (existing)   │  │  (fix emit)  │  │  (new crate)  │  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬────────┘  │
│         │                  │                  │           │
│  ┌──────┴───────┐  ┌──────┴───────┐  ┌──────┴────────┐  │
│  │ PyO3 bridge  │  │  3-line alias │  │  C headers    │  │
│  │ 2 functions  │  │  hooks.py     │  │  (cbindgen)   │  │
│  └──────┬───────┘  └──────────────┘  └──────┬────────┘  │
│         │                                    │           │
└─────────┼────────────────────────────────────┼───────────┘
          │                                    │
  ┌───────┴────────────┐            ┌──────────┴──────────┐
  │ amplifier-foundation│            │  Go / C# / C++ host │
  │  services.py        │            │  via libamplifier.so │
  │  (thin adapter)     │            │                      │
  └────────────────────┘            └─────────────────────┘
```

The PyO3 bridge gives the Python adapter zero-maintenance type translation. The C ABI gives non-Rust hosts the same kernel capabilities. Both share the same `conversions.rs` and kernel internals — no logic duplication.

## Components

### Section 1: Proto Evolution

Add `repeated ContentBlock content_blocks = 7` to `ChatResponse` in `proto/amplifier_module.proto`:

```protobuf
message ChatResponse {
  string                    content        = 1;  // DEPRECATED — keep for backward compat
  repeated ToolCallMessage  tool_calls     = 2;
  Usage                     usage          = 3;
  Degradation               degradation    = 4;
  string                    finish_reason  = 5;
  string                    metadata_json  = 6;
  repeated ContentBlock     content_blocks = 7;  // NEW: typed, replaces JSON string
}
```

The proto already defines all 7 typed `ContentBlock` messages (`TextBlock`, `ThinkingBlock`, etc.) and the `ContentBlock` oneof wrapper. The `Message` type already uses them. Only `ChatResponse` was left behind with a flat `content: string`.

**Conversion strategy in `conversions.rs`:**

- **Write path:** Populate BOTH `content` (JSON string, legacy) and `content_blocks` (typed). Dual-write ensures backward compatibility.
- **Read path:** Prefer `content_blocks` if non-empty, fall back to JSON-deserializing `content`.

**ChatRequest input path:** Already handled — `Message` already has typed `ContentBlock` via `oneof content { text_content | block_content }`. No changes needed.

**Blast radius:** Additive field — no existing consumers break. Field 7 is available. Requires regenerating Rust and Python proto stubs.

### Section 2: PyO3 Bridge for Proto-to-Pydantic Translation

Expose `conversions.rs` to Python via two new PyO3 functions. This eliminates the need for any Python translation code.

**New Rust functions (~50 lines in `bindings/python/`):**

```rust
#[pyfunction]
fn proto_chat_request_to_json(proto_bytes: &[u8]) -> PyResult<String> {
    let proto = ChatRequest::decode(proto_bytes)?;
    let native = proto_chat_request_to_native(proto);
    Ok(serde_json::to_string(&native)?)
}

#[pyfunction]
fn json_to_proto_chat_response(json: &str) -> PyResult<Vec<u8>> {
    let native: messages::ChatResponse = serde_json::from_str(json)?;
    let proto = native_chat_response_to_proto(&native);
    Ok(proto.encode_to_vec())
}
```

**Python adapter becomes trivially thin:**

```python
from amplifier_core._engine import proto_chat_request_to_json, json_to_proto_chat_response

async def Complete(self, request, context):
    json_str = proto_chat_request_to_json(request.SerializeToString())
    native_request = ChatRequest.model_validate_json(json_str)
    response = await _invoke(self._provider.complete, native_request)
    return ChatResponse.FromString(json_to_proto_chat_response(response.model_dump_json()))
```

**Why NOT a Python translation layer:** The Rust `conversions.rs` (2,377 lines) already does the complete proto-to-native translation. Reimplementing this in Python — even with auto-discovery from proto descriptors — creates a second source of truth that drifts. The PyO3 bridge gives zero Python translation code and zero maintenance when block types change.

**Important field naming:** Proto `content_blocks` maps to Pydantic `content` — different field names, same semantic content. The base Pydantic `ChatResponse` does NOT get a `content_blocks` field (5 provider subclasses already use that name for streaming dataclass types — incompatible type hierarchy).

### Section 3: hooks.py Thin Re-export

Replace 348-line `python/amplifier_core/hooks.py` with a 3-line alias:

```python
from amplifier_core._engine import RustHookRegistry as HookRegistry
from amplifier_core.models import HookResult

__all__ = ["HookRegistry", "HookResult"]
```

**Rust fix (~5 lines in `bindings/python/src/hooks.rs`):** Change `emit_and_collect()` to return `list[dict]` instead of `list[str]` — deserialize JSON strings to Python dicts before returning, matching the pattern `emit()` already uses.

**Why a simple alias works (no subclass needed):**

- `from amplifier_core.hooks import HookRegistry` resolves to `RustHookRegistry` — Python doesn't distinguish aliases from the real class
- `HookRegistry.SESSION_END` works — `#[classattr]` lives on the class object, accessible via either name
- Nobody accesses `_handlers` or Python-specific internals (confirmed across 13 repos)
- `HookResult` re-exported for backward compatibility

**Ecosystem impact confirmed safe:**

- `hooks-backup`: uses `HookRegistry.CONTEXT_PRE_COMPACT` — inherited from Rust `#[classattr]`
- `hooks-approval`: imports `HookRegistry` by name — re-export preserves path
- All 3 orchestrators: `from amplifier_core import HookRegistry` for type hints — works
- `session_spawner.py`: `from amplifier_core.hooks import HookResult` — re-exported

**Also fix:** Sync `.pyi` stub (lists 16 constants but only 8 exist in Rust source).

### Section 4: CompleteStreaming (Simulated)

Call `provider.complete()` (non-streaming), return the full `ChatResponse` as a single stream element. ~5 lines beyond what `Complete()` already does:

```python
async def CompleteStreaming(self, request, context):
    """Simulated streaming — returns full response as single stream element."""
    json_str = proto_chat_request_to_json(request.SerializeToString())
    native_request = ChatRequest.model_validate_json(json_str)
    response = await _invoke(self._provider.complete, native_request)
    yield chat_response_to_proto(response)
```

**Why simulated:** `stream()` is NOT on the `Provider` protocol — it's a duck-typed optional extension. The Rust kernel's own `CompleteWithProviderStreaming` is already simulated (one element). Going from 1 stream element to N is backward-compatible — when real streaming is added later, the wire contract doesn't change.

### Section 5: Direct KernelService Usage (No Coordinator Proxy)

The gRPC adapter does NOT build a `GrpcCoordinatorProxy`. The coordinator is a convenience API for in-process Python modules. The cross-process API is `KernelService`. That's not a gap — it's a deliberate architectural boundary.

All three experts independently reached this conclusion by examining the existing gRPC bridge architecture:

- Every gRPC bridge (Tool, Provider, Hook, Context, Orchestrator, Approval) follows the same pattern — host creates a bridge, mounts it kernel-side, remote module uses `KernelService` RPCs for callbacks
- The orchestrator bridge's code explicitly discards the coordinator parameter (`_coordinator: Value`)
- `MountRequest` in the proto intentionally has no coordinator field

**For providers (the primary target):**

A thin `KernelClient` (~15 lines) wraps the `KernelService` gRPC stub:

```python
class KernelClient:
    """Direct kernel callback client for out-of-process modules."""
    def __init__(self, stub, metadata, session_id):
        self._stub = stub
        self._metadata = metadata
        self.session_id = session_id

    async def emit_hook(self, event, data=None):
        return await self._stub.EmitHook(
            EmitHookRequest(...), metadata=self._metadata
        )

    async def get_capability(self, name):
        resp = await self._stub.GetCapability(
            GetCapabilityRequest(name=name), metadata=self._metadata
        )
        return json.loads(resp.value_json) if resp.found else None
```

**For existing Python providers that call `coordinator.*` during `mount()`:**

A `SimpleNamespace` shim with no-ops for mount-time operations (since the kernel handles registration for out-of-process modules):

```python
shim = SimpleNamespace(
    hooks=SimpleNamespace(emit=kernel_client.emit_hook),
    get_capability=kernel_client.get_capability,
    session_id=kernel_client.session_id,
    parent_id=kernel_client.parent_id,
    mount=lambda *args, **kwargs: logger.debug(
        "mount() is a no-op — kernel registers the bridge automatically"
    ),
    register_contributor=lambda *args, **kwargs: logger.debug(
        "register_contributor() not available over gRPC"
    ),
    register_cleanup=lambda *args, **kwargs: None,
)
result = await module.mount(shim, config)
```

**Module type support:**

| Module Type | Coverage | Key Operations |
|---|---|---|
| **Providers** | Run unmodified | `mount()`=no-op, `hooks.emit()`=RPC, `get_capability()`=RPC |
| **Hook modules** | Kernel-pulls-subscriptions | `HookService.GetSubscriptions` — already implemented for gRPC hooks |
| **Orchestrators** | Stay in-process | Too tightly coupled to kernel internals (`process_hook_result`, cancellation, `register_contributor`) |

### Section 6: C ABI Layer (Phase 5A)

New `crates/amplifier-ffi/` crate producing `libamplifier.so` / `amplifier.dll` / `libamplifier.dylib`. Enables Go (CGo), C# (P/Invoke), and C++ to use the Rust kernel directly.

**Core pattern:** Opaque handles + JSON for complex types + blocking calls for v1.

```c
// Every kernel object is an opaque pointer
typedef void* AmplifierHandle;

// Every function returns an error code
typedef int32_t AmplifierResult;  // 0 = success, non-zero = error

// Complex types cross the boundary as JSON strings
// The caller provides a buffer, the function fills it
```

**25 functions across 7 groups:**

#### Group 1: Runtime (Tokio lifecycle)

```c
AmplifierResult amplifier_runtime_create(AmplifierHandle* out);
AmplifierResult amplifier_runtime_destroy(AmplifierHandle runtime);
```

Creates/destroys the Tokio runtime. All other calls use this runtime internally.

#### Group 2: Session

```c
AmplifierResult amplifier_session_create(AmplifierHandle runtime, const char* config_json,
                                         AmplifierHandle* out);
AmplifierResult amplifier_session_destroy(AmplifierHandle session);
AmplifierResult amplifier_session_execute(AmplifierHandle session, const char* prompt,
                                          char** out_json);
AmplifierResult amplifier_session_initialize(AmplifierHandle session);
AmplifierResult amplifier_session_cleanup(AmplifierHandle session);
```

`execute` blocks the calling thread (Go: `runtime.LockOSThread()`). Returns response as JSON string.

#### Group 3: Coordinator (mount points)

```c
AmplifierResult amplifier_session_mount_provider(AmplifierHandle session,
                                                  AmplifierHandle provider, const char* name);
AmplifierResult amplifier_session_mount_tool(AmplifierHandle session,
                                              AmplifierHandle tool, const char* name);
AmplifierResult amplifier_session_set_orchestrator(AmplifierHandle session,
                                                    AmplifierHandle orchestrator);
AmplifierResult amplifier_session_set_context(AmplifierHandle session,
                                               AmplifierHandle context);
```

#### Group 4: gRPC Transport Loaders

```c
AmplifierResult amplifier_load_grpc_provider(AmplifierHandle runtime, const char* endpoint,
                                              AmplifierHandle* out);
AmplifierResult amplifier_load_grpc_tool(AmplifierHandle runtime, const char* endpoint,
                                          AmplifierHandle* out);
AmplifierResult amplifier_load_grpc_orchestrator(AmplifierHandle runtime, const char* endpoint,
                                                  const char* session_id, AmplifierHandle* out);
AmplifierResult amplifier_load_grpc_hook(AmplifierHandle runtime, const char* endpoint,
                                          AmplifierHandle* out);
AmplifierResult amplifier_load_grpc_context(AmplifierHandle runtime, const char* endpoint,
                                             AmplifierHandle* out);
AmplifierResult amplifier_load_grpc_approval(AmplifierHandle runtime, const char* endpoint,
                                              AmplifierHandle* out);
```

Each returns an opaque handle that can be mounted via the Group 3 functions.

#### Group 5: KernelService (for out-of-process module callbacks)

```c
AmplifierResult amplifier_kernel_service_start(AmplifierHandle session, uint16_t port,
                                                char** out_token);
AmplifierResult amplifier_kernel_service_stop(AmplifierHandle session);
```

Starts the gRPC `KernelService` server for a session. Returns an auth token that out-of-process modules use for callback RPCs.

#### Group 6: Capabilities

```c
AmplifierResult amplifier_register_capability(AmplifierHandle session, const char* name,
                                               const char* value_json);
AmplifierResult amplifier_get_capability(AmplifierHandle session, const char* name,
                                          char** out_json);
```

#### Group 7: Memory Management

```c
void amplifier_string_free(char* str);
const char* amplifier_last_error(void);
```

`amplifier_string_free` frees strings allocated by the library. `amplifier_last_error` returns a thread-local error message (like `errno` + `strerror`).

**Key design decisions:**

- **Blocking calls for v1.** Go uses `runtime.LockOSThread()`, C# wraps in `Task.Run`. Callback-based async is deferred to v1.1.
- **JSON as universal bridge** for all complex types. Every language has JSON support; no codegen required.
- **Thread-local errors** (like `errno` + `strerror`). Non-zero return code means check `amplifier_last_error()`.
- **cbindgen** for header generation. The `amplifier.h` header is generated from Rust source.
- **Deferred:** Callback-based async (v1.1), `register_contributor`/`register_cleanup` (closures can't cross FFI), WASM loading.

**Example: Go host session**

```go
runtime := C.amplifier_runtime_create()
defer C.amplifier_runtime_destroy(runtime)

session := C.amplifier_session_create(runtime, configJSON)
defer C.amplifier_session_destroy(session)

provider := C.amplifier_load_grpc_provider(runtime, "127.0.0.1:50051")
C.amplifier_session_mount_provider(session, provider, "anthropic")

token := C.amplifier_kernel_service_start(session, 0)
C.amplifier_session_initialize(session)
response := C.amplifier_session_execute(session, "Hello, world!")
```

## Data Flow

### Proto-to-Pydantic (Complete RPC)

```
Python proto request
    │
    ▼ SerializeToString()
proto bytes
    │
    ▼ PyO3 boundary
Rust: decode proto → conversions.rs → serde_json::to_string
    │
    ▼ PyO3 boundary
JSON string
    │
    ▼ ChatRequest.model_validate_json()
Pydantic ChatRequest → provider.complete() → Pydantic ChatResponse
    │
    ▼ model_dump_json()
JSON string
    │
    ▼ PyO3 boundary
Rust: serde_json::from_str → conversions.rs → proto.encode_to_vec
    │
    ▼ PyO3 boundary
proto bytes
    │
    ▼ ChatResponse.FromString()
Python proto response (with both content and content_blocks populated)
```

### C ABI (Go host session)

```
Go caller
    │
    ▼ CGo call (blocks Go thread)
amplifier_session_execute(session, prompt, &out_json)
    │
    ▼ Rust: block_on(tokio runtime)
kernel.execute(prompt)
    │  ├── orchestrator loop
    │  ├── provider.complete()  ← may call gRPC to remote provider
    │  └── tool calls           ← may call gRPC to remote tools
    │
    ▼ serde_json::to_string(&response)
JSON response string (caller-owned, free with amplifier_string_free)
    │
    ▼ CGo return
Go: C.GoString(out_json)
```

### Out-of-Process Module Callbacks

```
Python provider (out-of-process)
    │
    ▼ coordinator.hooks.emit("event", data)
SimpleNamespace shim → KernelClient.emit_hook()
    │
    ▼ gRPC call
KernelService.EmitHook RPC → Rust kernel → hook dispatch
    │
    ▼ gRPC response
Result returned to provider
```

## Error Handling

### PyO3 Bridge Errors

- Proto decode failures (`ChatRequest::decode`) → `PyErr` with descriptive message
- JSON serialization failures → `PyErr` with serde error details
- Pydantic validation failures (`model_validate_json`) → standard Pydantic `ValidationError` on the Python side

### C ABI Errors

- Every function returns `AmplifierResult` (0 = success, non-zero = error code)
- Detailed error message available via `amplifier_last_error()` (thread-local)
- NULL handle checks on all functions that accept handles
- Double-free protection on `amplifier_string_free`

### SimpleNamespace Shim Errors

- `mount()` → no-op with debug log (kernel registers the bridge automatically)
- `register_contributor()` → no-op with debug log
- `register_capability(name, callable)` → `TypeError` if value is callable (functions can't be JSON-serialized over gRPC)
- `register_cleanup()` → no-op with debug log ("use Cleanup RPC instead")

## Implementation Order

These changes span two repos but are coordinated into a single release cycle.

### Phase 1: amplifier-core release (all Rust + proto changes)

1. Proto evolution: add `content_blocks = 7` to `ChatResponse`
2. Update `conversions.rs` for dual-write (both `content` and `content_blocks`)
3. Add PyO3 proto conversion functions (`proto_chat_request_to_json`, `json_to_proto_chat_response`)
4. Fix `emit_and_collect()` return type in `hooks.rs` (JSON strings → dicts)
5. Thin `hooks.py` to 3-line alias
6. Fix `.pyi` stub divergence (16 constants → 8)
7. Scaffold `crates/amplifier-ffi/` with 25 functions
8. Regenerate proto stubs (Rust + Python)
9. Bump version, E2E smoke test, tag, publish

### Phase 2: amplifier-foundation update (depends on Phase 1)

1. Update `services.py` to use PyO3 bridge for `Complete` / `ParseToolCalls`
2. Add `CompleteStreaming` (simulated)
3. Add `KernelClient` + `SimpleNamespace` shim for coordinator access
4. Update integration tests
5. PR + merge

## Testing Strategy

- **Round-trip test:** proto bytes → Rust → JSON → Pydantic → JSON → Rust → proto bytes. Verify all 7 `ContentBlock` types survive the round trip.
- **Import-time assertion:** proto `ContentBlock` oneof types == Pydantic `ContentBlockUnion` types. Catches drift at startup.
- **E2E adapter test:** Spawn adapter, call `Complete` with typed content blocks, verify `content_blocks` field 7 is populated in response.
- **C ABI smoke test:** Simple Go test program that creates a session, loads a mock gRPC tool, and executes a prompt.
- **Hooks thinning:** Verify all ecosystem modules (`hooks-backup`, `hooks-approval`, all orchestrators) mount successfully with the 3-line alias.
- **Simulated streaming:** Call `CompleteStreaming`, verify single-element stream with valid `ChatResponse`.
- **Shim coverage:** Mount a real provider (e.g., anthropic) through the adapter, verify `coordinator.mount()` no-ops cleanly and `hooks.emit()` routes to `KernelService`.

## Open Questions

1. **`content_blocks` on base `ChatResponse`:** Provider subclasses already use this name for streaming dataclass types. Should we unify `TextBlock`/`TextContent` type hierarchies long-term?
2. **Hook registration for out-of-process hooks:** Use existing kernel-pulls-subscriptions pattern (`HookService.GetSubscriptions`) for now. Future `RegisterHookSubscription` RPC for push model.
3. **C ABI callback async (v1.1):** How should C# consumers handle async? Callback-based or blocking + `Task.Run`?
4. **`resolve_module()` type accuracy:** Rust hardcodes `ModuleType::Tool` for Python modules. Worth fixing, but lower priority than other changes.
