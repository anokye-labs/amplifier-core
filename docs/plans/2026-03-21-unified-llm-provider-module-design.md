# Unified-LLM Provider Module Design

## Goal

Integrate `unified-llm-client-rust` (a Rust crate with 4 LLM providers, real streaming, 925 tests) into the Amplifier ecosystem as a provider module callable from the current Python-based environment.

## Background

Today, all provider modules (OpenAI, Anthropic, Gemini, etc.) are pure Python packages wrapping HTTP clients. The `unified-llm-client-rust` crate already provides a unified, well-tested Rust client for multiple LLM providers. Exposing it as a provider module would give us:

- A single crate covering 4 providers (OpenAI, Anthropic, Gemini, Azure OpenAI)
- Real streaming support with proper backpressure
- 925 existing tests with strong coverage
- Rust-level performance for type conversion and request handling

The challenge is that the current host (amplifier-app-cli) and all orchestrators are Python. The Rust provider must cross a PyO3 bridge to be usable.

Four independent experts (core-expert, zen-architect, amplifier-expert, foundation-expert) unanimously agreed on a narrowed scope: **Provider-only, Direction 1 (Rust callable from Python), ~300-400 lines.** The full bidirectional bridge for all 7 module traits was rejected as over-building by ~5x.

## Approach

**Build a single Rust provider module with a PyO3 bridge, not a general-purpose module framework.**

The module implements the 5-method Python Provider Protocol directly as a `#[pyclass]`, wrapping `unified-llm-client-rust` for the actual LLM communication. A thin Python shim package provides the `mount()` entry point for module discovery.

This is the minimum viable integration: one trait (Provider), one direction (Rust→Python), `complete()` only (non-streaming). It validates the full type mapping and PyO3 bridge before we expand scope.

Key design principles:
- **YAGNI over generality** — Don't build Direction 2 (Python→Rust) or other 5 trait wrappers until demand pulls
- **Prove the type boundary first** — `conversions.rs` is where production bugs will live; test it exhaustively before widening
- **Zero changes to existing code** — Foundation, app-cli, and orchestrators remain untouched

## Architecture

### Integration Axes

The Amplifier module bridge system has three integration axes. This design targets Axis 2, Direction 1 only.

**Axis 1: Native (Rust↔Rust) — Zero overhead, works today.** A Rust crate implements traits and mounts directly via `coordinator.mount_provider("name", Arc::new(provider))`. In-process, no serialization. Bypasses `transport.rs` entirely. Fully functional for all 7 traits.

**Axis 2: PyO3 Bridge (Rust↔Python) — Bidirectional, partially built.** Rust→Python (Direction 1): `#[pyclass]` wrappers exist for 6 types behind `#[cfg(feature = "wasm")]`. Python→Rust (Direction 2): 3 full bridges + 1 stub exist. **This design addresses Direction 1 for Provider only.**

**Axis 3: Transport Bridges (Any↔Any) — Complete.** gRPC and WASM bridges for all 6 types already exist.

### Call Path

```
Python Orchestrator
    ↓ (Python method call)
PyUnifiedLlmProvider (#[pyclass])
    ↓ (Arc<UnifiedLlmProviderCore>)
UnifiedLlmProviderCore (pure Rust)
    ↓ (unified-llm Client + ProviderAdapter)
HTTP → LLM API
    ↓ (response)
UnifiedLlmProviderCore
    ↓ (conversions.rs: unified-llm types → amplifier JSON)
PyUnifiedLlmProvider
    ↓ (Python dict)
Python Orchestrator
```

This is the exact pattern `PyWasmProvider` already uses. `PyCoordinator` stores providers as opaque `Py<PyAny>` — it doesn't care if the underlying implementation is Python or Rust.

### Why Not Pure Rust-to-Rust

Orchestrators are Python today. The path to eliminate the PyO3 hop is a Rust orchestrator (Phase 2+). Architecture Decision #3 (CONTEXT_TRANSFER.md) already covers this: "Python-from-Rust-host pattern: spawn a gRPC adapter and connect via existing gRPC bridges." Direction 2 may never need PyO3.

## Components

### Module Structure

```
amplifier-module-provider-unified/
├── Cargo.toml                    # depends on unified-llm-client-rust + amplifier-core
├── pyproject.toml                # Python package via maturin, multiple entry points
├── src/
│   ├── lib.rs                    # PyO3 module + #[pyclass] PyUnifiedLlmProvider
│   ├── core.rs                   # UnifiedLlmProviderCore (pure Rust, no PyO3, testable)
│   └── conversions.rs            # ChatRequest ↔ unified-llm types (direct struct mapping)
└── python/
    └── amplifier_module_provider_unified/
        └── __init__.py           # mount_openai(), mount_anthropic(), mount_gemini()
```

### `core.rs` — UnifiedLlmProviderCore

Pure Rust struct. No PyO3 dependency. Fully unit-testable in isolation.

Holds:
- `unified_llm::Client` — the unified-llm HTTP client
- `unified_llm::ProviderAdapter` — provider-specific configuration
- Provider metadata (name, model catalog)

