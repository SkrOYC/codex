# Detailed File Reference - API Integration Files

## Critical Path Files for API Implementation

### 1. PROVIDER CONFIGURATION SYSTEM

#### File: /home/user/codex/codex-rs/core/src/model_provider_info.rs
- **Lines**: 1-533
- **Key Functions**:
  - `built_in_model_providers()` (line 266-319): Returns default providers
  - `create_oss_provider()` (line 321-340): Creates Ollama provider
  - `create_oss_provider_with_base_url()` (line 342-358): Helper for OSS
  - `ModelProviderInfo::create_request_builder()` (line 107-137): Builds HTTP requests
  - `ModelProviderInfo::get_full_url()` (line 152-174): Constructs endpoint URLs
  - `ModelProviderInfo::is_azure_responses_endpoint()` (line 176-189): Azure detection
  - `ModelProviderInfo::apply_http_headers()` (line 194-211): Header application
  - `ModelProviderInfo::api_key()` (line 216-237): Environment variable handling

- **Key Data Structures**:
  - `WireApi` enum (line 33-42): Responses | Chat
  - `ModelProviderInfo` struct (line 45-96): Complete provider definition

### 2. HTTP CLIENT LAYER

#### File: /home/user/codex/codex-rs/core/src/default_client.rs
- **Lines**: 1-376
- **Key Functions**:
  - `create_client()` (line 260-277): Creates HTTP client with defaults
  - `get_codex_user_agent()` (line 202-225): Generates User-Agent
  - `originator()` (line 198-200): Gets request originator

- **Key Structures**:
  - `CodexHttpClient` (line 35-65): Main client wrapper
  - `CodexRequestBuilder` (line 69-156): Request builder with fluent API
  - `Originator` (line 157-161): Request origin metadata

- **Features**:
  - Bearer auth support (line 102-107)
  - JSON body support (line 109-114)
  - Request logging with ID extraction (line 145-155)

### 3. MODEL CLIENT (Main API Interface)

#### File: /home/user/codex/codex-rs/core/src/client.rs
- **Lines**: 1-58KB
- **Key Methods**:
  - `ModelClient::new()` (line 96-119): Constructor
  - `ModelClient::stream()` (line 143-184): Dispatches to appropriate API
  - `ModelClient::stream_responses()` (line 187-400+): Responses API implementation
  - `ModelClient::stream_chat_completions()` (inherited, via streaming): Chat API

- **Key Structures**:
  - `ModelClient` (line 82-92): Main client struct
  - `ErrorResponse` (line 65-79): Error parsing
  
- **Important Constants**:
  - Response stream configuration with idle timeout, retry settings

### 4. REQUEST/RESPONSE STRUCTURES

#### File: /home/user/codex/codex-rs/core/src/client_common.rs
- **Lines**: 1-200+
- **Key Structures**:
  - `Prompt` (line 32-49): API request payload definition
    - `input`: Conversation items
    - `tools`: Available tools
    - `parallel_tool_calls`: Tool call settings
    - `output_schema`: Structured output schema
    - `base_instructions_override`: Custom instructions
  
  - `ResponsesApiRequest<'a>` (line ~200-220): Responses API request
    - `model`, `instructions`, `input`
    - `tools`: Tool definitions
    - `reasoning`: Reasoning configuration
    - `store`, `stream`: Behavior flags
    - `include`: Fields to include in response

  - `ResponseEvent`: Streaming event type
  - `ResponseStream`: Stream wrapper for events

- **Key Functions**:
  - `Prompt::get_full_instructions()` (line 51-74): Build system instructions
  - `Prompt::get_formatted_input()` (line 76-92): Format input for API

### 5. CHAT COMPLETIONS IMPLEMENTATION

#### File: /home/user/codex/codex-rs/core/src/chat_completions.rs
- **Lines**: 1-40KB
- **Key Function**:
  - `stream_chat_completions()` (line 40-47): Main Chat Completions implementation
    - Message building (line 54-200+)
    - Tool transformation
    - Reasoning block handling
    - Stream response parsing

- **Response Format**:
  - SSE stream parsing with `eventsource_stream`
  - Delta aggregation for message chunks
  - Tool call extraction and formatting

### 6. CONFIGURATION SYSTEM

#### File: /home/user/codex/codex-rs/core/src/config/mod.rs
- **Lines**: 1-122KB
- **Key Structures**:
  - `Config` (line 77-200+): Main configuration struct
    - `model`: Selected model
    - `model_provider_id`: Provider selector
    - `model_provider`: Provider instance
    - All model-specific settings

- **Key Functions**:
  - Configuration loading from multiple sources
  - Provider loading from config.toml
  - Merge logic for overrides

#### File: /home/user/codex/codex-rs/core/src/config/types.rs
- **Lines**: 1-24KB
- **Configuration Types**:
  - `ModelProviderInfo` definition
  - API endpoint specifications
  - Retry/timeout settings
  - Header configurations

### 7. AUTHENTICATION SYSTEM

