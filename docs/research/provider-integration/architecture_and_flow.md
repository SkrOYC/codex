# Architecture and Data Flow - Visual Guide

## System Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     Codex Application                           │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  CLI/TUI Layer (codex-cli, codex-tui)                          │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Configuration System                       │   │
│  │  ~/.codex/config.toml (Model, Provider, Settings)     │   │
│  └─────────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Config Loader (core/src/config/mod.rs)               │   │
│  │  - Load TOML                                          │   │
│  │  - Merge with CLI overrides                           │   │
│  │  - Validate configuration                             │   │
│  └─────────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Core Codex Engine (codex-core)                        │   │
│  │                                                        │   │
│  │  ┌──────────────────────────────────────────────────┐  │   │
│  │  │  ConversationManager                            │  │   │
│  │  │  - Manage conversation history                  │  │   │
│  │  │  - Message state tracking                       │  │   │
│  │  │  - Context management                           │  │   │
│  │  └──────────────────────────────────────────────────┘  │   │
│  │       │                                                 │   │
│  │       ▼                                                 │   │
│  │  ┌──────────────────────────────────────────────────┐  │   │
│  │  │  ModelClient (core/src/client.rs)              │  │   │
│  │  │  - Dispatch to API (Responses or Chat)          │  │   │
│  │  │  - Stream response handling                     │  │   │
│  │  │  - Error handling & retries                     │  │   │
│  │  └──────────────────────────────────────────────────┘  │   │
│  │       │                                                 │   │
│  │       ├─────────────────────────────────────────────┐   │   │
│  │       │                                             │   │   │
│  │       ▼                                             ▼   │   │
│  │  Responses API                           Chat Completions │   │
│  │  (core/src/client.rs)                    (chat_completions.rs)
│  │  - Sends ResponsesApiRequest             - Builds messages array
│  │  - Parses structured response            - Sends Chat request
│  │  - Handles reasoning blocks              - Parses delta stream
│  │                                                        │   │
│  └──────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Provider Manager                                      │   │
│  │  (core/src/model_provider_info.rs)                    │   │
│  │  - Provider lookup                                    │   │
│  │  - Request builder creation                           │   │
│  │  - Header/auth application                            │   │
│  │  - URL construction                                   │   │
│  └─────────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  HTTP Client (core/src/default_client.rs)             │   │
│  │  - Bearer token auth                                  │   │
│  │  - Custom headers (originator, User-Agent)            │   │
│  │  - Request/response logging                           │   │
│  │  - Connection management                              │   │
│  └─────────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Authentication (core/src/auth.rs)                    │   │
│  │  - OAuth token handling                               │   │
│  │  - API key management                                 │   │
│  │  - Token refresh                                      │   │
│  │  - Credential storage (~/.codex/auth.json)            │   │
│  └─────────────────────────────────────────────────────────┘   │
│       │                                                         │
│       ▼                                                         │
│      [HTTP Network Layer - reqwest]                           │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
          │
          │ HTTP/HTTPS
          ▼
    ┌─────────────────────────────────────────┐
    │        External LLM Providers            │
    ├─────────────────────────────────────────┤
    │                                         │
    │  • OpenAI (built-in)                   │
    │    https://api.openai.com/v1            │
    │                                         │
    │  • Custom Provider (configurable)      │
    │    (User-defined base_url)             │
    │                                         │
    │  • Anthropic Claude (via config)       │
    │    https://api.anthropic.com/v1         │
    │                                         │
    │  • Google GenAI (via config)           │
    │    https://generativelanguage.googleapis.com
    │                                         │
    │  • Azure OpenAI (via config)           │
    │    https://{resource}.openai.azure.com │
    │                                         │
    │  • Ollama/Local (via config)           │
    │    http://localhost:11434/v1            │
    │                                         │
    └─────────────────────────────────────────┘
```

## Request/Response Flow Sequence

### Chat Completions Flow

```
User Input
    │
    ▼
ConversationManager::add_user_message()
    │
    ├─ Store message in history
    ├─ Build Prompt struct
    │  ├─ input: ResponseItem[] (conversation items)
    │  ├─ tools: ToolSpec[] (available tools)
    │  └─ output_schema: Optional JSON schema
    │
    ▼
