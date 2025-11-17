# Google GenAI Provider Integration

## Overview

This document describes the integration of Google's Generative Language API (Gemini) as a first-class provider in Codex. The integration enables users to use Google's Gemini models through Codex's unified interface.

## API Details

- **Provider ID**: `google_genai`
- **API Endpoint**: `https://generativelanguage.googleapis.com/v1beta`
- **Streaming Endpoint**: `/models/{model}:streamGenerateContent`
- **API Version**: `v1beta`
- **Authentication**: API key via `x-goog-api-key` header

## Configuration

### Environment Variables

- `GOOGLE_GENAI_API_KEY` (required): Your Google AI Studio API key
  - Get your API key from: https://aistudio.google.com/app/apikey
- `GOOGLE_GENAI_BASE_URL` (optional): Override the default base URL

### config.toml Example

```toml
[model_providers.google_genai]
name = "Google GenAI"
base_url = "https://generativelanguage.googleapis.com/v1beta"
env_key = "GOOGLE_GENAI_API_KEY"
wire_api = "google_genai"
```

To use a Google GenAI model:

```toml
[model]
provider = "google_genai"
model = "gemini-2.0-flash"
```

## Request/Response Mapping

### Request Structure Mapping

#### System Instructions

Codex's system instructions are mapped to Google's `systemInstruction` field:

```rust
// Codex Internal
model_family.base_instructions

// Google GenAI Request
{
  "systemInstruction": {
    "role": "user",
    "parts": [{"text": "..."}]
  }
}
```

#### Conversation Messages

Codex's `ResponseItem::Message` is mapped to Google's `Content` structure:

| Codex Role | Google Role | Notes |
|------------|-------------|-------|
| `user` | `user` | Direct mapping |
| `assistant` | `model` | Google uses "model" instead of "assistant" |

Example mapping:

```rust
// Codex Internal
ResponseItem::Message {
    role: "user",
    content: vec![ContentItem::InputText { text: "Hello" }]
}

// Google GenAI Request
{
  "role": "user",
  "parts": [{"text": "Hello"}]
}
```

#### Function Calls

Codex's function calls are mapped to Google's `functionCall` part:

```rust
// Codex Internal
ResponseItem::FunctionCall {
    name: "get_weather",
    arguments: r#"{"location":"Paris"}"#,
    call_id: "call_123"
}

// Google GenAI Request
{
  "role": "model",
  "parts": [{
    "functionCall": {
      "name": "get_weather",
      "args": {"location": "Paris"}
    }
  }]
}
```

#### Function Responses

Codex's function call outputs are mapped to Google's `functionResponse` part:

```rust
// Codex Internal
ResponseItem::FunctionCallOutput {
    call_id: "call_123",
    output: FunctionCallOutputPayload {
        content: "The weather is sunny"
    }
}

// Google GenAI Request
{
  "role": "user",
  "parts": [{
    "functionResponse": {
      "name": "function",
      "response": {"output": "The weather is sunny"}
    }
  }]
}
```

#### Tools

Codex's `ToolSpec` is mapped to Google's `tools` array:

```rust
// Codex Internal
ToolSpec::Function(FunctionToolDefinition {
    name: "get_weather",
    description: Some("Get weather for a location"),
    parameters: Some(schema)
})

// Google GenAI Request
{
  "tools": [{
    "functionDeclarations": [{
      "name": "get_weather",
      "description": "Get weather for a location",
      "parameters": { /* JSON Schema */ }
    }]
  }]
}
```

### Response Structure Mapping

#### Text Deltas

Google streams text content in `candidates[0].content.parts[].text`:

```json
{
  "candidates": [{
    "content": {
      "role": "model",
      "parts": [{"text": "Hello"}]
    }
  }]
}
```

This is mapped to:
- `ResponseEvent::OutputTextDelta("Hello")` - For streaming chunks
- `ResponseEvent::OutputItemDone(ResponseItem::Message {...})` - When complete

#### Function Calls in Responses

Google returns function calls in parts:

```json
{
  "candidates": [{
    "content": {
      "role": "model",
      "parts": [{
        "functionCall": {
          "name": "get_weather",
          "args": {"location": "Paris"}
        }
      }]
    }
  }]
}
```

This is mapped to:
- `ResponseEvent::OutputItemAdded(ResponseItem::FunctionCall {...})`

#### Completion and Token Usage

Google sends completion information in the final chunk:

```json
{
  "candidates": [{
    "finishReason": "STOP"
  }],
  "usageMetadata": {
    "promptTokenCount": 10,
    "candidatesTokenCount": 20,
    "totalTokenCount": 30
  }
}
```

This is mapped to:
- `ResponseEvent::Completed { response_id, token_usage }`

Where `token_usage` contains:
```rust
TokenUsage {
    input_tokens: promptTokenCount,
    output_tokens: candidatesTokenCount
}
```

## Supported Features

### ✅ Currently Supported

- Text-only conversations (user/assistant messages)
- Function calling (tool use)
- Function responses (tool outputs)
- Streaming responses with text deltas
- Token usage reporting
- Multi-turn conversations
- System instructions
- Retry logic with exponential backoff
- Timeout handling

### ⚠️ Partial Support

- Tool definitions (Function and Freeform specs only)
- Error handling (basic HTTP status codes)

