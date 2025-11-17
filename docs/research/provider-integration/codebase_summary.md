# Codex Codebase Structure - API Integration Analysis

## Repository Overview
- **Main Codebase**: `/home/user/codex/codex-rs` (Rust-based backend)
- **TypeScript SDK**: `/home/user/codex/sdk/typescript`
- **CLI Application**: `/home/user/codex/codex-cli`
- **Documentation**: `/home/user/codex/docs`

## Current Architecture

### 1. Provider Integration System

#### Core Files
- **`/home/user/codex/codex-rs/core/src/model_provider_info.rs`** (370 lines)
  - Main provider definition structure: `ModelProviderInfo`
  - Supports two wire APIs: `Responses` (OpenAI Responses API) and `Chat` (OpenAI Chat Completions)
  - Handles authentication, headers, query parameters, retry configuration

#### Key Structures

```rust
pub struct ModelProviderInfo {
    pub name: String,                                    // Display name
    pub base_url: Option<String>,                        // API endpoint base URL
    pub env_key: Option<String>,                         // Environment variable for API key
    pub env_key_instructions: Option<String>,            // User-facing instructions
    pub experimental_bearer_token: Option<String>,       // Direct token (discouraged)
    pub wire_api: WireApi,                              // "responses" or "chat"
    pub query_params: Option<HashMap<String, String>>,   // Query parameters
    pub http_headers: Option<HashMap<String, String>>,   // Static headers
    pub env_http_headers: Option<HashMap<String, String>>, // Headers from env vars
    pub request_max_retries: Option<u64>,               // HTTP retry config
    pub stream_max_retries: Option<u64>,                // Stream retry config
    pub stream_idle_timeout_ms: Option<u64>,            // Idle timeout in ms
    pub requires_openai_auth: bool,                     // OAuth requirement flag
}
```

#### Built-in Providers
```rust
built_in_model_providers() -> HashMap<String, ModelProviderInfo>
```

Current built-in providers:
- **openai**: Uses Responses API at `https://api.openai.com/v1/responses`
- **oss**: Local open-source provider (defaults to Ollama at `localhost:11434`)

### 2. API Client Architecture

#### HTTP Client Implementation
**File**: `/home/user/codex/codex-rs/core/src/default_client.rs`

```rust
pub struct CodexHttpClient {
    inner: reqwest::Client,
}

pub struct CodexRequestBuilder {
    builder: reqwest::RequestBuilder,
    method: Method,
    url: String,
}
```

Features:
- Custom headers: `originator`, `User-Agent`
- Bearer token authentication support
- Request logging and ID extraction (cf-ray, x-request-id, x-oai-request-id)
- Automatic User-Agent generation with version info

#### Model Client
**File**: `/home/user/codex/codex-rs/core/src/client.rs` (58KB)

```rust
pub struct ModelClient {
    config: Arc<Config>,
    auth_manager: Option<Arc<AuthManager>>,
    otel_event_manager: OtelEventManager,
    client: CodexHttpClient,
    provider: ModelProviderInfo,
    conversation_id: ConversationId,
    effort: Option<ReasoningEffortConfig>,
    summary: ReasoningSummaryConfig,
    session_source: SessionSource,
}
```

Methods:
- `stream()`: Main method for streaming responses
- `stream_responses()`: Implements OpenAI Responses API
- `stream_chat_completions()`: Implements Chat Completions API

### 3. Request/Response Protocol

#### Request Structures
**File**: `/home/user/codex/codex-rs/core/src/client_common.rs`

```rust
pub(crate) struct ResponsesApiRequest<'a> {
    pub(crate) model: &'a str,
    pub(crate) instructions: &'a str,
    pub(crate) input: &'a Vec<ResponseItem>,
    pub(crate) tools: &'a [serde_json::Value],
    pub(crate) tool_choice: &'static str,
    pub(crate) parallel_tool_calls: bool,
    pub(crate) reasoning: Option<Reasoning>,
    pub(crate) store: bool,
    pub(crate) stream: bool,
    pub(crate) include: Vec<String>,
    pub(crate) prompt_cache_key: Option<String>,
    pub(crate) text: Option<TextControls>,
}
```

#### Response Handling
- Streams responses via Server-Sent Events (SSE)
- Parses `eventsource_stream` for real-time data
- Supports reasoning blocks, tool calls, and structured output

### 4. Configuration System

#### Configuration Loading
**File**: `/home/user/codex/codex-rs/core/src/config/mod.rs` (120KB+)

```rust
pub struct Config {
    pub model: String,
    pub review_model: String,
    pub model_family: ModelFamily,
    pub model_context_window: Option<i64>,
    pub model_max_output_tokens: Option<i64>,
    pub model_auto_compact_token_limit: Option<i64>,
    pub model_provider_id: String,
    pub model_provider: ModelProviderInfo,
    // ... many more fields
}
```