ModelClient::stream()
    │
    ├─ Check provider.wire_api
    │  └─ If WireApi::Chat, call stream_chat_completions()
    │
    ▼
stream_chat_completions()
    │
    ├─ Build messages array:
    │  ├─ [0]: System message with instructions
    │  ├─ [1...N]: Conversation history
    │  └─ Include reasoning blocks, tool calls
    │
    ├─ Transform tools to OpenAI format:
    │  └─ ToolSpec[] → OpenAI function definitions
    │
    ├─ Create request JSON:
    │  {
    │    "model": "gpt-4",
    │    "messages": [...],
    │    "tools": [function definitions],
    │    "stream": true,
    │    "temperature": ...,
    │    "top_p": ...
    │  }
    │
    ▼
ModelProviderInfo::create_request_builder()
    │
    ├─ Load API key from env_key environment variable
    ├─ Apply static headers (http_headers)
    ├─ Apply dynamic headers (env_http_headers from env)
    ├─ Set bearer token auth
    ├─ Build full URL:
    │  └─ base_url + "/chat/completions" + query_params
    │
    ▼
CodexRequestBuilder::json() + ::send()
    │
    ├─ Build POST request with JSON body
    ├─ Add all headers
    ├─ Send via reqwest
    │
    ▼
HTTP POST to Provider Endpoint
    │
    ▼
Provider API Processes Request
    │
    ▼
Provider Returns SSE Stream
    │
    ├─ Event: {"type": "content_block_start", ...}
    ├─ Event: {"type": "content_block_delta", "delta": {"text": "chunk"}}
    ├─ Event: {"type": "content_block_stop", ...}
    ├─ Event: {"type": "message_stop", ...}
    │
    ▼
eventsource_stream Parser
    │
    ├─ Receive: data: {json}\n\n
    ├─ Parse JSON
    ├─ Extract delta text chunks
    ├─ Aggregate message tokens
    │
    ▼
AggregatedChatStream
    │
    ├─ Buffer chunks for this content block
    ├─ Emit complete message when done
    ├─ Handle tool calls
    ├─ Format tool call outputs
    │
    ▼
ResponseEvent Stream
    │
    ├─ ContentBlock (text token)
    ├─ ToolCall (function invocation)
    ├─ ToolCallResult (function output)
    │
    ▼
TUI/CLI Output
    │
    └─ Display streamed response in real-time
```

### Responses API Flow

```
User Input
    │
    ▼
ModelClient::stream_responses()
    │
    ├─ Build Prompt struct (same as Chat)
    │
    ├─ Build ResponsesApiRequest:
    │  {
    │    "model": "gpt-5",
    │    "instructions": "system_prompt",
    │    "input": [...ResponseItem],
    │    "tools": [{"type": "function", "function": {...}}],
    │    "stream": true,
    │    "include": ["reasoning.encrypted_content"],
    │    "reasoning": {
    │      "effort": "medium",
    │      "summary": "auto"
    │    }
    │  }
    │
    ▼
ModelProviderInfo::create_request_builder()
    │
    ├─ Same as Chat flow
    ├─ URL: base_url + "/responses" + query_params
    │
    ▼
HTTP POST to /v1/responses endpoint
    │
    ▼
Provider Responses API
    │
    ├─ Process structured request
    ├─ Generate reasoning blocks
    ├─ Handle tool use
    │
    ▼
SSE Stream Response
    │
    ├─ event: content_block_start
    │  data: {"type": "content_block_start", "index": 0}
    │
    ├─ event: content_block_delta
    │  data: {"type": "content_block_delta", "delta": {"text": "..."}}
    │
    ├─ event: message_delta
    │  data: {"type": "message_delta", "delta": {...}}
    │
    ├─ event: message_stop
    │  data: {"type": "message_stop"}
    │
    ▼
Parser & Aggregation
    │
    ├─ Extract reasoning blocks
    ├─ Aggregate text deltas
    ├─ Parse tool calls
    │
    ▼
ResponseEvent Stream (Same as Chat)
```

## Provider Configuration Resolution Flow

```
User Starts Codex
    │
    ▼
