# Amplifier Core

Ultra-thin Rust kernel for the Amplifier modular AI agent system, with Python bindings via PyO3 and additional bindings for Node.js and .NET.

## Tech Stack
- **Kernel**: Rust (workspace with multiple crates)
- **Python bindings**: PyO3 + Maturin
- **Node.js bindings**: Neon
- **.NET bindings**: FFI via `Amplifier.FFI.Runtime`
- **Build**: Cargo (Rust), uv/pip (Python)
- **Testing**: Python (pytest), Rust (cargo test)
- **Proto**: Protocol Buffers for contracts

## Build & Test
```bash
# Rust
cargo build
cargo test

# Python bindings
pip install -e ".[dev]"
# or with uv:
uv pip install -e ".[dev]"

# Run Python tests
python -m pytest tests/ -v
```

## Project Structure
- `crates/amplifier-core/` — Core Rust kernel crate
- `crates/amplifier-guest/` — Guest/plugin Rust crate
- `bindings/python/` — PyO3 Python bindings
- `bindings/node/` — Node.js bindings
- `bindings/dotnet/` — .NET FFI bindings
- `tests/` — Python integration tests
- `proto/` — Protocol Buffer definitions
- `wit/` — WebAssembly Interface Types
- `scripts/` — Build and maintenance scripts
- `ai_context/` — AI agent context documents
- `agents/` — Agent definitions
- `behaviors/` — Behavior definitions

## Conventions
- Kernel follows the Linux kernel model: tiny stable center, all policies as replaceable modules
- Python API is zero-change compatible — same imports, same API, same behavior as pure-Python predecessor
- Release profile uses LTO, single codegen unit, and symbol stripping
- `CONTRACTS.md` defines the stable API contracts

## Important Notes
- Existing Python code requires zero changes when using Rust bindings
- The `pyproject.toml` uses Maturin as the build backend
- See `CONTRACTS.md` for the stable API that must not be broken