Implements the core logic:
- `complete(request) → Result<Response>` — sends a ChatRequest through unified-llm and returns the response
- `list_models() → Vec<ModelInfo>` — returns available models from the provider catalog
- `get_info() → ProviderInfo` — returns provider metadata

### `lib.rs` — PyUnifiedLlmProvider

`#[pyclass]` wrapper holding `Arc<UnifiedLlmProviderCore>`. The `Arc` is required for the `'static` lifetime across async boundaries in `pyo3_async_runtimes::tokio::future_into_py`.

**This IS the Provider** — there is no separate Rust `Provider` trait to implement. The Python Provider is a Protocol (structural typing). The `#[pyclass]` directly exposes the 5 methods Python expects:

1. `name` — property returning provider name string
2. `get_info()` — returns `ProviderInfo` as dict
3. `list_models()` — returns list of model info dicts
4. `complete(request, **kwargs)` — the main completion method
5. `parse_tool_calls(response)` — extracts tool calls from a response

### `conversions.rs` — Type Mapping

Two different conversion strategies at two different boundaries:

**Python↔Rust boundary (JSON round-trip):**
- Inbound: Pydantic `.model_dump_json()` → `serde_json::from_str` → Rust struct
- Outbound: Rust struct → `serde_json::to_string` → Python `json.loads()` → dict

**Within Rust (direct struct-to-struct mapping):**
- amplifier-core ChatRequest fields → unified-llm Request fields
- unified-llm Response fields → amplifier-core ChatResponse fields
- No JSON involved — direct field-to-field mapping in `conversions.rs`

**Critical conversion risks** (core-expert flagged — test these exhaustively):
- `ThinkingData.signature` — must be preserved exactly across round-trips (multi-turn thinking depends on it)
- `ContentPart::Extension` / `ContentPart::Unknown` — must pass through, never silently dropped
- System message injection — amplifier top-level `system` field → unified-llm system `Message`
- Error variant classification — how `unified_llm::Error` kinds surface to Python
- `finish_reason` exact mapping (`stop` vs `length` vs `tool_calls`)
- `Usage` fields — preserve without recomputation
- Mixed content responses — text + tool calls in same turn
- Tool calls with malformed JSON arguments — must not panic across PyO3 boundary

### `python/__init__.py` — Mount Entry Points

Thin Python shim (~20 lines per entry point). Three separate mount functions, registered as separate entry points in `pyproject.toml`:

- `mount_openai(coordinator, config)` — creates `PyUnifiedLlmProvider` configured for OpenAI
- `mount_anthropic(coordinator, config)` — creates `PyUnifiedLlmProvider` configured for Anthropic
- `mount_gemini(coordinator, config)` — creates `PyUnifiedLlmProvider` configured for Gemini

Each calls `coordinator.mount("providers", provider)` with the configured `PyUnifiedLlmProvider` instance. Single-provider per `mount()` call — multiple entry points in `pyproject.toml` allow bundle configs to reference `unified-openai`, `unified-anthropic`, `unified-gemini` independently.

### Future: module_wrappers.rs (Deferred)

When demand pulls additional trait wrappers, `bindings/python/src/module_wrappers.rs` will contain shared free functions for the wrapper pattern (JSON round-trip, async conversion, error mapping). Both the existing `PyWasm*` types and new `Rust*` types would delegate to these functions. This avoids code duplication while keeping distinct Python-visible class names.

For now, the unified-llm provider module is self-contained and doesn't need this infrastructure.

## Data Flow

### Complete Request Flow

1. Python orchestrator calls `provider.complete(chat_request, **kwargs)`
2. PyO3 receives `chat_request` as `Py<PyAny>`, calls `.model_dump_json()` to get JSON string
3. `serde_json::from_str` deserializes into amplifier-core `ChatRequest` Rust struct
4. `conversions.rs` maps `ChatRequest` → unified-llm `Request` (direct struct mapping)
5. `UnifiedLlmProviderCore` sends request via unified-llm `Client`
6. unified-llm handles HTTP, retries, response parsing
7. `conversions.rs` maps unified-llm `Response` → amplifier-core `ChatResponse` fields
8. `serde_json::to_string` serializes response to JSON
9. PyO3 returns JSON string to Python, orchestrator receives dict
10. Orchestrator uses dict-key access patterns (`result["content"]`, `result["tool_calls"]`)

### Mount Flow

1. Bundle config references `unified-anthropic` as a provider module
2. Module resolver finds the `amplifier_module_provider_unified` Python package
3. Package entry point calls `mount_anthropic(coordinator, config)`
4. `mount_anthropic()` reads API key from config/environment
5. Creates `PyUnifiedLlmProvider` with Anthropic adapter configuration
6. Calls `coordinator.mount("providers", provider)` — coordinator stores as opaque `Py<PyAny>`
7. Orchestrator later retrieves provider, calls methods — doesn't know it's Rust-backed

## Error Handling