Load config.toml from ~/.codex/
    │
    ▼
Parse [model_providers.*] sections
    │
    └─ [model_providers.openai]      (built-in)
    └─ [model_providers.azure]       (user-defined)
    └─ [model_providers.anthropic]   (user-defined)
    └─ [model_providers.google-genai](user-defined)
    │
    ▼
merge with built_in_model_providers()
    │
    ├─ Start with:
    │  ├─ openai (Responses API)
    │  └─ oss (Ollama local)
    │
    ├─ Add/Override with user configs:
    │  ├─ New keys create new providers
    │  ├─ Existing keys (openai, oss) NOT overridden
    │
    ▼
Create Config struct
    │
    ├─ model_provider_id: String (e.g., "anthropic")
    ├─ model_provider: ModelProviderInfo
    │
    ▼
ModelProviderInfo Loaded
    │
    ├─ name: "Anthropic Claude"
    ├─ base_url: "https://api.anthropic.com/v1"
    ├─ env_key: "ANTHROPIC_API_KEY"
    ├─ wire_api: WireApi::Chat
    ├─ http_headers: { "anthropic-version": "2023-06-01" }
    │
    ▼
Ready for API Requests
    │
    └─ Model selection happens independently
       ├─ model: "claude-3-sonnet"  (from config.toml)
       └─ Sent in API request as "model" field
```

## Environment Variable Resolution

```
Provider Definition: env_key = "GOOGLE_API_KEY"
    │
    ▼
When building request:
    │
    ├─ ModelProviderInfo::api_key()
    │  ├─ Read env::var("GOOGLE_API_KEY")
    │  ├─ Check not empty
    │  ├─ Return value or error
    │
    ▼
HTTP Header Application:
    │
    ├─ env_http_headers = { "X-Feature": "FEATURE_FLAG_VAR" }
    │
    ├─ For each (header_name, env_var_name) pair:
    │  ├─ Try to read env::var(env_var_name)
    │  ├─ If exists and not empty:
    │      └─ Add header: header_name: env_value
    │  └─ If missing or empty:
    │      └─ Omit header (no error)
    │
    ▼
Result: Headers include dynamic values from environment
```

## Authentication Flow

```
ModelProviderInfo specifies auth:
    │
    ├─ experimental_bearer_token (direct token - discouraged)
    ├─ env_key (environment variable name)
    │
    ▼
create_request_builder():
    │
    ├─ Check experimental_bearer_token
    │  └─ If set, use directly
    │
    ├─ Else try env_key
    │  ├─ Lookup environment variable
    │  └─ If found, create CodexAuth
    │
    ├─ Else fall back to OAuth auth (if available)
    │  ├─ Check stored auth from ~/.codex/auth.json
    │  ├─ If expired, refresh token
    │
    ├─ Else check direct auth parameter
    │  └─ CodexAuth passed from auth manager
    │
    ▼
CodexAuth::get_token()
    │
    ├─ If API key mode:
    │  └─ Return stored API key
    │
    ├─ If OAuth mode:
    │  ├─ Check token expiration
    │  ├─ If expired:
    │  │  └─ POST to REFRESH_TOKEN_URL
    │  │     ├─ Send refresh_token
    │  │     ├─ Get new access_token
    │  │     └─ Update storage
    │  ├─ Return access_token
    │
    ▼
Add bearer header:
    │
    └─ Authorization: Bearer {token}
```

## Data Structure Transformations

### Prompt → Chat Completions Request

```
Prompt {
  input: [
    ResponseItem::Message { role: "user", content: "..." },
    ResponseItem::FunctionCall { name: "tool_name", ... },
    ResponseItem::FunctionCallOutput { ... }
  ],
  tools: [
    ToolSpec::Function { name: "tool_name", ... }
  ]
}
    │
    ▼
Transform:
    │
    ├─ Add system message first
    ├─ Convert ResponseItem → Chat message:
    │  ├─ Message → {"role": "user/assistant", "content": "..."}
    │  ├─ FunctionCall → {"role": "assistant", "tool_calls": [...]}
    │  └─ FunctionCallOutput → {"role": "tool", "content": "..."}
    │
    ├─ Convert ToolSpec → OpenAI format:
    │  ├─ Function tool → {"type": "function", "function": {...}}
    │
    ▼
