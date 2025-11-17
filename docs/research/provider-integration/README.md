# Provider Integration Research Documentation

This directory contains research and analysis documentation for implementing direct API access to Google GenAI and Anthropic Claude APIs, making the Codex fork provider-agnostic.

## Documents

### 1. [Codebase Summary](./codebase_summary.md)
**Purpose:** High-level overview of the Codex codebase architecture relevant to provider integration.

**Contents:**
- Repository structure overview
- Current provider integration system
- API client architecture
- Request/response protocol
- Configuration system
- Authentication system
- Provider integration examples
- Core dependencies
- Wire API definitions

**Use this for:** Understanding the overall architecture and how providers are currently integrated.

---

### 2. [Detailed File Reference](./detailed_file_reference.md)
**Purpose:** Specific file paths, line numbers, and function references for implementation.

**Contents:**
- Critical path files with exact locations
- Key functions and their line numbers
- Provider configuration system details
- HTTP client layer implementation
- Model client interface
- Request/response structures
- Configuration system files
- Authentication system files
- Provider implementation examples
- Direct implementation paths for Google GenAI and Anthropic

**Use this for:** Actual implementation work - knowing exactly which files to modify and where key functions are located.

---

### 3. [Architecture and Flow](./architecture_and_flow.md)
**Purpose:** Visual diagrams and data flow sequences for understanding system behavior.

**Contents:**
- System architecture diagram (ASCII art)
- Request/response flow sequences for both Chat Completions and Responses APIs
- Provider configuration resolution flow
- Environment variable resolution flow
- Authentication flow diagram
- Data structure transformations
- Configuration hierarchy
- Provider detection and selection
- Key design patterns

**Use this for:** Understanding how data flows through the system, request/response lifecycles, and architectural patterns.

---

## Research Findings Summary

### Current State
- ✅ Codex architecture is **already provider-agnostic**
- ✅ Excellent abstraction layers in place (`ModelProviderInfo`, `WireApi` enum)
- ✅ Configuration-driven provider system via `~/.codex/config.toml`
- ✅ Two built-in providers: `openai` (Responses API) and `oss` (Ollama, Chat API)

### Integration Approach
- **Recommended:** Extend `WireApi` enum with `GoogleGenAI` and `AnthropicMessages` variants
- Create dedicated adapter modules for each provider
- Transform internal `Prompt` structure to provider-specific formats
- Parse provider responses back to unified `ResponseEvent` stream

### Key Files to Modify
1. `/home/user/codex/codex-rs/core/src/model_provider_info.rs` - Provider definitions
2. `/home/user/codex/codex-rs/core/src/client.rs` - Request routing
3. `/home/user/codex/codex-rs/core/src/chat_completions.rs` - Chat API reference

### Files to Create
1. `/home/user/codex/codex-rs/core/src/google_genai.rs` - Google GenAI adapter
2. `/home/user/codex/codex-rs/core/src/anthropic.rs` - Anthropic adapter
3. `/home/user/codex/codex-rs/core/src/types/google_genai.rs` - Type definitions
4. `/home/user/codex/codex-rs/core/src/types/anthropic.rs` - Type definitions

### Effort Estimate
- **Total:** 21-40 hours
- **Complexity:** Medium
- **Phases:**
  1. Architecture Extension: 1-2 hours
  2. Google GenAI Integration: 8-16 hours
  3. Anthropic Integration: 6-12 hours
  4. Testing & Documentation: 6-10 hours

## API Comparison Quick Reference

| Feature | OpenAI | Google GenAI | Anthropic |
|---------|--------|--------------|-----------|
| **Base URL** | `api.openai.com/v1` | `generativelanguage.googleapis.com/v1beta` | `api.anthropic.com/v1` |
| **Auth Header** | `Authorization: Bearer` | `x-goog-api-key` | `x-api-key` + `anthropic-version` |
| **System Msg** | In messages array | `systemInstruction` field | `system` field |
| **Message Format** | `{role, content}` | `{role, parts: [{text}]}` | `{role, content}` |
| **Tool Format** | `parameters` | `parametersJsonSchema` | `input_schema` |

## Core Design Implementation

### WireApi Extension (COMPLETED)

The `WireApi` enum has been extended with two new variants in `codex-rs/core/src/model_provider_info.rs`:
- `GoogleGenAI`: For Google Generative Language API (Gemini models)
- `AnthropicMessages`: For Anthropic Messages API (Claude models)