#### Configuration Sources (Priority Order)
1. Command-line flags (`--config key=value`)
2. `config.toml` file in `$CODEX_HOME/` (default: `~/.codex/`)
3. Built-in defaults

#### Environment Variable Usage
- **`CODEX_HOME`**: Base directory for config (~/.codex default)
- **`OPENAI_BASE_URL`**: Override OpenAI endpoint
- **`CODEX_OSS_BASE_URL`**: Override OSS provider base URL
- **`CODEX_OSS_PORT`**: Override OSS provider port
- **`OPENAI_API_KEY`**: OpenAI API credentials
- **`OPENAI_ORGANIZATION`**: OpenAI org ID
- **`OPENAI_PROJECT`**: OpenAI project ID

### 5. Authentication System

#### Core Auth Module
**File**: `/home/user/codex/codex-rs/core/src/auth.rs` (42KB)

```rust
pub struct CodexAuth {
    pub mode: AuthMode,                         // ChatGPT or API
    pub(crate) api_key: Option<String>,
    pub(crate) auth_dot_json: Arc<Mutex<Option<AuthDotJson>>>,
    storage: Arc<dyn AuthStorageBackend>,
    pub(crate) client: CodexHttpClient,
}

impl CodexAuth {
    pub async fn refresh_token(&self) -> Result<String, RefreshTokenError>
    pub async fn get_token(&self) -> Result<String>
}
```

#### OAuth Configuration
- **Refresh Token URL**: `https://auth.openai.com/oauth/token`
- Override via env var: `CODEX_REFRESH_TOKEN_URL_OVERRIDE`
- Support for token storage via file or system keyring

#### Auth Storage Modes
```rust
pub enum AuthCredentialsStoreMode {
    File,      // Default: ~/.codex/auth.json
    Keyring,   // OS native keyring
    Auto,      // Auto-detect best option
}
```

### 6. Provider Integration Examples

#### ChatGPT Provider
**File**: `/home/user/codex/codex-rs/chatgpt/src/chatgpt_client.rs`

```rust
pub(crate) async fn chatgpt_get_request<T: DeserializeOwned>(
    config: &Config,
    path: String,
) -> anyhow::Result<T> {
    let client = create_client();
    let url = format!("{base_url}{path}");
    
    let response = client
        .get(&url)
        .bearer_auth(&token.access_token)
        .header("chatgpt-account-id", account_id?)
        .header("Content-Type", "application/json")
        .send()
        .await?
}
```

#### Ollama Provider
**File**: `/home/user/codex/codex-rs/ollama/src/client.rs`

```rust
pub struct OllamaClient {
    client: reqwest::Client,
    host_root: String,
    uses_openai_compat: bool,
}

impl OllamaClient {
    pub async fn try_from_oss_provider(config: &Config) -> io::Result<Self>
}
```

### 7. Configuration Examples

#### Basic Provider Configuration
**Location**: `~/.codex/config.toml`

```toml
# Select model and provider
model = "gpt-4o"
model_provider = "custom-provider"

# Define custom provider
[model_providers.custom-provider]
name = "Custom API Provider"
base_url = "https://api.example.com/v1"
env_key = "CUSTOM_API_KEY"
wire_api = "chat"
query_params = { key = "value" }
http_headers = { "X-Custom-Header" = "value" }
env_http_headers = { "X-Auth" = "AUTH_ENV_VAR" }
request_max_retries = 4
stream_max_retries = 5
stream_idle_timeout_ms = 300000
```

#### Azure Provider Example
```toml
[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://YOUR_PROJECT.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
wire_api = "responses"
query_params = { api-version = "2025-04-01-preview" }
```

### 8. Request/Response Flow

#### Responses API Flow
1. **Request Construction** (client.rs:187-249)
   - Build instructions from Prompt
   - Create tools JSON
   - Set reasoning parameters (effort, summary)
   - Build ResponsesApiRequest payload

2. **Request Sending** (model_provider_info.rs:107-137)
   - Apply provider-specific headers
   - Add bearer token auth
   - Create POST request to full URL

3. **Response Streaming**
   - Receive Server-Sent Events (SSE)
   - Parse events from stream
   - Aggregate responses per turn
   - Extract reasoning blocks, tool calls

#### Chat Completions Flow
1. **Message Building** (chat_completions.rs:40-200+)
   - System instructions as first message
   - Conversation history items
   - Tool definitions
   - Reasoning blocks attached to assistant messages

2. **Request Format**
   - POST to `{base_url}/chat/completions`
   - JSON body with messages array
   - Tools in OpenAI function calling format

### 9. Core Dependencies

**File**: `/home/user/codex/codex-rs/core/Cargo.toml`

