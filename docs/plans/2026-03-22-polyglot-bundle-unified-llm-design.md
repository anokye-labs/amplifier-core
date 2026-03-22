# Polyglot Bundle and Unified-LLM Integration Design

> Polyglot module loading architecture for amplifier-core — any module in any language loadable by any host — with `amplifier-bundle-unified-llm` as the first proving ground.

**Status:** Approved
**Date:** 2026-03-22
**Prerequisites:** amplifier-core v1.3.2 (gRPC bridges complete, WASM loading complete, module resolver complete)
**Prior art:** `2026-03-21-unified-llm-provider-module-design.md` (PyO3 approach — superseded by this design)

---

## 1. Goal

Ship polyglot module loading in amplifier-core so that modules written in any language (Rust, Python, WASM, or any gRPC-speaking language) can be loaded by any host. The `amplifier-bundle-unified-llm` bundle — a pure Rust provider wrapping `unified-llm-client-rust` — is the first consumer of this architecture.

---

## 2. Background

Today all provider modules in the Python host are pure Python packages. The `unified-llm-client-rust` crate already provides a unified, well-tested Rust client for 4 LLM providers (OpenAI, Anthropic, Gemini, Azure OpenAI) with 925 tests and real streaming support. Making it available as an Amplifier provider module requires answering the foundational question: **how does a non-Rust host load a Rust module?**

Multiple experts (core-expert, zen-architect, foundation-expert, amplifier-expert) were consulted. Industry research covered 10 polyglot plugin systems (go-plugin, Terraform, VS Code, Envoy WASM, Kubernetes Operators, Dapr, OTel Collector, Backstage, WASI Component Model, Extism).

### Key Finding

No system does true runtime polyglot discovery. They all normalize the boundary through:
- A **protocol** (gRPC, WASM ABI, Kubernetes API)
- A **manifest** (amplifier.toml, package.json, terraform-registry-manifest.json)
- Or **compile-time wiring** (OTel builder)

The gRPC sidecar pattern (go-plugin, Terraform, Dapr) is the most proven pattern for cross-language plugin loading. Amplifier already has complete gRPC bridges for all 6 module types — making this the natural choice.

### Expert Consensus

All experts independently reached the same conclusion: **there is no "dynamically load a Rust crate at runtime." That's not how Rust works.** The two paths are gRPC sidecar (spawn binary, connect via existing bridges) or WASM (compile to .wasm, load in-process). Both are already built in core.

---

## 3. Approach

Three coordinated pieces ship together:

1. **Core — `Transport::Rust` and polyglot loading** (~150 lines of new code)
2. **Foundation — Polyglot awareness** (~50 lines)
3. **`amplifier-bundle-unified-llm`** — first polyglot bundle (new repo)

The loading mechanism for non-Rust hosts is the **gRPC sidecar pattern**: spawn the Rust module as a gRPC server process and connect via existing gRPC bridges. This is the symmetric mirror of Architecture Decision #3 (Python-from-Rust-host: spawn gRPC adapter).

| Host | Encounters `transport = "rust"` | What it does |
|------|--------------------------------|-------------|
| **Rust host** | Links crate directly at compile time | `Arc::new(provider)` — zero overhead |
| **Python host** | Can't link Rust | Spawns module binary as gRPC server, connects via `load_grpc_provider(endpoint)` |
| **Go/C#/any host** | Can't link Rust | Same gRPC sidecar pattern |

The module author writes `transport = "rust"` once and never changes it. The host decides how to consume.

**Sequencing:** Core PR first (version bump to v1.3.3 + E2E smoke test) → Foundation PR second → Bundle repo third.

---

## 4. Architecture

### 4.1 The `amplifier.toml` Spec

The module-level manifest declares what the module IS and how to load it. Each module self-describes via an `amplifier.toml` in its own directory.

**Format for unified-llm:**

```toml
[module]
type = "provider"
transport = "rust"
crate = "amplifier_module_provider_unified"
```

**Existing format works forever:**

```toml
[module]
transport = "wasm"
type = "tool"
```

**Transport vocabulary:**

