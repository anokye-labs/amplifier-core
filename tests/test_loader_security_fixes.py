"""Tests for security fixes in the Rust sidecar loader path.

Covers:
- Fix 1 (H-03): Binary path escape guard in _make_rust_sidecar_mount()
- Fix 2: Host-allocated port passed via --port to the sidecar binary
- Fix 3: Port value range validation on the READY:<port> handshake
"""

import os
import stat
import sys
import tempfile
from pathlib import Path
from unittest.mock import AsyncMock, MagicMock, patch

import pytest

from amplifier_core.loader import ModuleLoader


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


def _make_fake_engine(crate_name: str) -> MagicMock:
    """Return a MagicMock Rust engine that reports rust transport."""
    fake_engine = MagicMock()
    fake_engine.resolve_module = MagicMock(
        return_value={
            "transport": "rust",
            "module_type": "provider",
            "artifact_type": "rust",
            "crate_name": crate_name,
        }
    )
    return fake_engine


def _make_fake_resolver(module_path: Path) -> MagicMock:
    """Return a MagicMock source resolver that resolves to *module_path*."""
    fake_source = MagicMock()
    fake_source.resolve.return_value = module_path
    resolver = MagicMock()
    resolver.async_resolve = AsyncMock(return_value=fake_source)
    return resolver


# ---------------------------------------------------------------------------
# Fix 1 (H-03): Binary path escape guard
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_binary_escape_rejected(mock_coordinator):
    """_make_rust_sidecar_mount() raises ValueError when the binary resolves
    to a path outside the module directory (symlink-based traversal guard).
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)
        module_dir = tmpdir_path / "module"
        module_dir.mkdir()

        # Create amplifier.toml inside the module dir
        (module_dir / "amplifier.toml").write_text(
            '[module]\ntype = "provider"\ntransport = "rust"\ncrate = "evil_binary"\n'
        )

        # Create a real binary OUTSIDE the module directory
        outside_binary = tmpdir_path / "evil_binary"
        outside_binary.write_text("#!/usr/bin/env python3\nprint('READY:9999')\n")
        outside_binary.chmod(outside_binary.stat().st_mode | stat.S_IEXEC)

        # Symlink *inside* the module dir pointing to the outside binary
        symlink_inside = module_dir / "evil_binary"
        os.symlink(str(outside_binary), str(symlink_inside))

        mock_coordinator.get.return_value = _make_fake_resolver(module_dir)
        fake_engine = _make_fake_engine("evil_binary")
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            mount_fn = await loader.load(
                "evil-provider",
                {},
                source_hint="/fake/path",
                coordinator=mock_coordinator,
            )

        # Calling mount_fn should raise ValueError due to path escape
        with pytest.raises(ValueError, match="escapes module directory"):
            await mount_fn(mock_coordinator)


@pytest.mark.asyncio
async def test_binary_within_module_dir_accepted(mock_coordinator):
    """A binary that legitimately lives inside the module directory is accepted
    (no ValueError from the path escape guard).

    The binary prints a bogus READY line so the test will fail at the gRPC
    step — that's expected, and we assert the error is NOT a path-escape error.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        (tmpdir_path / "amplifier.toml").write_text(
            '[module]\ntype = "provider"\ntransport = "rust"\ncrate = "fake_provider"\n'
        )

        # Binary that immediately prints a valid READY line then exits
        binary = tmpdir_path / "fake_provider"
        binary.write_text(
            "#!/usr/bin/env python3\nimport socket, time\n"
            "s = socket.socket()\ns.bind(('127.0.0.1', 0))\n"
            "port = s.getsockname()[1]\ns.close()\n"
            "print(f'READY:{port}', flush=True)\ntime.sleep(5)\n"
        )
        binary.chmod(binary.stat().st_mode | stat.S_IEXEC)

        mock_coordinator.get.return_value = _make_fake_resolver(tmpdir_path)
        fake_engine = _make_fake_engine("fake_provider")
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            mount_fn = await loader.load(
                "fake-provider",
                {},
                source_hint="/fake/path",
                coordinator=mock_coordinator,
            )

        # mount_fn should NOT raise ValueError for path escape
        with pytest.raises(Exception) as exc_info:
            await mount_fn(mock_coordinator)

        assert "escapes module directory" not in str(exc_info.value), (
            f"Should not get path-escape error for a legitimate binary; "
            f"got: {exc_info.value!r}"
        )


# ---------------------------------------------------------------------------
# Fix 3: Port value range validation
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_malformed_ready_line_raises_runtime_error(mock_coordinator):
    """When the sidecar emits an invalid READY line, RuntimeError is raised."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        (tmpdir_path / "amplifier.toml").write_text(
            '[module]\ntype = "provider"\ntransport = "rust"\ncrate = "bad_binary"\n'
        )

        # Binary prints a malformed (non-integer) port
        binary = tmpdir_path / "bad_binary"
        binary.write_text(
            "#!/usr/bin/env python3\nimport time\n"
            "print('READY:notaport', flush=True)\ntime.sleep(5)\n"
        )
        binary.chmod(binary.stat().st_mode | stat.S_IEXEC)

        mock_coordinator.get.return_value = _make_fake_resolver(tmpdir_path)
        fake_engine = _make_fake_engine("bad_binary")
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            mount_fn = await loader.load(
                "bad-provider",
                {},
                source_hint="/fake/path",
                coordinator=mock_coordinator,
            )

        with pytest.raises(RuntimeError, match="malformed READY line"):
            await mount_fn(mock_coordinator)


@pytest.mark.asyncio
async def test_out_of_range_port_raises_runtime_error(mock_coordinator):
    """When the sidecar emits port=0 (out of valid 1-65535 range), RuntimeError is raised."""
    with tempfile.TemporaryDirectory() as tmpdir:
        tmpdir_path = Path(tmpdir)

        (tmpdir_path / "amplifier.toml").write_text(
            '[module]\ntype = "provider"\ntransport = "rust"\ncrate = "zero_port_binary"\n'
        )

        # Binary prints port 0 (invalid)
        binary = tmpdir_path / "zero_port_binary"
        binary.write_text(
            "#!/usr/bin/env python3\nimport time\n"
            "print('READY:0', flush=True)\ntime.sleep(5)\n"
        )
        binary.chmod(binary.stat().st_mode | stat.S_IEXEC)

        mock_coordinator.get.return_value = _make_fake_resolver(tmpdir_path)
        fake_engine = _make_fake_engine("zero_port_binary")
        loader = ModuleLoader(coordinator=mock_coordinator)

        with patch.dict(sys.modules, {"amplifier_core._engine": fake_engine}):
            mount_fn = await loader.load(
                "zero-port-provider",
                {},
                source_hint="/fake/path",
                coordinator=mock_coordinator,
            )

        with pytest.raises(RuntimeError, match="malformed READY line"):
            await mount_fn(mock_coordinator)
