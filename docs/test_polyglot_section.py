"""
TDD test for polyglot module loading section in DESIGN_PHILOSOPHY.md.
"""
import re
import os


DESIGN_PHILOSOPHY_PATH = os.path.join(
    os.path.dirname(__file__), "DESIGN_PHILOSOPHY.md"
)


def read_doc():
    with open(DESIGN_PHILOSOPHY_PATH, "r") as f:
        return f.read()


def test_polyglot_section_exists():
    """Section '## Polyglot Module Loading' must exist in the file."""
    content = read_doc()
    assert "## Polyglot Module Loading" in content, (
        "Section '## Polyglot Module Loading' not found in DESIGN_PHILOSOPHY.md"
    )


def test_polyglot_section_before_implementation_patterns():
    """Polyglot section must appear before '## Implementation Patterns'."""
    content = read_doc()
    polyglot_pos = content.find("## Polyglot Module Loading")
    impl_pos = content.find("## Implementation Patterns")
    assert polyglot_pos != -1, "Section '## Polyglot Module Loading' not found"
    assert impl_pos != -1, "Section '## Implementation Patterns' not found"
    assert polyglot_pos < impl_pos, (
        "Polyglot section must appear BEFORE Implementation Patterns"
    )


def test_polyglot_section_has_four_transport_types():
    """Section must mention all four transport types: python, rust, wasm, grpc."""
    content = read_doc()
    polyglot_pos = content.find("## Polyglot Module Loading")
    impl_pos = content.find("## Implementation Patterns")
    assert polyglot_pos != -1, "Section '## Polyglot Module Loading' not found"
    section = content[polyglot_pos:impl_pos]

    for transport in ["python", "rust", "wasm", "grpc"]:
        assert transport in section, (
            f"Transport type '{transport}' not mentioned in Polyglot section"
        )


def test_polyglot_section_has_module_declares_subsection():
    """Section must contain the subsection about module declaration vs host consumption."""
    content = read_doc()
    assert "The Module Declares What It IS, The Host Decides How to CONSUME" in content, (
        "Subsection about module declaration vs host consumption not found"
    )


def test_polyglot_section_has_grpc_bridge_subsection():
    """Section must contain the subsection about gRPC as universal cross-language bridge."""
    content = read_doc()
    assert "gRPC as Universal Cross-Language Bridge" in content, (
        "Subsection about gRPC as Universal Cross-Language Bridge not found"
    )


def test_no_changelog_language_in_polyglot_section():
    """No changelog language in the new section (no 'added in', 'new as of', version tags, etc.)."""
    content = read_doc()
    polyglot_pos = content.find("## Polyglot Module Loading")
    impl_pos = content.find("## Implementation Patterns")
    if polyglot_pos == -1:
        return  # Section missing — other tests handle this
    section = content[polyglot_pos:impl_pos]

    forbidden_patterns = [
        r"\badded\b",
        r"\bnew as of\b",
        r"\bv1\.\d+\.\d+\b",
        r"\bchangelog\b",
        r"\baddendum\b",
    ]
    for pattern in forbidden_patterns:
        matches = re.findall(pattern, section, re.IGNORECASE)
        assert not matches, (
            f"Changelog language found in Polyglot section: {matches} (pattern: {pattern})"
        )


if __name__ == "__main__":
    import sys

    tests = [
        test_polyglot_section_exists,
        test_polyglot_section_before_implementation_patterns,
        test_polyglot_section_has_four_transport_types,
        test_polyglot_section_has_module_declares_subsection,
        test_polyglot_section_has_grpc_bridge_subsection,
        test_no_changelog_language_in_polyglot_section,
    ]

    failed = []
    for test in tests:
        try:
            test()
            print(f"  PASS: {test.__name__}")
        except AssertionError as e:
            print(f"  FAIL: {test.__name__}: {e}")
            failed.append(test.__name__)

    if failed:
        print(f"\n{len(failed)} test(s) failed.")
        sys.exit(1)
    else:
        print(f"\nAll {len(tests)} tests passed.")
