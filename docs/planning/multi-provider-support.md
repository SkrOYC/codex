# Multi-Provider Support: Google Gemini and Anthropic Claude

## Overview

This document outlines the technical plan for extending Codex CLI to support Google Gemini and Anthropic Claude models through protocol-specific adapters.

## Goals

1. Enable users to configure and use Google Gemini models (gemini-2.0-flash, etc.)
2. Enable users to configure and use Anthropic Claude models (claude-3-5-sonnet, etc.)
3. Maintain provider-agnostic agentic loop architecture
4. Ensure tool calling works across all providers
5. Support streaming and multi-turn conversations for all providers

## Current Architecture

### Strengths

The codebase is already well-positioned for provider-agnostic implementation:

- **Clean abstraction**: `ModelProviderInfo` struct in `codex-rs/core/src/model_provider_info.rs` provides provider configuration
- **Protocol enum**: `WireApi` enum supports different wire protocols (currently: `Responses`, `Chat`)
- **Provider-neutral loop**: The agentic loop in `codex-rs/core/src/codex.rs` doesn't branch on provider type
- **Extensible config**: Users can add providers via `~/.codex/config.toml`
- **Unified HTTP client**: `CodexHttpClient` works with any endpoint
- **Aggregation layer**: Chat Completions API aggregator normalizes streaming to match Responses API

### Current Limitations

- **Wire protocols**: Only supports OpenAI Responses API and Chat Completions API
- **Authentication**: OAuth flow hardcoded to OpenAI endpoints in `codex-rs/core/src/auth.rs`
- **Model detection**: Hardcoded OpenAI model patterns in `codex-rs/core/src/model_family.rs`
- **Tool format**: Assumes OpenAI tool call format

## Technical Approach

### Phase 1: Provider Adapter Foundation

#### Objective

Create extensible architecture for integrating providers with non-OpenAI protocols.

#### Components

**1. Extend WireApi Enum**

Location: `codex-rs/core/src/model_provider_info.rs`

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    #[default]
    Responses,         // OpenAI Responses API
    Chat,              // OpenAI Chat Completions (and compatible)
    Gemini,            // Google Gemini API
    AnthropicMessages, // Anthropic Messages API
}
```

**2. Define ProviderAdapter Trait**

Location: `codex-rs/core/src/providers/adapter.rs` (new file)

```rust
#[async_trait]
pub trait ProviderAdapter {
    /// Stream responses from the provider
    async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream>;

    /// Build provider-specific request payload
    fn build_request(&self, prompt: &Prompt) -> Result<Value>;

    /// Parse provider SSE event into internal ResponseEvent
    fn parse_sse_event(&self, event_type: &str, data: &str) -> Result<ResponseEvent>;