| Transport | Meaning | Transport-specific fields |
|-----------|---------|--------------------------|
| `"python"` | Python package | `package` (pip package name) |
| `"rust"` | Rust crate | `crate` (crate name — for Rust host to link, for non-Rust host to identify binary) |
| `"wasm"` | WebAssembly component | `artifact` + optional `sha256` |
| `"grpc"` | Remote gRPC service | Endpoint is runtime config (declared at mount time, not in manifest) |

**Rules:**
- `transport` field is required — one of: `"python"`, `"rust"`, `"wasm"`, `"grpc"`
- `Transport::Native` removed from the enum (never used by any existing module)
- `Transport::Rust` added
- Module declares what it IS (`transport`). Host decides how to CONSUME it.

**What `transport = "rust"` means depends on the host:**
- Rust host → links crate directly (`Arc::new(provider)`, zero overhead)
- Python host → spawns module binary as gRPC server process, connects via existing `load_grpc_provider(endpoint)` bridges
- Go/C#/any host → same gRPC sidecar pattern

**Future:** When a module also compiles to WASM, it can change to `transport = "wasm"` for sandboxed in-process loading on any host. WASM has networking constraints that make it unsuitable for HTTP-calling providers today.

### 4.2 Cross-Language Loading: gRPC Sidecar Pattern

When a non-Rust host encounters `transport = "rust"`, the loading sequence is:

```
Python host
    │
    ├── resolve_module(path) → Transport::Rust, RustCrate { crate_name }
    │
    ├── Look for pre-built binary in module directory
    │
    ├── Spawn binary as subprocess
    │   └── Binary serves gRPC on random port
    │   └── Prints READY:<port> to stdout
    │
    ├── Connect via existing load_grpc_provider(endpoint)
    │
    └── Result: indistinguishable from any other gRPC-bridged module
```

**What already exists and doesn't need building:**
- 6 complete gRPC bridges (Tool, Provider, Orchestrator, ContextManager, HookHandler, ApprovalProvider)
- Proto definitions for all module services
- `load_grpc_*()` functions in transport.rs
- KernelService gRPC server (1900+ lines)

**What needs building:**
- ~50 lines: Python host "spawn binary + connect" launcher
- ~100 lines: Rust gRPC server scaffold template for module authors
- Transport enum + module resolver changes

### 4.3 Architecture Decision #18

**Rust-from-non-Rust-host pattern:** When a non-Rust host encounters a module with `transport = "rust"`, it spawns the module's pre-built binary (which exposes gRPC services) and connects via existing gRPC bridges. This mirrors Architecture Decision #3 (Python-from-Rust-host: spawn gRPC adapter). gRPC is the universal cross-language bridge.

---

## 5. Components

### 5.1 Core Changes — `Transport::Rust` and Module Resolver

**`transport.rs`:**
- Remove `Transport::Native` variant entirely (never used)
- Add `Transport::Rust` variant
- `from_str("rust")` → `Transport::Rust`

**`module_resolver.rs`:**
- `parse_amplifier_toml()`: read `crate` field when `transport = "rust"`
- New variant: `ModuleArtifact::RustCrate { crate_name: String }` — the crate name for a Rust host to link, or for a non-Rust host to know which binary to spawn
- `load_module()`: `RustCrate` → `LoadedModule::RustDelegated { crate_name }` — signal to the host that this module needs external handling

**`bindings/python/src/module_wrappers.rs`:**
- NOT needed for this approach. The gRPC bridge handles Rust↔Python communication — no PyO3 wrapper required.

### 5.2 Python Host Dispatch

**`loader.py`:**
- When `resolve_module()` returns `Transport::Rust` / `RustDelegated`:
  1. Look for pre-built binary in the module directory
  2. Spawn it as a subprocess (serves gRPC on a random port, prints `READY:<port>` to stdout)
  3. Connect via existing `load_grpc_provider(endpoint)`
  4. Result is indistinguishable from any other gRPC-bridged module

~50 lines of new code in the Python host.

### 5.3 Rust Module gRPC Scaffold

A thin `main.rs` template that wraps any `impl Provider` in a gRPC server:
- Uses existing proto contracts and KernelService patterns
- Module author writes pure Rust; scaffold is platform-provided
- ~100 lines

The scaffold handles:
- Selecting a random available port
- Starting the gRPC server
- Printing `READY:<port>` to stdout
- Graceful shutdown on SIGTERM

### 5.4 Foundation Changes (~50 lines)