### ❌ Not Yet Supported

- Image inputs (multimodal)
- Reasoning blocks (Google-specific feature)
- Safety settings configuration
- Generation config parameters (temperature, top_p, max_tokens)
- Grounding and retrieval features
- Code execution
- Custom tool types beyond Function and Freeform

## Implementation Details

### Module Structure

- **File**: `codex-rs/core/src/google_genai.rs`
- **Entry point**: `stream_google_genai()`
- **Request builder**: `build_google_genai_request()`
- **SSE processor**: `process_google_genai_sse()`

### URL Construction

The Google GenAI endpoint requires the model name in the URL path:

```
https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent
```

The `{model}` placeholder is replaced at runtime with the actual model name (e.g., `gemini-2.0-flash`).

### Authentication Flow

1. User sets `GOOGLE_GENAI_API_KEY` environment variable
2. Provider configuration specifies `env_http_headers`:
   ```rust
   env_http_headers: Some([
       ("x-goog-api-key".to_string(), "GOOGLE_GENAI_API_KEY".to_string())
   ])
   ```
3. `ModelProviderInfo::create_request_builder()` reads the env var and adds the header
4. Requests include `x-goog-api-key: <API_KEY>` header

### Error Handling

The implementation follows Codex's standard error handling patterns:

- **429 (Too Many Requests)**: Retry with exponential backoff
- **5xx (Server Errors)**: Retry with exponential backoff
- **4xx (Client Errors)**: Fail immediately with error details
- **Network errors**: Retry up to `request_max_retries` times
- **Stream timeouts**: Fail after `stream_idle_timeout` (default 5 minutes)

### Streaming Implementation

Google GenAI uses Server-Sent Events (SSE) for streaming:

1. HTTP POST to the streaming endpoint
2. Response is a stream of SSE events
3. Each event contains JSON with `candidates` array
4. Text deltas are extracted from `parts[].text`
5. Function calls are extracted from `parts[].functionCall`
6. Stream completes when `finishReason` is present

## Known Limitations

1. **Empty messages are skipped**: Messages with no text content are not sent to Google
2. **Generic function response names**: Function responses use a generic "function" name rather than tracking the original function name
3. **No image support**: Image inputs/outputs are ignored in the current implementation
4. **Reasoning blocks skipped**: Codex's reasoning blocks are not mapped to Google's equivalent
5. **Custom tool types**: LocalShellCall, CustomToolCall, and other Codex-specific tool types are not supported

## Testing

### Unit Tests

Located in `codex-rs/core/src/google_genai.rs` under `#[cfg(test)] mod tests`:

- `test_simple_text_message_mapping()` - Basic text message conversion
- `test_assistant_role_mapping()` - Role name conversion (assistant → model)
- `test_function_call_mapping()` - Function call structure mapping
- `test_function_response_mapping()` - Function response structure mapping
- `test_multi_turn_conversation_mapping()` - Multi-turn conversation handling
- `test_tool_spec_conversion_function()` - Function tool conversion
- `test_tool_spec_conversion_freeform()` - Freeform tool conversion
- `test_empty_message_skipped()` - Empty message filtering
- `test_url_construction()` - URL placeholder replacement
- `test_parse_google_genai_text_chunk()` - Text chunk parsing
- `test_parse_google_genai_function_call_chunk()` - Function call chunk parsing
- `test_parse_google_genai_completion_chunk()` - Completion chunk with usage metadata
- `test_parse_malformed_chunk()` - Malformed JSON handling

Run tests with:

```bash
cargo test -p codex-core google_genai
```

## Troubleshooting

### "API Key not found" error

**Cause**: `GOOGLE_GENAI_API_KEY` environment variable not set

**Solution**:
```bash
export GOOGLE_GENAI_API_KEY="your-api-key-here"
```

### "Model not found" error

**Cause**: Invalid model name or model not available in your region

**Solution**: Use a valid Google GenAI model name like `gemini-2.0-flash` or `gemini-1.5-pro`

### Streaming timeout

**Cause**: Google GenAI taking longer than expected to respond

**Solution**: Increase `stream_idle_timeout_ms` in provider configuration:

```toml
[model_providers.google_genai]
stream_idle_timeout_ms = 600000  # 10 minutes
```

### Function calling not working

**Cause**: Tool definitions might not be compatible with Google's function calling format

**Solution**: Ensure your tool definitions use standard JSON Schema format in `parameters` field

## Future Enhancements

Potential areas for future development:

1. **Multimodal support**: Add image input/output handling
2. **Generation config**: Expose temperature, top_p, max_tokens parameters
3. **Safety settings**: Configure content filtering and safety thresholds
4. **Grounding**: Support Google Search grounding and custom grounding
5. **Code execution**: Enable Google's code execution feature
6. **Thinking/Reasoning**: Map Google's thinking blocks to Codex reasoning
7. **Better function response tracking**: Preserve function names in responses
8. **System instructions in parts**: Use Google's native systemInstruction field properly

## References

- Google GenAI API Documentation: https://ai.google.dev/api/rest/v1beta
- TypeScript SDK: https://github.com/googleapis/js-genai
- API Key Management: https://aistudio.google.com/app/apikey
- Model Information: https://ai.google.dev/gemini-api/docs/models/gemini
