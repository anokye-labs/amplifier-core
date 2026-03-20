"""Tests for _find_package_dir guarding against nonexistent paths.

Bug: _find_package_dir calls module_path.iterdir() without first checking
whether module_path exists, raising FileNotFoundError for absent directories.

Fix: Add an existence check before any file operations.
"""

from amplifier_core.loader import ModuleLoader


def make_loader() -> ModuleLoader:
    """Return a ModuleLoader with no coordinator (sufficient for unit tests)."""
    return ModuleLoader(coordinator=None)


def test_find_package_dir_returns_none_for_nonexistent_path(tmp_path):
    """_find_package_dir must return None (not raise) when module_path doesn't exist."""
    loader = make_loader()
    missing = tmp_path / "does_not_exist"

    # Should NOT raise FileNotFoundError
    result = loader._find_package_dir("some-module", missing)

    assert result is None, (
        f"_find_package_dir must return None for a nonexistent path, got {result!r}"
    )