```toml
# HTTP & Async
reqwest = { features = ["json", "stream"] }  # HTTP client
tokio = { features = ["io-std", "macros", "rt-multi-thread"] }
futures = {}
async-trait = {}

# Data Formats
serde = { features = ["derive"] }
serde_json = {}
toml = {}
toml_edit = {}

# Streaming
eventsource-stream = {}  # SSE parsing

# Auth
chrono = { features = ["serde"] }
```

### 10. Wire API Definitions

#### Responses API (OpenAI Modern)
- **Endpoint**: `/v1/responses`
- **Request Format**: ResponsesApiRequest struct
- **Features**:
  - Structured output via JSON schema
  - Built-in reasoning capabilities
  - Streaming support
  - Tool use with parallel calls

#### Chat Completions API (Compatible)
- **Endpoint**: `/v1/chat/completions`
- **Request Format**: Messages array with roles
- **Features**:
  - Broader compatibility
  - Function calling (tool use)
  - Streaming via SSE
  - Less feature-rich than Responses API

### 11. Type Definitions & Enums

**File**: `/home/user/codex/codex-rs/protocol/src/config_types.rs`

```rust
#[derive(Serialize, Deserialize)]
pub enum ReasoningEffort {
    None, Minimal, Low, Medium, High
}

#[derive(Serialize, Deserialize)]
pub enum ReasoningSummary {
    Auto, Concise, Detailed, None
}

#[derive(Serialize, Deserialize)]
pub enum Verbosity {
    Low, Medium, High
}

#[derive(Serialize, Deserialize)]
pub enum WireApi {
    Responses,
    #[default]
    Chat,
}
```

## Key Implementation Details

### Provider Registration
- Built-in providers defined in `built_in_model_providers()`
- User-defined providers merged from `config.toml` `[model_providers.*]` sections
- New providers don't override built-ins (by design)

### URL Construction
```rust
fn get_full_url(&self, auth: &Option<CodexAuth>) -> String {
    let base_url = self.base_url.clone()
        .unwrap_or(default_base_url);
    
    match self.wire_api {
        WireApi::Responses => format!("{base_url}/responses{query_string}"),
        WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
    }
}
```

### Header Application
- Static headers from `http_headers` HashMap
- Dynamic headers from environment variables (from `env_http_headers`)
- Provider-specific processing for Azure Responses endpoints

### Authentication Priority
1. Direct token from `experimental_bearer_token` (if set)
2. API key from environment variable (from `env_key`)
3. OAuth token from auth storage
4. No authentication (for public APIs)

## Summary of Key Files

| File Path | Size | Purpose |
|-----------|------|---------|
| `core/src/model_provider_info.rs` | 533 lines | Provider definition structure and methods |
| `core/src/client.rs` | 58KB | Main ModelClient for API calls |
| `core/src/client_common.rs` | 18KB | Request/response structures |
| `core/src/chat_completions.rs` | 40KB | Chat Completions API implementation |
| `core/src/default_client.rs` | 11KB | HTTP client wrapper |
| `core/src/config/mod.rs` | 122KB | Configuration loading and merging |
| `core/src/auth.rs` | 42KB | Authentication and token management |
| `chatgpt/src/chatgpt_client.rs` | Small | ChatGPT provider integration |
| `ollama/src/client.rs` | ~100 lines | Ollama provider integration |
| `docs/config.md` | Large | Configuration documentation |

## External Integration Patterns

### Adding a New Provider
1. Create provider entry in `~/.codex/config.toml` under `[model_providers.PROVIDER_ID]`
2. Specify `base_url`, `env_key`, `wire_api`, and other options
3. Set environment variables for API keys
4. Use `--model-provider PROVIDER_ID` to select

### Direct API Implementation
1. Use `CodexHttpClient` for HTTP operations
2. Implement bearer token authentication via `env_key` or `experimental_bearer_token`
3. Support both Responses and Chat Completions wire formats
4. Include custom headers via `http_headers` or `env_http_headers`

## Next Steps for Google GenAI & Anthropic Integration

Based on this analysis, to implement direct API access for Google GenAI and Anthropic:

1. **Define Provider Configurations** in ModelProviderInfo:
   - Google GenAI: Base URL, API key environment variable
   - Anthropic Claude: Base URL, API key environment variable

2. **Create Wire API Adapters** (if needed):
   - Map Google GenAI API format to Responses or Chat Completions
   - Map Anthropic API format similarly

3. **Implement Request/Response Mapping**:
   - Convert Prompt structure to provider-specific format
   - Parse provider-specific responses back to ResponseEvent

4. **Configuration Examples**:
   - Add TOML examples in docs
   - Update config.md with provider examples

5. **Testing**:
   - Unit tests for request mapping
   - Integration tests against mock endpoints
