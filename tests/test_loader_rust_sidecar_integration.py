"""Integration test — end-to-end Python loader with Rust module.

Validates that loader.load() correctly dispatches to the Rust sidecar path,
finds the binary, spawns it, and reads the READY:<port> handshake.

The fake binary (FAKE_BINARY_SCRIPT) is a Python script that implements the
READY protocol but is not a real gRPC server. The test verifies:
1. The loader dispatches to the Rust sidecar path (not Python path)
2. The binary is found and executable
3. The READY:<port> handshake is successfully read
4. The gRPC connection attempt follows (may fail — expected behavior)
"""

import stat
import sys
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from amplifier_core.loader import ModuleLoader

# A Python script that simulates a Rust sidecar binary using the READY protocol.
# Binds a socket to 127.0.0.1:0 to get an ephemeral port, closes it
# (since we're not serving real gRPC), prints READY:<port>, then sleeps.
FAKE_BINARY_SCRIPT = """\
#!/usr/bin/env python3
import socket
import time

s = socket.socket()
s.bind(('127.0.0.1', 0))
port = s.getsockname()[1]
s.close()
print(f'READY:{port}', flush=True)
time.sleep(5)
"""


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


@pytest.mark.asyncio
async def test_rust_sidecar_ready_protocol(mock_coordinator):
    """loader.load() with Rust transport dispatches, finds binary, spawns it,
    and reads the READY:<port> handshake.

    The fake binary isn't a real gRPC server so the gRPC connection attempt
    after READY will fail — the test validates the dispatch + spawn + READY
    flow, not the full gRPC session.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        # Create amplifier.toml with rust transport config
        toml_path = tmpdir_path / "amplifier.toml"
        toml_path.write_text(
            '[module]\n'
            'type = "provider"\n'
            'transport = "rust"\n'
            'crate = "fake_provider"\n'
        )

        # Write fake binary script that implements the READY protocol
        binary_path = tmpdir_path / "fake_provider"
        binary_path.write_text(FAKE_BINARY_SCRIPT)
        # Make it executable (owner + group + other execute bits)
        binary_path.chmod(
            binary_path.stat().st_mode
            | stat.S_IEXEC
            | stat.S_IXGRP
            | stat.S_IXOTH
        )

        # Mock source resolution: async_resolve returns a source whose
        # resolve() returns the temp dir path
        fake_source = MagicMock()
        fake_source.resolve.return_value = tmpdir_path

        mock_resolver = MagicMock()
        mock_resolver.async_resolve = AsyncMock(return_value=fake_source)
        mock_coordinator.get.return_value = mock_resolver

        # Mock the Rust engine: resolve_module returns a manifest declaring
        # rust transport with crate_name matching the fake binary filename
        fake_engine = MagicMock()
        fake_engine.resolve_module = MagicMock(
            return_value={
                "transport": "rust",
                "module_type": "provider",
                "artifact_type": "rust",
                "crate_name": "fake_provider",
            }
        )

        # Create loader and call load() — should dispatch to Rust sidecar path
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            mount_fn = await loader.load(
                "fake-provider",
                {},
                source_hint="/fake/path",
                coordinator=mock_coordinator,
            )

        # Primary assertion: loader returned a callable mount function
        assert callable(mount_fn), (
            f"Expected loader.load() to return a callable, got {type(mount_fn)}"
        )

        # Secondary assertion: calling mount_fn exercises the spawn + READY flow.
        # The fake binary prints READY:<port> but is not a real gRPC server, so
        # the gRPC connection step will fail. We verify the error is gRPC-related,
        # NOT a binary-not-found or READY-timeout error (which would mean the
        # dispatch or READY protocol failed).
        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            with pytest.raises(Exception) as exc_info:
                await mount_fn(mock_coordinator)

        error_msg = str(exc_info.value).lower()

        # Must NOT be a binary-not-found error (that means dispatch failed)
        assert "no rust sidecar binary found" not in error_msg, (
            f"Binary was not found — dispatch failed: {exc_info.value}"
        )

        # Must NOT be a READY timeout error (that means the READY protocol failed)
        assert "did not send ready" not in error_msg, (
            f"READY protocol failed — sidecar spawn or output read failed: {exc_info.value}"
        )

        # The error should be gRPC-related, confirming the loader got past
        # the READY handshake and attempted the gRPC connection step.
        grpc_keywords = ("grpc", "connect", "channel", "rpc", "proto", "unavailable")
        assert any(kw in error_msg for kw in grpc_keywords), (
            f"Expected gRPC-related error after READY, got: {exc_info.value!r}\n"
            f"Error type: {type(exc_info.value).__name__}"
        )
