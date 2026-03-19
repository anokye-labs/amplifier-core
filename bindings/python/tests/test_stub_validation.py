"""Stub validation tests — verify .pyi stubs match the compiled _engine module.

These tests ensure the type stubs declared in _engine.pyi accurately reflect
the actual exports and signatures of the compiled Rust extension module.
"""

import ast
import pathlib


# The 8 actual #[classattr] entries from bindings/python/src/hooks.rs
EXPECTED_RUST_CLASSATTR_CONSTANTS = [
    "SESSION_START",
    "SESSION_END",
    "PROMPT_SUBMIT",
    "TOOL_PRE",
    "TOOL_POST",
    "CONTEXT_PRE_COMPACT",
    "ORCHESTRATOR_COMPLETE",
    "USER_NOTIFICATION",
]

# Phantom constants that must NOT appear in the stub
PHANTOM_CONSTANTS = [
    "SESSION_ERROR",
    "SESSION_RESUME",
    "SESSION_FORK",
    "TURN_START",
    "TURN_END",
    "TURN_ERROR",
    "PROVIDER_REQUEST",
    "PROVIDER_RESPONSE",
    "PROVIDER_ERROR",
    "TOOL_CALL",
    "TOOL_RESULT",
    "TOOL_ERROR",
    "CANCEL_REQUESTED",
    "CANCEL_COMPLETED",
]


def _get_rust_hook_registry_constants_from_stub() -> list[str]:
    """Parse _engine.pyi and return list of class-level constant names in RustHookRegistry."""
    # tests/ is at bindings/python/tests/; stub is at python/amplifier_core/_engine.pyi
    stub_path = (
        pathlib.Path(__file__).parent.parent.parent.parent
        / "python"
        / "amplifier_core"
        / "_engine.pyi"
    )
    source = stub_path.read_text()
    tree = ast.parse(source)

    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef) and node.name == "RustHookRegistry":
            constants = []
            for item in node.body:
                # Class-level type-annotated attribute: `NAME: str`
                if isinstance(item, ast.AnnAssign) and isinstance(item.target, ast.Name):
                    constants.append(item.target.id)
            return constants

    return []


def test_rust_hook_registry_stub_has_exactly_8_classattr_constants():
    """Stub must list exactly the 8 #[classattr] event constants from hooks.rs."""
    constants = _get_rust_hook_registry_constants_from_stub()
    assert set(constants) == set(EXPECTED_RUST_CLASSATTR_CONSTANTS), (
        f"RustHookRegistry stub constants mismatch.\n"
        f"  Expected: {sorted(EXPECTED_RUST_CLASSATTR_CONSTANTS)}\n"
        f"  Got:      {sorted(constants)}"
    )
    assert len(constants) == 8, (
        f"Expected exactly 8 event constants, got {len(constants)}: {constants}"
    )


def test_rust_hook_registry_stub_no_phantom_constants():
    """Stub must not list any of the 14 phantom constants that don't exist in Rust."""
    constants = _get_rust_hook_registry_constants_from_stub()
    phantom_found = [c for c in constants if c in PHANTOM_CONSTANTS]
    assert phantom_found == [], (
        f"Phantom constants found in RustHookRegistry stub: {phantom_found}\n"
        f"These constants do not exist as #[classattr] in hooks.rs and must be removed."
    )


def test_rust_hook_registry_runtime_classattrs_match_stub():
    """Runtime RustHookRegistry class attributes must match the 8 stub constants."""
    from amplifier_core._engine import RustHookRegistry

    for name in EXPECTED_RUST_CLASSATTR_CONSTANTS:
        assert hasattr(RustHookRegistry, name), (
            f"RustHookRegistry missing classattr '{name}' at runtime"
        )
        value = getattr(RustHookRegistry, name)
        assert isinstance(value, str), (
            f"RustHookRegistry.{name} should be str, got {type(value)}"
        )


def test_engine_exports_match_stubs():
    """Verify the Rust module exports match what the stubs declare."""
    import amplifier_core._engine as engine

    # Module-level attributes
    assert hasattr(engine, "__version__")
    assert hasattr(engine, "RUST_AVAILABLE")

    # All four PyO3 classes
    assert hasattr(engine, "RustSession")
    assert hasattr(engine, "RustHookRegistry")
    assert hasattr(engine, "RustCancellationToken")
    assert hasattr(engine, "RustCoordinator")


def test_rust_session_has_stub_members():
    """Verify RustSession exposes every member declared in the stub."""
    from amplifier_core._engine import RustSession

    # __init__ takes a config dict
    assert callable(RustSession)

    # Minimal valid config for Rust SessionConfig::from_value
    config = {"session": {"orchestrator": "loop-basic", "context": "context-simple"}}
    session = RustSession(config)
    assert hasattr(session, "session_id")
    assert hasattr(session, "parent_id")
    assert hasattr(session, "initialized")

    # Methods declared in stubs
    assert hasattr(session, "initialize")
    assert hasattr(session, "execute")
    assert hasattr(session, "cleanup")
    assert callable(session.initialize)
    assert callable(session.execute)
    assert callable(session.cleanup)


def test_rust_hook_registry_has_stub_members():
    """Verify RustHookRegistry exposes every member declared in the stub."""
    from amplifier_core._engine import RustHookRegistry

    registry = RustHookRegistry()

    assert hasattr(registry, "register")
    assert hasattr(registry, "emit")
    assert hasattr(registry, "unregister")
    assert callable(registry.register)
    assert callable(registry.emit)
    assert callable(registry.unregister)


def test_rust_cancellation_token_has_stub_members():
    """Verify RustCancellationToken exposes every member declared in the stub."""
    from amplifier_core._engine import RustCancellationToken

    token = RustCancellationToken()

    assert hasattr(token, "request_cancellation")
    assert hasattr(token, "is_cancelled")
    assert hasattr(token, "state")
    assert callable(token.request_cancellation)
    # is_cancelled is a property, not a method — verify it returns a bool
    assert isinstance(token.is_cancelled, bool)


def test_rust_coordinator_has_stub_members():
    """Verify RustCoordinator exposes every member declared in the stub."""
    from amplifier_core._engine import RustCoordinator

    class _FakeSession:
        session_id = "test-123"
        parent_id = None
        config = {"session": {"orchestrator": "loop-basic"}}

    coordinator = RustCoordinator(_FakeSession())

    # Properties declared in stubs
    assert hasattr(coordinator, "hooks")
    assert hasattr(coordinator, "cancellation")
    assert hasattr(coordinator, "config")


def test_version_and_flag_values():
    """Verify module-level constants have the expected types and values."""
    import amplifier_core._engine as engine

    assert isinstance(engine.__version__, str)
    assert isinstance(engine.RUST_AVAILABLE, bool)
    assert engine.RUST_AVAILABLE is True
