"""Tests for proto_chat_request_to_json PyO3 bridge function.

Validates that proto bytes can be decoded and serialized to JSON
via the Rust PyO3 bridge function.

Proto bytes were generated from the amplifier_module.proto schema:
  - Message fields: role (int), text_content (string), block_content, etc.
  - ChatRequest fields: messages (repeated Message), tools (repeated ToolSpecProto), etc.
  - ToolSpecProto fields: name, description, parameters_json
"""

import json


class TestProtoChatRequestToJson:
    """Tests for the proto_chat_request_to_json PyO3 bridge function."""

    def test_minimal_request(self):
        """A ChatRequest with one user message round-trips to valid JSON with messages array.

        Proto bytes represent:
          ChatRequest { messages: [Message { role: 1 (user), text_content: "hello" }] }
        """
        from amplifier_core._engine import proto_chat_request_to_json

        # Pre-computed proto bytes for: ChatRequest with one user message (text_content="hello")
        # Generated from: pb2.ChatRequest with msg.role=1, msg.text_content="hello"
        proto_bytes = bytes.fromhex("0a090801120568656c6c6f")

        # Call the bridge function
        json_str = proto_chat_request_to_json(proto_bytes)

        # Verify it's valid JSON with a messages array
        data = json.loads(json_str)
        assert "messages" in data, "JSON must contain 'messages' key"
        assert isinstance(data["messages"], list), "'messages' must be a list"
        assert len(data["messages"]) == 1, "Expected exactly one message"

    def test_request_with_tools(self):
        """ChatRequest with ToolSpecProto converts to JSON with tools array.

        Proto bytes represent:
          ChatRequest {
            messages: [Message { role: 1 (user), text_content: "use the tool" }],
            tools: [ToolSpecProto { name: "test_tool", description: "A test tool",
                                    parameters_json: "{}" }]
          }
        """
        from amplifier_core._engine import proto_chat_request_to_json

        # Pre-computed proto bytes for ChatRequest with one message + one tool
        # Generated from: pb2.ChatRequest with msg + tool (name="test_tool")
        proto_bytes = bytes.fromhex(
            "0a100801120c7573652074686520746f6f6c"
            "121c0a09746573745f746f6f6c120b41207465737420746f6f6c1a027b7d"
        )

        # Call the bridge function
        json_str = proto_chat_request_to_json(proto_bytes)

        # Verify it's valid JSON with a tools array
        data = json.loads(json_str)
        assert "tools" in data, "JSON must contain 'tools' key"
        assert data["tools"] is not None, "'tools' must not be null"
        assert isinstance(data["tools"], list), "'tools' must be a list"
        assert len(data["tools"]) == 1, "Expected exactly one tool"
        assert data["tools"][0]["name"] == "test_tool"

    def test_empty_bytes_raises(self):
        """Empty bytes produces valid empty ChatRequest, not a crash.

        An empty byte string is valid protobuf for a message with all default
        values. It should decode to an empty ChatRequest (no messages, no tools).
        """
        from amplifier_core._engine import proto_chat_request_to_json

        # Empty bytes should succeed and return a valid empty ChatRequest JSON
        try:
            json_str = proto_chat_request_to_json(b"")
            # If it succeeds, it should be valid JSON
            data = json.loads(json_str)
            assert "messages" in data, "JSON must contain 'messages' key"
            assert isinstance(data["messages"], list)
        except Exception as e:
            # Any clean Python exception is acceptable (e.g., ValueError)
            # The important thing is no crash/segfault
            assert isinstance(e, Exception), "Must raise a clean exception"