- **Missing API key at mount time:** Return `None` / log warning, don't crash. Mount must degrade gracefully.
- **unified-llm HTTP errors:** Map `unified_llm::Error` variants to appropriate Python exceptions. Rate limit errors, auth errors, and server errors should be distinguishable.
- **Malformed tool call arguments:** Never panic across the PyO3 boundary. Return the malformed JSON as-is and let the orchestrator decide.
- **Serialization failures:** If `model_dump_json()` or `serde_json` fails, raise a clear Python exception with the field that failed.
- **Unknown content types:** Pass through as-is. Never silently drop `ContentPart::Extension` or `ContentPart::Unknown` variants.
- **`cleanup()` callback:** Must be invoked on unmount, even if the Rust side has nothing to clean up.

## Testing Strategy

### Layer 1: Rust Unit Tests

Pure Rust tests in `src/`, using `unified_llm::testing::MockProviderAdapter`. No PyO3 involved.

Coverage:
- Type conversion fidelity: every field in ChatRequest↔Request and Response↔ChatResponse round-tripped
- `ThinkingData.signature` preservation (critical for multi-turn thinking)
- Unknown `ContentPart` variants pass through (not dropped)
- Error variant classification: how `unified_llm::Error` kinds surface
- `finish_reason` exact mapping (`stop` vs `length` vs `tool_calls`)
- `Usage` fields preserved without recomputation
- Mixed content responses (text + tool calls in same turn)
- Tool calls with malformed JSON arguments don't panic
- Empty `list_models()` catalog doesn't panic

### Layer 2: Python Integration Tests

Against actual PyO3 `#[pyclass]` — `maturin develop` is a test prerequisite.

```
tests/
├── test_conversions.py           # Round-trip type mapping
├── test_complete.py              # Mock unified-llm client, test complete()
├── test_thinking_signature.py    # Critical: ThinkingData.signature preservation
├── test_tool_calls.py            # Tool call parsing edge cases
└── test_mount.py                 # coordinator.mount() integration
```

Uses:
- `MockCoordinator` from `amplifier_core.testing` for mount tests
- `ProviderBehaviorTests` from `amplifier_core.validation.behavioral` — the standard contract harness all provider modules must pass

Coverage:
- Mount graceful degradation (missing API key → None, not crash)
- `cleanup()` callback invoked on unmount
- `parse_tool_calls()` on non-tool response returns `[]`
- Multi-tool responses preserved in order
- Same provider mounted twice under different names (no global singleton state)
- Provider-specific config passthrough (`reasoning_effort`, `temperature`)
- `get_info()` returns populated `ProviderInfo` (not zero-value struct)

### Layer 3: E2E Smoke Test

- Keep existing amplifier-core smoke test as-is
- Add one targeted test: `amplifier run` with a bundle config mounting `unified-anthropic` instead of the Python `provider-anthropic`. Confirms the Rust-backed PyO3 bridge mounts and completes a real call end-to-end.
- Full provider matrix (OpenAI, Gemini) as optional `pytest -m live` tests, not mandatory CI gates

### Success Criteria

- All 5 Provider Protocol methods work from Python
- `ProviderBehaviorTests` conformance harness passes
- Existing orchestrators (loop-basic, loop-streaming) work unchanged with the new providers
- E2E smoke test passes with `unified-anthropic` (same script used for core releases)
- Zero changes needed in foundation, app-cli, or any orchestrator module

## Phased Delivery

### Phase 1 (NOW): ~300-400 lines

- `UnifiedLlmProviderCore` (pure Rust, wrapping unified-llm client)
- `PyUnifiedLlmProvider` (`#[pyclass]` exposing 5 Provider Protocol methods)
- `conversions.rs` (ChatRequest ↔ unified-llm types)
- Python shim package with `mount_openai()`, `mount_anthropic()`, `mount_gemini()`
- `complete()` only — non-streaming
- Full test suite (Layers 1-3)

### Phase 2: Streaming

- Add `stream()` method to `PyUnifiedLlmProvider`
- The streaming orchestrator already checks `hasattr(provider, "stream")` — it will just start working
- Design the Python streaming protocol with the working bridge as a lab

### Phase 3: When Demand Pulls

- Other trait wrappers (`RustTool`, `RustOrchestrator`, etc.) via `module_wrappers.rs`
- Direction 2 bridges (Python→Rust) — only needed for a Rust orchestrator that doesn't exist yet
- Ergonomics: `Transport::Native` in resolver, `amplifier.toml` support, `load_native_*()` functions

## Open Questions

- **Return type migration:** Currently returning dicts to match existing orchestrator expectations (`result["success"]`). Switching to Pydantic instances would silently break those call sites. Requires a separate PR with a full call-site audit. When should this be prioritized?
- **Azure OpenAI as a fourth entry point:** unified-llm supports Azure OpenAI. Add `mount_azure_openai()` or defer until someone needs it?
- **Streaming protocol design:** Phase 2 needs to decide how Rust async streams map to Python async iterators across PyO3. Worth investigating during Phase 1 to de-risk?
