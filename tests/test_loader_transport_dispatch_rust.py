"""Tests for Rust transport dispatch through ModuleLoader.load().

Verifies that loader.load() routes to the sidecar/binary loader path when
the Rust engine resolves a module with transport = "rust".

This test is intentionally RED: the loader currently falls through to the
Python entry-point path for unknown transports (including "rust").
The test documents the expected behavior — an exception whose message
signals a Rust/sidecar transport error — so that implementing the
Rust transport branch turns this test GREEN.
"""

import sys
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from amplifier_core.loader import ModuleLoader

MODULE_ID = "provider-unified"


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------


@pytest.fixture
def mock_coordinator():
    """MagicMock coordinator with real mount_points structure."""
    coord = MagicMock()
    coord.mount_points = {
        "orchestrator": None,
        "providers": {},
        "tools": {},
        "context": None,
        "hooks": MagicMock(),
        "module-source-resolver": None,
    }
    return coord


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_rust_dispatch_routes_to_sidecar_loader(mock_coordinator):
    """loader.load() with Rust transport dispatches to sidecar/binary loader.

    When the Rust engine resolves a module as 'rust' transport, loader.load()
    should dispatch to the sidecar/binary loading path and raise an error
    whose message contains Rust-related keywords, confirming the loader routed
    to the Rust transport path rather than the Python entry-point path.

    This test is RED: the loader currently has no 'rust' transport branch, so
    it falls through to Python validation which produces an error about a
    missing Python package — not the expected Rust/sidecar error.
    """
    # -- Create temp module dir with amplifier.toml --------------------------
    with tempfile.TemporaryDirectory() as tmpdir:
        toml_path = Path(tmpdir) / "amplifier.toml"
        toml_path.write_text(
            "[module]\n"
            "name = 'provider-unified'\n"
            "type = 'provider'\n"
            "transport = 'rust'\n"
            "\n"
            "[rust]\n"
            "crate = 'amplifier_module_provider_unified'\n"
        )

        # -- Mock source resolution ------------------------------------------
        fake_source = MagicMock()
        fake_source.resolve.return_value = Path(tmpdir)

        mock_resolver = MagicMock()
        mock_resolver.async_resolve = AsyncMock(return_value=fake_source)
        mock_coordinator.get.return_value = mock_resolver

        # -- Mock Rust engine ------------------------------------------------
        fake_engine = MagicMock()
        fake_engine.resolve_module.return_value = {
            "transport": "rust",
            "module_type": "provider",
            "artifact_type": "rust",
            "crate_name": "amplifier_module_provider_unified",
        }

        # -- Execute ---------------------------------------------------------
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            with pytest.raises((ImportError, OSError, Exception)) as exc_info:
                await loader.load(
                    MODULE_ID,
                    {},
                    source_hint="/fake/path",
                    coordinator=mock_coordinator,
                )

        # -- Verify ----------------------------------------------------------
        # The error message must contain Rust/sidecar-related keywords,
        # confirming the loader dispatched to the Rust transport path (not
        # the Python path).
        #
        # Currently this assertion FAILS because the loader falls through to
        # Python validation, producing an error about a missing Python package
        # rather than a Rust/sidecar transport error.  Implementing the
        # 'transport == "rust"' branch in loader.load() will make this GREEN.
        error_msg = str(exc_info.value).lower()
        rust_keywords = ("rust", "sidecar", "binary", "transport", "crate")
        assert any(kw in error_msg for kw in rust_keywords), (
            f"Expected Rust/sidecar-related error but got: {exc_info.value!r}"
        )
