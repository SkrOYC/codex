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

## Next Steps

See the main research report in the parent directory or the comprehensive analysis above for:
- Detailed API specifications
- Request/response format differences
- Implementation roadmap
- Configuration examples
- Security considerations

---

**Generated:** 2025-11-17
**Research Phase:** Complete
**Status:** Ready for implementation planning