    /// Convert tool definitions to provider format
    fn convert_tools(&self, tools: &[ToolSpec]) -> Result<Value>;
}
```

**3. Refactor Client Stream Method**

Location: `codex-rs/core/src/client.rs`

```rust
pub async fn stream(&self, prompt: &Prompt) -> Result<ResponseStream> {
    let adapter = self.get_adapter(); // Factory method based on wire_api
    adapter.stream(prompt).await
}
```

#### Files to Create

- `codex-rs/core/src/providers/mod.rs` - Provider adapter module
- `codex-rs/core/src/providers/adapter.rs` - Trait definition
- `codex-rs/core/src/providers/openai.rs` - Refactored OpenAI adapter (Responses API)
- `codex-rs/core/src/providers/openai_chat.rs` - Refactored Chat Completions adapter

#### Files to Modify

- `codex-rs/core/src/model_provider_info.rs` - Extend WireApi enum
- `codex-rs/core/src/client.rs` - Refactor to use adapter pattern

#### Acceptance Criteria

- ProviderAdapter trait defined with complete documentation
- WireApi enum extended with Gemini and AnthropicMessages variants
- OpenAI adapters refactored to implement trait
- Client uses adapter pattern instead of direct match on wire_api
- SSE event conversion tested (OpenAI → ResponseEvent)
- Tool spec conversion tested (internal → provider format)
- Zero regressions in existing OpenAI functionality
- Unit tests for adapter trait
- Integration tests verifying OpenAI still works
- Documentation updated

---

### Phase 2: Google Gemini Integration

#### Objective

Implement ProviderAdapter for Google Gemini API to enable Gemini models in Codex CLI.

#### Protocol Details

**Endpoint**: `POST {base_url}/models/{model}:generateContent`

**Authentication**: API key in header `x-goog-api-key`

**Request Format**:
```json
{
  "contents": [
    {"role": "user", "parts": [{"text": "..."}]}
  ],
  "tools": [
    {"functionDeclarations": [...]}
  ],
  "generationConfig": {...}
}
```

**SSE Event Format**:
```json
data: {"candidates": [{"content": {"parts": [{"text": "..."}]}}]}
data: {"candidates": [{"content": {"parts": [{"functionCall": {...}}]}}]}
```

#### Key Differences from OpenAI

- Uses `functionDeclarations` instead of `tools` array
- Different SSE format (no event types, just data)
- API key in custom header `x-goog-api-key`
- Different error response format
- Different conversation history format (`contents` with roles and parts)

#### Implementation

**Files to Create**:
- `codex-rs/core/src/providers/gemini.rs` - Gemini adapter implementation

**Core Components**:

1. **GeminiAdapter struct** - Implements ProviderAdapter trait
2. **Request building** - Convert Codex prompt → Gemini contents format
3. **Tool conversion** - Convert Codex tools → Gemini functionDeclarations
4. **SSE parsing** - Parse Gemini events → ResponseEvent
5. **Function call handling** - Convert Gemini functionCall → internal format
6. **Error handling** - Parse and handle Gemini-specific errors

**Model Detection**:
- Add Gemini model patterns to `model_family.rs`
- Context windows: gemini-2.0-flash (1M tokens), gemini-1.5-pro (2M tokens)
- Model capabilities detection

#### User Configuration

Example `~/.codex/config.toml`:

```toml
model = "gemini-2.0-flash"
model_provider = "gemini"

[model_providers.gemini]
name = "Google Gemini"
base_url = "https://generativelanguage.googleapis.com/v1beta"
env_key = "GOOGLE_API_KEY"
wire_api = "gemini"
```

#### Acceptance Criteria

- Gemini models respond to user prompts
- Tool calling works (Read, Write, Bash, Grep, etc.)
- Parallel tool calls supported (if Gemini supports it)
- Streaming text displays correctly in TUI
- Multi-turn conversations maintain context
- Error messages are user-friendly
- Gemini-specific errors handled gracefully
- Model detection works for `gemini-*` patterns
- Configuration documented in `docs/config.md`
- Example config in `docs/example-config.md`

#### Testing Requirements

- Basic chat completion (single turn)
- Multi-turn agentic loop
- Single tool call
- Multiple parallel tool calls
- Large context (> 100K tokens)
- Error scenarios: invalid API key, rate limiting, network timeout, malformed SSE
- Streaming edge cases (reconnection, partial events)

#### Reference

Use `@google/generative-ai` TypeScript SDK for protocol reference:
- Repository: https://github.com/google/generative-ai-js
- Study: Request building, SSE parsing, error handling

---

### Phase 3: Anthropic Claude Integration

#### Objective

Implement ProviderAdapter for Anthropic Claude using the Messages API protocol.

#### Protocol Details

**Endpoint**: `POST {base_url}/v1/messages`

**Required Headers**:
```
x-api-key: {api_key}
anthropic-version: 2023-06-01
content-type: application/json
```

**Request Format**:
```json
{
  "model": "claude-3-5-sonnet-20241022",
  "messages": [{"role": "user", "content": "..."}],
  "tools": [
    {"name": "...", "description": "...", "input_schema": {...}}
  ],
  "max_tokens": 4096,
  "stream": true
}
```

**SSE Event Types**:
- `message_start` - Message metadata
- `content_block_start` - New content block (text or tool_use)
- `content_block_delta` - Incremental content (text delta or tool input)
- `content_block_stop` - Content block complete
- `message_delta` - Message-level updates (stop_reason, usage)
- `message_stop` - Message complete

#### Key Differences from OpenAI

- Different event structure (content blocks vs items)
- Tool use in content blocks, not separate array
- Thinking/reasoning as content block type
- Required `max_tokens` parameter
- Required `anthropic-version` header
- Different tool schema format

#### Event Mapping

```
message_start           → (initialize state)
content_block_start (text)      → OutputItemAdded
content_block_delta (text_delta) → OutputTextDelta
content_block_stop (text)       → OutputItemDone (message)
content_block_start (tool_use)  → OutputItemAdded
content_block_delta (input_json_delta) → (accumulate)
content_block_stop (tool_use)   → OutputItemDone (function_call)
message_stop            → Completed
```

#### Implementation

**Files to Create**:
- `codex-rs/core/src/providers/anthropic.rs` - Anthropic adapter implementation

**Core Components**:

1. **AnthropicAdapter struct** - Implements ProviderAdapter trait
2. **Request building** - Convert Codex prompt → Anthropic messages format
3. **Tool conversion** - Convert Codex tools → Anthropic tool schema
4. **SSE parsing** - Parse Anthropic events → ResponseEvent
5. **Tool use handling** - Convert tool_use blocks → internal format
6. **Thinking support** - Handle thinking/reasoning blocks
7. **Error handling** - Parse and handle Anthropic-specific errors

**Model Detection**:
- Add Claude model patterns to `model_family.rs`
- Context windows: claude-3-5-sonnet (200K tokens), claude-3-opus (200K tokens)
- Extended thinking support (if applicable)
- Parallel tool use detection

#### User Configuration

Example `~/.codex/config.toml`:

```toml
model = "claude-3-5-sonnet-20241022"
model_provider = "anthropic"