**`ModuleActivator`:**
- Reads `amplifier.toml` to determine transport
- Skips `uv pip install` for non-Python modules
- Foundation reads `amplifier.toml` via Python `tomllib` directly

**`GitSourceHandler._verify_clone_integrity()`:**
- Accepts `amplifier.toml` as a valid clone marker (previously only checked for Python-specific files)

Foundation handles source resolution (transport-agnostic). Core handles transport detection and loading. The boundary is clean: Foundation never interprets transport semantics.

### 5.5 `amplifier-bundle-unified-llm`

The first polyglot bundle — a pure Rust provider module.

**Bundle structure:**

```
amplifier-bundle-unified-llm/
├── bundle.md                          # Root: namespace + context ref
├── providers/
│   ├── anthropic.yaml                 # adapter: anthropic, default model
│   ├── openai.yaml                    # adapter: openai, default model
│   └── gemini.yaml                    # adapter: gemini, default model
├── context/
│   └── provider-awareness.md          # Thin: what this provides (~20 lines)
├── modules/
│   └── provider-unified/
│       ├── amplifier.toml             # transport = "rust", type = "provider"
│       ├── Cargo.toml
│       └── src/                       # Pure Rust (lib.rs, core.rs, conversions.rs)
├── docs/
├── README.md
└── LICENSE
```

**Key design decisions:**
- Module is **pure Rust**. No Python shim, no `__init__.py`, no `pyproject.toml`, no maturin.
- `amplifier.toml` lives inside the module directory (not bundle root). Each module self-describes.
- Bundles can have multiple modules in different languages, each with its own `amplifier.toml`.
- Single `mount()` entry point — `config["adapter"]` selects the backend (like the github-copilot pattern).
- Provider variant YAMLs are the composable units users include in their bundles.
- Lean bundle: no behaviors dir, no agents dir (yet). Context is one thin file. Expand when provider-specific tools arrive.

**User consumption via includes:**

```yaml
includes:
  - bundle: foundation
  - bundle: unified-llm:providers/anthropic
```

---

## 6. Data Flow

### 6.1 Module Loading (Python host, `transport = "rust"`)

```
Bundle Config (YAML)
    │  source_hint: "unified-llm:modules/provider-unified"
    ▼
Foundation: resolve source → filesystem path
    │  (transport-agnostic — reads amplifier.toml, skips pip install)
    ▼
Core: resolve_module(path)
    │  reads amplifier.toml → Transport::Rust, RustCrate { crate_name }
    ▼
Python host: spawn binary subprocess
    │  binary serves gRPC on localhost:<random_port>
    │  prints READY:<port> to stdout
    ▼
Python host: load_grpc_provider("localhost:<port>")
    │  connects via existing Provider gRPC bridge
    ▼
Provider mounted — indistinguishable from Python or WASM provider
```

### 6.2 Module Loading (Rust host, `transport = "rust"`)

```
Bundle Config
    │  transport: "rust", crate: "amplifier_module_provider_unified"
    ▼
Rust host: links crate at compile time
    │  coordinator.mount_provider("unified", Arc::new(provider))
    ▼
Provider mounted — zero overhead, in-process
```

### 6.3 Request Flow (after loading)

```
Orchestrator calls provider.complete(request)
    │
    ├── [Rust host] Direct method call → unified-llm-client-rust → HTTP → LLM API
    │
    └── [Python host] gRPC call → gRPC server in subprocess
                          │
                          └── unified-llm-client-rust → HTTP → LLM API
                          └── response → gRPC → Python host
```

---

## 7. Error Handling

### Binary Not Found
If the pre-built binary is not present in the module directory, the Python host raises a clear error:
`"Module 'provider-unified' declares transport='rust' but no pre-built binary found at {expected_path}. Build with 'cargo build --release' or download platform binaries."`

### Binary Startup Failure
- Python host reads stderr from the spawned process
- Timeout (default 10s) waiting for `READY:<port>` on stdout → error with captured stderr
- Process exit code checked on failure

### gRPC Connection Failure
After `READY:<port>`, if the gRPC connection fails, existing `load_grpc_provider()` error handling applies — the same path used for all gRPC modules.

### Process Lifecycle
- Subprocess tied to session lifecycle — killed on session teardown
- SIGTERM first, SIGKILL after 5s grace period
- Crash detection: if subprocess exits unexpectedly, provider calls return an error indicating the backend is unavailable