#### File: /home/user/codex/codex-rs/core/src/auth.rs
- **Lines**: 1-42KB
- **Key Structures**:
  - `CodexAuth` (line 39-46): Authentication holder
    - `mode`: AuthMode (ChatGPT or API)
    - `api_key`: Direct API key
    - `auth_dot_json`: Persistent auth data
    - `storage`: Backend storage

- **Key Methods**:
  - `CodexAuth::refresh_token()` (line 96-150+): Token refresh logic
  - `CodexAuth::get_token()`: Retrieve current token

- **Key Constants**:
  - `REFRESH_TOKEN_URL` (line 62): OAuth endpoint
  - `TOKEN_REFRESH_INTERVAL` (line 55): Refresh timing

#### File: /home/user/codex/codex-rs/core/src/auth/storage.rs
- **Authentication Storage**:
  - `AuthCredentialsStoreMode`: File | Keyring | Auto
  - `AuthDotJson`: Persistent auth structure
  - Token storage location: `~/.codex/auth.json`

### 8. PROVIDER IMPLEMENTATIONS

#### ChatGPT Integration
**File**: /home/user/codex/codex-rs/chatgpt/src/chatgpt_client.rs
- Direct ChatGPT backend API integration
- Bearer token auth with chatgpt-account-id header
- Task fetching and command application

#### Ollama Integration
**File**: /home/user/codex/codex-rs/ollama/src/client.rs
- Local Ollama server detection and health checking
- OpenAI-compatible API support
- Default port: 11434

### 9. CONFIGURATION FILES

#### Main Configuration
**File**: `/home/user/codex/docs/config.md` (500+ lines)
- Complete configuration reference
- Provider configuration examples
- All configuration options documented

#### Example Configuration
**File**: `/home/user/codex/docs/example-config.md`
- Starting template for users
- All sections with defaults

#### Old Config Reference
**File**: `/home/user/codex/codex-rs/config.md`
- Redirects to /docs/config.md

### 10. PROTOCOL TYPES

#### File: /home/user/codex/codex-rs/protocol/src/config_types.rs
- **Enums**:
  - `ReasoningEffort`: Control reasoning levels
  - `ReasoningSummary`: Reasoning summary format
  - `Verbosity`: Output detail level
  - `WireApi`: API protocol selection
  - `SandboxMode`: Execution environment
  - `ForcedLoginMethod`: Auth method

#### File: /home/user/codex/codex-rs/protocol/src/models.rs
- **Message Types**:
  - `ResponseItem`: Union of all message types
  - Message, FunctionCall, Tool Call, Reasoning blocks
  - Request/response item definitions

### 11. TYPE DEFINITIONS

#### File: /home/user/codex/codex-rs/core/src/model_family.rs
- `ModelFamily` enum: Classification of models
- Model capabilities (reasoning, verbosity, context window)
- Model family specific settings

## Direct Implementation Paths

### To Add Google GenAI Provider:

1. Create config entry: `~/.codex/config.toml`
   ```toml
   [model_providers.google-genai]
   name = "Google Generative AI"
   base_url = "https://generativelanguage.googleapis.com/v1beta"
   env_key = "GOOGLE_API_KEY"
   wire_api = "chat"  # Or implement adapter for Responses API
   http_headers = { "X-User-Agent" = "codex" }
   ```

2. API Request Format: Use Chat Completions adapter or implement new wire_api type

3. Key Files to Reference:
   - `/home/user/codex/codex-rs/core/src/model_provider_info.rs` - Provider structure
   - `/home/user/codex/codex-rs/core/src/chat_completions.rs` - Chat API implementation
   - `/home/user/codex/codex-rs/core/src/client.rs` - Client routing logic

### To Add Anthropic Claude Provider:

1. Create config entry: `~/.codex/config.toml`
   ```toml
   [model_providers.anthropic]
   name = "Anthropic Claude"
   base_url = "https://api.anthropic.com/v1"
   env_key = "ANTHROPIC_API_KEY"
   wire_api = "chat"
   http_headers = { "anthropic-version" = "2023-06-01" }
   ```

2. Key Considerations:
   - Anthropic uses different header format (X-API-Key or Authorization header)
   - Messages format differs slightly from OpenAI
   - Tool use format differs from OpenAI function calling

3. Key Files to Reference:
   - `/home/user/codex/codex-rs/core/src/default_client.rs` - Header handling
   - `/home/user/codex/codex-rs/core/src/chat_completions.rs` - Message formatting
   - `/home/user/codex/codex-rs/core/src/client_common.rs` - Request structures

## Implementation Pattern

The codebase uses this pattern:
1. **Provider Definition** (ModelProviderInfo) specifies endpoint and auth
2. **HTTP Client** (CodexHttpClient) handles transport
3. **Request Mapping** converts Prompt to API format
4. **Response Parsing** converts API responses to ResponseEvent stream
5. **Configuration** allows users to define new providers in config.toml

For new providers like Google GenAI and Anthropic, this means:
- If they support OpenAI-compatible Chat Completions API, minimal changes needed
- If they have unique formats, may need to add new wire_api type or adapter
- All configuration-driven through config.toml without code changes