```rust
pub enum WireApi {
    Responses,           // OpenAI Responses API
    Chat,               // Chat Completions API
    GoogleGenAI,        // Google GenAI API
    AnthropicMessages,  // Anthropic Messages API
}
```

### Provider IDs and Configuration

Built-in provider IDs (defined in `built_in_model_providers()`):
- `openai` - OpenAI Responses API ✅
- `oss` - Local OSS/Ollama (Chat Completions) ✅
- `google_genai` - Google GenAI (Generative Language API) ✅ **IMPLEMENTED**
- `anthropic` - Anthropic Messages API (configuration available, implementation pending)

### Environment Variables

**Google GenAI:**
- `GOOGLE_GENAI_API_KEY` - Required API key (passed via `x-goog-api-key` header)
- `GOOGLE_GENAI_BASE_URL` - Optional base URL override (default: `https://generativelanguage.googleapis.com/v1beta`)

**Anthropic:**
- `ANTHROPIC_API_KEY` - Required API key (passed via `x-api-key` header)
- `ANTHROPIC_BASE_URL` - Optional base URL override (default: `https://api.anthropic.com/v1`)

### Provider Configuration Examples

**Using Google GenAI:**
```toml
# ~/.codex/config.toml
model_provider = "google_genai"
model = "gemini-2.0-flash"  # or gemini-1.5-pro, gemini-1.5-flash, etc.
```

Set the API key:
```bash
export GOOGLE_GENAI_API_KEY="your-api-key-from-ai-studio"
```

Get your API key from: https://aistudio.google.com/app/apikey

**Using Anthropic:**
```toml
# ~/.codex/config.toml
model_provider = "anthropic"
model = "claude-3-5-sonnet-20241022"
```

### Implementation Status

**Core Design (✅ COMPLETED):**
- WireApi enum extended with new variants
- Provider factory functions implemented (`create_google_genai_provider()`, `create_anthropic_provider()`)
- Built-in providers registry updated
- ModelClient routing extended with placeholder error handling
- Comprehensive unit tests added
- Documentation updated

**Google GenAI Provider (✅ COMPLETED):**
- Full request/response mapping implementation
- Streaming SSE support
- Function calling (tool use)
- Multi-turn conversations
- Token usage reporting
- Error handling and retries
- Comprehensive unit tests (14 test cases)
- Complete documentation in [google_genai_mapping.md](./google_genai_mapping.md)

**Anthropic Messages Provider (⏳ PENDING):**
- Request/response mapping (separate issue)
- Streaming support (separate issue)

### Technical Details

**URL Construction:**
- Google GenAI: `{base_url}/models/{model}:streamGenerateContent`
- Anthropic: `{base_url}/messages`

**Authentication:**
- Google GenAI: Uses `x-goog-api-key` header from environment
- Anthropic: Uses `x-api-key` header from environment, plus static `anthropic-version: 2023-06-01` header

**Error Handling:**
- Google GenAI: Fully operational with retry logic and proper error messages
- Anthropic: Placeholder error message until implementation is complete:
  ```
  Anthropic Messages provider is not yet fully implemented.
  The provider configuration is available for testing, but request/response
  mapping will be added in a future release.
  ```

## Google GenAI Implementation

The Google GenAI provider is now fully integrated and operational. See [google_genai_mapping.md](./google_genai_mapping.md) for:
- Complete request/response mapping documentation
- Supported features and limitations
- Configuration examples
- Troubleshooting guide
- API reference

**Key Files:**
- Implementation: `codex-rs/core/src/google_genai.rs`
- Tests: 14 comprehensive unit tests in the same file
- Integration: `codex-rs/core/src/client.rs` (WireApi::GoogleGenAI branch)
- Module export: `codex-rs/core/src/lib.rs`

## Next Steps

**For Anthropic Integration:**
See the main research report in the parent directory or the comprehensive analysis above for:
- Detailed API specifications
- Request/response format differences
- Implementation roadmap for Anthropic adapter
- Configuration examples
- Security considerations

**For Google GenAI Users:**
- Refer to [google_genai_mapping.md](./google_genai_mapping.md) for usage documentation
- Run tests: `cargo test -p codex-core google_genai`
- Set up your API key: `export GOOGLE_GENAI_API_KEY="..."`
- Configure in `~/.codex/config.toml` with `model_provider = "google_genai"`

---

**Generated:** 2025-11-17
**Research Phase:** Complete ✅
**Core Design Phase:** Complete ✅
**Google GenAI Implementation:** Complete ✅
**Status:** Google GenAI ready for production use; Anthropic pending implementation