---

## 8. Testing Strategy

### Core Tests
- `Transport::Rust` enum parsing: `from_str("rust")` → `Transport::Rust`
- `parse_amplifier_toml()` reads `crate` field for `transport = "rust"`
- `ModuleArtifact::RustCrate` construction and pattern matching
- `Transport::Native` removal does not break existing tests (verify no tests reference it)

### Python Host Tests
- Binary spawn + `READY:<port>` detection
- gRPC connection after binary startup
- Timeout handling (binary never prints READY)
- Subprocess cleanup on session teardown

### Foundation Tests
- `ModuleActivator` skips pip install for `transport = "rust"` modules
- `_verify_clone_integrity()` accepts `amplifier.toml` as valid marker
- `tomllib` parsing of `amplifier.toml`

### Integration / E2E
- Smoke test: load a `transport = "rust"` module from a bundle config in the Python host
- Verify the loaded provider is callable and returns valid responses
- Release gate: E2E smoke test must pass before v1.3.3 tag

### Bundle Tests
- Existing `unified-llm-client-rust` tests (925 tests) continue to pass
- Provider module integration tests via gRPC scaffold
- Each provider variant (anthropic, openai, gemini) tested individually

---

## 9. Context Documentation (Retcon Style)

Per `CONTEXT_POISONING.md` — no addendums, no changelogs, no "new in v1.3.3." Write as if it always was.

### In `amplifier-core`

1. **`docs/DESIGN_PHILOSOPHY.md`** — Section on polyglot module loading as established architecture:
   - Four transport types: `python`, `rust`, `wasm`, `grpc`
   - gRPC is the universal cross-language bridge
   - Rust host links Rust modules directly; non-Rust host spawns gRPC sidecar
   - WASM for sandboxed/portable scenarios
   - Module declares what it IS (`transport`). Host decides how to CONSUME it.

2. **`MOUNT_PLAN_SPECIFICATION.md`** — Document `transport = "rust"` with `crate` field alongside existing transports. Present as part of the transport vocabulary.

3. **Architecture Decisions in `CONTEXT_TRANSFER.md`** — Decision #18 on polyglot loading: "Rust-from-non-Rust-host: spawn gRPC server, connect via existing bridges. Symmetric with Decision #3."

### In `amplifier-foundation`

4. **Context file for polyglot bundles** — How bundles contain modules in different languages. Each module has its own `amplifier.toml` declaring transport. Foundation handles source resolution (transport-agnostic). Core handles transport detection and loading. Foundation skips `pip install` for non-Python modules.

---

## 10. Phased Delivery

| Phase | Scope | Status |
|-------|-------|--------|
| **Phase 1** (this design) | Core `Transport::Rust` + Foundation awareness + Bundle structure + gRPC scaffold | Active |
| **Phase 2** | Streaming support (`complete_stream()` on Provider, `StreamDelta` types) | Planned |
| **Phase 3** | Additional provider-specific tools within the bundle | Planned |
| **Phase 4** | WASM compilation path as alternative to gRPC for sandboxed/compute-only modules | Planned |

### Phase 1 Delivery Sequence

1. **Core PR** — `Transport::Rust`, module resolver changes, gRPC scaffold template. Version bump to v1.3.3. E2E smoke test. Tag + push.
2. **Foundation PR** — `ModuleActivator` polyglot awareness, `amplifier.toml` as clone marker. ~50 lines.
3. **Bundle repo** — `amplifier-bundle-unified-llm` with pure Rust provider module, provider variant YAMLs, and context.

---

## 11. Open Questions

1. **Pre-built binary distribution:** Should the module repo include CI that builds binaries for all platforms? Or should the host build from source (requires Rust toolchain on the host machine)?

2. **gRPC scaffold location:** Should the scaffold live in amplifier-core as a reusable crate (e.g., `amplifier-module-scaffold`), or be copied into each module repo? A shared crate reduces duplication but adds a dependency; a copy is self-contained.

3. **Existing implementation restructuring:** The current `unified-llm` provider module has a 108-test PyO3/maturin implementation. It needs restructuring as pure Rust + gRPC server. How much of the existing `conversions.rs` and `core.rs` is reusable vs. needs rewriting for the gRPC boundary?