JSON Request Body:
{
  "model": "gpt-4",
  "messages": [
    {"role": "system", "content": "instructions"},
    {"role": "user", "content": "user input"},
    {"role": "assistant", "tool_calls": [...]},
    {"role": "tool", "content": "tool result"}
  ],
  "tools": [...],
  "temperature": 0.7,
  "stream": true
}
```

### Chat Completions SSE Stream → ResponseEvent

```
SSE Event Stream:
    data: {"choices": [{"delta": {"content": "Hello"}}]}
    data: {"choices": [{"delta": {"content": " world"}}]}
    data: {"choices": [{"delta": {"tool_calls": [...]}}]}
    │
    ▼
Parser:
    │
    ├─ Extract delta from each event
    ├─ Aggregate text chunks per message
    ├─ Buffer tool calls
    │
    ▼
Aggregator:
    │
    ├─ Emit ContentBlock per chunk group
    ├─ Emit ToolCall when invocation complete
    ├─ Emit MessageEnd when message done
    │
    ▼
ResponseEvent Stream:
    │
    ├─ ContentBlock { content: "Hello" }
    ├─ ContentBlock { content: " world" }
    ├─ ToolCall { id, name: "tool_name", input: {...} }
    ├─ ToolCallResult { output: "..." }
    ├─ MessageEnd { stop_reason: "tool_calls" }
```

## Configuration Hierarchy

```
Built-in Defaults
    │
    ├─ model = "gpt-5-codex" (on Linux/macOS)
    ├─ model_provider = "openai"
    ├─ approval_policy = "on-request"
    ├─ sandbox_mode = "read-only"
    │
    ▼ (Overridden by)
    │
~/.codex/config.toml [root-level keys]
    │
    ├─ model = "claude-3"
    ├─ model_provider = "anthropic"
    │
    ▼ (Overridden by)
    │
CLI --config flags
    │
    ├─ --config model="gpt-4"
    ├─ --config model_provider="azure"
    │
    ▼ (Final Result)
    │
Active Config
    │
    ├─ model: "gpt-4" (from CLI)
    ├─ model_provider: "azure" (from CLI)
    ├─ approval_policy: "on-request" (from defaults)
    ├─ sandbox_mode: "read-only" (from defaults)
```

## Provider Detection and Selection

```
ModelProviderInfo Registry:
{
  "openai": ModelProviderInfo { ... },        // Built-in
  "oss": ModelProviderInfo { ... },           // Built-in
  "anthropic": ModelProviderInfo { ... },     // From config.toml
  "google-genai": ModelProviderInfo { ... },  // From config.toml
  "azure": ModelProviderInfo { ... },         // From config.toml
}
    │
    ▼
Config.model_provider_id = "anthropic"
    │
    ▼
Lookup in registry:
    │
    └─ providers["anthropic"] → ModelProviderInfo
        ├─ base_url: "https://api.anthropic.com/v1"
        ├─ env_key: "ANTHROPIC_API_KEY"
        ├─ wire_api: WireApi::Chat
        └─ http_headers: {"anthropic-version": "2023-06-01"}
    │
    ▼
Use for API requests:
    │
    └─ POST https://api.anthropic.com/v1/messages
       ├─ Authorization: Bearer {ANTHROPIC_API_KEY}
       ├─ Header: anthropic-version: 2023-06-01
       └─ Body: {...}
```

## Key Design Patterns

### 1. Provider Agnostic
- Core system doesn't know about specific providers
- All information is in ModelProviderInfo struct
- New providers added via config.toml only

### 2. Dual API Support
- Two wire formats: Responses and Chat Completions
- Routing based on provider.wire_api enum
- Adapter layer converts between internal format and API format

### 3. Streaming First
- All responses are streamed via SSE
- Client aggregates events into meaningful units
- Real-time UI updates as tokens arrive

### 4. Configuration Driven
- Minimal hard-coded provider info
- All endpoints, headers, auth via config
- Environment variables for secrets

### 5. Modular Authentication
- Multiple auth modes (API key, OAuth)
- Fallback chain for auth sources
- Secure storage of credentials