[model_providers.anthropic]
name = "Anthropic"
base_url = "https://api.anthropic.com/v1"
env_key = "ANTHROPIC_API_KEY"
wire_api = "anthropic_messages"
```

#### Acceptance Criteria

- Claude models respond to user prompts
- Tool calling works with Anthropic's tool_use format
- Streaming text displays correctly in TUI
- Multi-turn conversations work
- Thinking content handled properly (if supported)
- Error messages are user-friendly
- Anthropic-specific errors handled gracefully
- Model detection works for `claude-*` patterns
- Configuration documented in `docs/config.md`
- Example config in `docs/example-config.md`

#### Testing Requirements

- Basic chat completion (single turn)
- Multi-turn agentic loop
- Single tool call
- Multiple sequential tool calls
- Extended thinking (if model supports)
- Large context (> 100K tokens)
- Error scenarios: invalid API key, rate limiting (429), overloaded (529), network timeout
- Streaming edge cases

#### Reference

Use `@anthropic-ai/sdk` TypeScript SDK for protocol reference:
- Repository: https://github.com/anthropics/anthropic-sdk-typescript
- Study: `src/resources/messages.ts` for event parsing
- Study: `src/streaming.ts` for SSE handling

**Implementation Strategy**: Direct HTTP (using SDK as reference only)
- Cleaner architecture (no Node.js bridge needed)
- Better performance (native Rust)
- Full control over streaming and retries
- SDK source provides complete protocol documentation

---

### Phase 4: Multi-Provider Authentication

#### Objective

Create authentication abstraction to support provider-specific auth flows.

#### Requirements

Support for:
- **API Keys** - Simple bearer tokens (Anthropic, Gemini)
- **OAuth2** - Generic OAuth2 flows (Google, future providers)
- **OpenAI OAuth** - Existing OpenAI-specific flow (maintain compatibility)

#### Architecture

**AuthProvider Trait**:

Location: `codex-rs/core/src/auth.rs` (extend existing)

```rust
#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// Perform initial authentication
    async fn authenticate(&self) -> Result<Credentials>;

    /// Refresh expired credentials
    async fn refresh(&self, creds: &Credentials) -> Result<Credentials>;

    /// Check if credentials are still valid
    fn is_expired(&self, creds: &Credentials) -> bool;

    /// Does this provider support OAuth?
    fn supports_oauth(&self) -> bool;

    /// Provider-specific setup instructions
    fn get_setup_instructions(&self) -> String;
}
```

**Credential Storage**:

```rust
pub struct Credentials {
    pub provider: String,      // "openai", "gemini", "anthropic"
    pub credential_type: CredentialType,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub enum CredentialType {
    ApiKey,           // Static API key
    OAuth2,           // OAuth2 flow
    BearerToken,      // Simple bearer token
}
```

**Provider Implementations**:

```rust
pub struct ApiKeyAuth {
    provider: String,
    env_key: String,
}

pub struct OAuth2Auth {
    provider: String,
    auth_url: String,
    token_url: String,
    client_id: String,
}

pub struct OpenAIAuth {
    // Existing OpenAI OAuth logic (refactored to implement trait)
}
```

#### Implementation

**Files to Modify**:
- `codex-rs/core/src/auth.rs` - Add AuthProvider trait
- `codex-rs/core/src/auth/storage.rs` - Multi-provider credential storage
- `codex-rs/core/src/config/mod.rs` - Provider-specific auth config

**Login Flow**:
- `codex login --provider openai` → OAuth flow
- `codex login --provider gemini` → API key prompt
- `codex login --provider anthropic` → API key prompt
- Auto-detect provider from config if not specified

**Keyring Integration**:
- Store credentials per provider
- Key format: `codex_{provider}_credentials`
- Support multiple provider credentials simultaneously

#### Configuration

```toml
[model_providers.gemini]
# ...
auth_type = "api_key"
env_key = "GOOGLE_API_KEY"

[model_providers.anthropic]
auth_type = "api_key"
env_key = "ANTHROPIC_API_KEY"
```

#### Acceptance Criteria

- AuthProvider trait defined with documentation
- ApiKeyAuth implemented and tested
- Generic OAuth2Auth implemented
- OpenAI OAuth refactored to use trait (no regressions)
- Credentials stored per provider in keyring
- `codex login` supports `--provider` flag
- API key input via stdin (secure, no echo)
- API key validation before storing
- Multiple providers can be authenticated simultaneously
- Clear error messages for auth failures
- Documentation updated

#### Testing Requirements

- OpenAI OAuth flow (regression test)
- API key auth for new providers
- Credential storage and retrieval
- Refresh token flow (OAuth2)
- Multiple simultaneous providers
- Invalid credentials handling
- Environment variable fallback
- Keyring errors (graceful degradation)

---

## Implementation Phases Summary

### Phase 1: Provider Adapter Foundation
- Define ProviderAdapter trait
- Extend WireApi enum
- Refactor OpenAI implementations to use adapter pattern
- Ensure zero regressions

### Phase 2: Google Gemini Integration
- Implement GeminiAdapter
- Add Gemini protocol support
- Handle Gemini-specific SSE events and tool format
- Test with Gemini models

### Phase 3: Anthropic Claude Integration
- Implement AnthropicAdapter
- Add Anthropic Messages API support
- Handle Anthropic-specific SSE events and tool_use blocks
- Test with Claude models

### Phase 4: Multi-Provider Authentication
- Define AuthProvider trait
- Implement API key authentication
- Implement generic OAuth2 authentication
- Refactor OpenAI auth to use trait
- Support multiple simultaneous providers

---

## Success Criteria

### Functional Requirements

- ✅ Users can configure Google Gemini models via config.toml
- ✅ Users can configure Anthropic Claude models via config.toml
- ✅ Agentic loop works identically across all providers
- ✅ Tool calling (Read, Write, Bash, Grep, etc.) works for all providers
- ✅ Streaming responses display correctly in TUI for all providers
- ✅ Multi-turn conversations maintain context across all providers
- ✅ Error handling is robust and user-friendly

### Non-Functional Requirements

- ✅ Zero performance regression on existing OpenAI providers
- ✅ Code maintains clean separation of concerns
- ✅ Provider-specific logic isolated to adapter implementations
- ✅ Agentic loop remains provider-agnostic
- ✅ Configuration is intuitive and well-documented
- ✅ Authentication flows are secure

### Documentation Requirements

- ✅ Architecture documentation for provider adapters
- ✅ Configuration examples for Gemini and Anthropic
- ✅ User guide for setting up each provider
- ✅ API reference for ProviderAdapter trait
- ✅ Developer guide for adding new providers

---

## Future Considerations

### Additional Providers

The architecture should support future providers:
- Cohere Command models
- Mistral AI native API
- Amazon Bedrock
- Azure OpenAI (with custom auth)

### Advanced Features

Possible future enhancements:
- Multi-modal support (images, audio)
- Prompt caching (Anthropic)
- Grounding/search (Gemini)
- Extended thinking controls
- Model routing (select provider based on task)
- Cost optimization (cheapest provider for task)

### Performance Optimizations

- Connection pooling per provider
- Request batching where supported
- Parallel provider requests for comparison
- Streaming optimizations

---

## References

### Documentation

- [Google Gemini API Docs](https://ai.google.dev/api)
- [Anthropic Messages API Docs](https://docs.anthropic.com/claude/reference/messages_post)
- [OpenAI Responses API Docs](https://platform.openai.com/docs/api-reference/responses)
- [OpenAI Chat Completions API Docs](https://platform.openai.com/docs/api-reference/chat)

### Code References

- Current provider abstraction: `codex-rs/core/src/model_provider_info.rs`
- Agentic loop: `codex-rs/core/src/codex.rs`
- Existing auth: `codex-rs/core/src/auth.rs`
- Tool execution: `codex-rs/core/src/tools/`
- Chat Completions aggregation: `codex-rs/core/src/chat_completions.rs`

### SDK References

- Google Generative AI SDK: https://github.com/google/generative-ai-js
- Anthropic SDK: https://github.com/anthropics/anthropic-sdk-typescript

---

## Appendix: Architecture Diagrams

### Current Architecture

```
User Request
    ↓
Codex Loop (codex.rs)
    ↓
ModelClient (client.rs)
    ↓
    ├─→ WireApi::Responses → stream_responses()
    └─→ WireApi::Chat → stream_chat_completions() → Aggregator
         ↓
    ResponseStream (unified events)
         ↓
Tool Execution (tools/)
         ↓
Back to Codex Loop
```

### Proposed Architecture

```
User Request
    ↓
Codex Loop (codex.rs) [provider-agnostic]
    ↓
ModelClient (client.rs)
    ↓
ProviderAdapter Factory
    ↓
    ├─→ WireApi::Responses → OpenAIAdapter
    ├─→ WireApi::Chat → ChatCompletionsAdapter
    ├─→ WireApi::Gemini → GeminiAdapter
    └─→ WireApi::AnthropicMessages → AnthropicAdapter
         ↓
Protocol-specific HTTP/SSE
         ↓
    ResponseStream (unified events)
         ↓
Tool Execution (tools/) [provider-agnostic]
         ↓
Back to Codex Loop
```

### Authentication Flow

```
codex login --provider <name>
    ↓
AuthManager
    ↓
AuthProvider Factory
    ↓
    ├─→ OpenAI → OpenAIAuth (OAuth2)
    ├─→ Gemini → ApiKeyAuth
    └─→ Anthropic → ApiKeyAuth
         ↓
Credentials
         ↓
Keyring Storage (per provider)
```

---

## Conclusion

This plan provides a comprehensive approach to extending Codex CLI with multi-provider support while maintaining the existing provider-agnostic architecture. The phased approach ensures:

1. **Solid foundation** - ProviderAdapter trait provides extensibility
2. **Clean implementation** - Each provider isolated in its own adapter
3. **Zero regression** - Existing OpenAI functionality remains unchanged
4. **Future-proof** - Architecture supports additional providers easily
5. **User-friendly** - Simple configuration and authentication flows

The implementation leverages Rust's strengths (performance, type safety) while using official SDKs as protocol references to ensure correct implementation of each provider's API.
