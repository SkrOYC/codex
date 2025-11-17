//! Registry of model providers supported by Codex.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Codex works out-of-the-box.
//!   2. User-defined entries inside `~/.codex/config.toml` under the `model_providers`
//!      key. These override or extend the defaults at runtime.

use crate::CodexAuth;
use crate::default_client::CodexHttpClient;
use crate::default_client::CodexRequestBuilder;
use codex_app_server_protocol::AuthMode;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::env::VarError;
use std::time::Duration;

use crate::error::EnvVarError;
const DEFAULT_STREAM_IDLE_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_STREAM_MAX_RETRIES: u64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: u64 = 4;
/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: u64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: u64 = 100;

/// Wire protocol that the provider speaks. Most third-party services only
/// implement the classic OpenAI Chat Completions JSON schema, whereas OpenAI
/// itself (and a handful of others) additionally expose the more modern
/// *Responses* API. The two protocols use different request/response shapes
/// and *cannot* be auto-detected at runtime, therefore each provider entry
/// must declare which one it expects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    Responses,

    /// Regular Chat Completions compatible with `/v1/chat/completions`.
    #[default]
    Chat,

    /// Google Generative Language API (Gemini) at `/v1beta/models/{model}:streamGenerateContent`.
    #[serde(rename = "google_genai")]
    GoogleGenAI,

    /// Anthropic Messages API (Claude) at `/v1/messages`.
    #[serde(rename = "anthropic_messages")]
    AnthropicMessages,
}

/// Serializable representation of a provider definition.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelProviderInfo {
    /// Friendly display name.
    pub name: String,
    /// Base URL for the provider's OpenAI-compatible API.
    pub base_url: Option<String>,
    /// Environment variable that stores the user's API key for this provider.
    pub env_key: Option<String>,

    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub env_key_instructions: Option<String>,

    /// Value to use with `Authorization: Bearer <token>` header. Use of this
    /// config is discouraged in favor of `env_key` for security reasons, but
    /// this may be necessary when using this programmatically.
    pub experimental_bearer_token: Option<String>,

    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,

    /// Optional query parameters to append to the base URL.
    pub query_params: Option<HashMap<String, String>>,

    /// Additional HTTP headers to include in requests to this provider where
    /// the (key, value) pairs are the header name and value.
    pub http_headers: Option<HashMap<String, String>>,

    /// Optional HTTP headers to include in requests to this provider where the
    /// (key, value) pairs are the header name and _environment variable_ whose
    /// value should be used. If the environment variable is not set, or the
    /// value is empty, the header will not be included in the request.
    pub env_http_headers: Option<HashMap<String, String>>,

    /// Maximum number of times to retry a failed HTTP request to this provider.
    pub request_max_retries: Option<u64>,

    /// Number of times to retry reconnecting a dropped streaming response before failing.
    pub stream_max_retries: Option<u64>,

    /// Idle timeout (in milliseconds) to wait for activity on a streaming response before treating
    /// the connection as lost.
    pub stream_idle_timeout_ms: Option<u64>,

    /// Does this provider require an OpenAI API Key or ChatGPT login token? If true,
    /// user is presented with login screen on first run, and login preference and token/key
    /// are stored in auth.json. If false (which is the default), login screen is skipped,
    /// and API key (if needed) comes from the "env_key" environment variable.
    #[serde(default)]
    pub requires_openai_auth: bool,
}

impl ModelProviderInfo {
    /// Construct a `POST` RequestBuilder for the given URL using the provided
    /// [`CodexHttpClient`] applying:
    ///   • provider-specific headers (static + env based)
    ///   • Bearer auth header when an API key is available.
    ///   • Auth token for OAuth.
    ///
    /// If the provider declares an `env_key` but the variable is missing/empty, returns an [`Err`] identical to the
    /// one produced by [`ModelProviderInfo::api_key`].
    pub async fn create_request_builder<'a>(
        &'a self,
        client: &'a CodexHttpClient,
        auth: &Option<CodexAuth>,
    ) -> crate::error::Result<CodexRequestBuilder> {
        let effective_auth = if let Some(secret_key) = &self.experimental_bearer_token {
            Some(CodexAuth::from_api_key(secret_key))
        } else {
            match self.api_key() {
                Ok(Some(key)) => Some(CodexAuth::from_api_key(&key)),
                Ok(None) => auth.clone(),
                Err(err) => {
                    if auth.is_some() {
                        auth.clone()
                    } else {
                        return Err(err);
                    }
                }
            }
        };

        let url = self.get_full_url(&effective_auth);

        let mut builder = client.post(url);

        if let Some(auth) = effective_auth.as_ref() {
            builder = builder.bearer_auth(auth.get_token().await?);
        }

        Ok(self.apply_http_headers(builder))
    }

    fn get_query_string(&self) -> String {
        self.query_params
            .as_ref()
            .map_or_else(String::new, |params| {
                let full_params = params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("&");
                format!("?{full_params}")
            })
    }

    pub(crate) fn get_full_url(&self, auth: &Option<CodexAuth>) -> String {
        let default_base_url = if matches!(
            auth,
            Some(CodexAuth {
                mode: AuthMode::ChatGPT,
                ..
            })
        ) {
            "https://chatgpt.com/backend-api/codex"
        } else {
            "https://api.openai.com/v1"
        };
        let query_string = self.get_query_string();
        let base_url = self
            .base_url
            .clone()
            .unwrap_or(default_base_url.to_string());

        match self.wire_api {
            WireApi::Responses => format!("{base_url}/responses{query_string}"),
            WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
            WireApi::GoogleGenAI => {
                // Note: Google GenAI requires the model name in the URL path.
                // This placeholder will need to be replaced with the actual model name
                // when the full implementation is added.
                format!("{base_url}/models/{{model}}:streamGenerateContent{query_string}")
            }
            WireApi::AnthropicMessages => format!("{base_url}/messages{query_string}"),
        }
    }

    pub(crate) fn is_azure_responses_endpoint(&self) -> bool {
        if self.wire_api != WireApi::Responses {
            return false;
        }

        if self.name.eq_ignore_ascii_case("azure") {
            return true;
        }

        self.base_url
            .as_ref()
            .map(|base| matches_azure_responses_base_url(base))
            .unwrap_or(false)
    }

    /// Apply provider-specific HTTP headers (both static and environment-based)
    /// onto an existing [`CodexRequestBuilder`] and return the updated
    /// builder.
    fn apply_http_headers(&self, mut builder: CodexRequestBuilder) -> CodexRequestBuilder {
        if let Some(extra) = &self.http_headers {
            for (k, v) in extra {
                builder = builder.header(k, v);
            }
        }

        if let Some(env_headers) = &self.env_http_headers {
            for (header, env_var) in env_headers {
                if let Ok(val) = std::env::var(env_var)
                    && !val.trim().is_empty()
                {
                    builder = builder.header(header, val);
                }
            }
        }
        builder
    }

    /// If `env_key` is Some, returns the API key for this provider if present
    /// (and non-empty) in the environment. If `env_key` is required but
    /// cannot be found, returns an error.
    pub fn api_key(&self) -> crate::error::Result<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let env_value = std::env::var(env_key);
                env_value
                    .and_then(|v| {
                        if v.trim().is_empty() {
                            Err(VarError::NotPresent)
                        } else {
                            Ok(Some(v))
                        }
                    })
                    .map_err(|_| {
                        crate::error::CodexErr::EnvVar(EnvVarError {
                            var: env_key.clone(),
                            instructions: self.env_key_instructions.clone(),
                        })
                    })
            }
            None => Ok(None),
        }
    }

    /// Effective maximum number of request retries for this provider.
    pub fn request_max_retries(&self) -> u64 {
        self.request_max_retries
            .unwrap_or(DEFAULT_REQUEST_MAX_RETRIES)
            .min(MAX_REQUEST_MAX_RETRIES)
    }

    /// Effective maximum number of stream reconnection attempts for this provider.
    pub fn stream_max_retries(&self) -> u64 {
        self.stream_max_retries
            .unwrap_or(DEFAULT_STREAM_MAX_RETRIES)
            .min(MAX_STREAM_MAX_RETRIES)
    }

    /// Effective idle timeout for streaming responses.
    pub fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(DEFAULT_STREAM_IDLE_TIMEOUT_MS))
    }
}

const DEFAULT_OLLAMA_PORT: u32 = 11434;

pub const BUILT_IN_OSS_MODEL_PROVIDER_ID: &str = "oss";
pub const BUILT_IN_GOOGLE_GENAI_MODEL_PROVIDER_ID: &str = "google_genai";
pub const BUILT_IN_ANTHROPIC_MODEL_PROVIDER_ID: &str = "anthropic";

/// Built-in default provider list.
pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    use ModelProviderInfo as P;

    // We include OpenAI, OSS (Ollama), Google GenAI, and Anthropic as built-in providers
    // to provide first-class support for major LLM providers. Users can add additional
    // providers in `model_providers` in config.toml.
    [
        (
            "openai",
            P {
                name: "OpenAI".into(),
                // Allow users to override the default OpenAI endpoint by
                // exporting `OPENAI_BASE_URL`. This is useful when pointing
                // Codex at a proxy, mock server, or Azure-style deployment
                // without requiring a full TOML override for the built-in
                // OpenAI provider.
                base_url: std::env::var("OPENAI_BASE_URL")
                    .ok()
                    .filter(|v| !v.trim().is_empty()),
                env_key: None,
                env_key_instructions: None,
                experimental_bearer_token: None,
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: Some(
                    [("version".to_string(), env!("CARGO_PKG_VERSION").to_string())]
                        .into_iter()
                        .collect(),
                ),
                env_http_headers: Some(
                    [
                        (
                            "OpenAI-Organization".to_string(),
                            "OPENAI_ORGANIZATION".to_string(),
                        ),
                        ("OpenAI-Project".to_string(), "OPENAI_PROJECT".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
                // Use global defaults for retry/timeout unless overridden in config.toml.
                request_max_retries: None,
                stream_max_retries: None,
                stream_idle_timeout_ms: None,
                requires_openai_auth: true,
            },
        ),
        (BUILT_IN_OSS_MODEL_PROVIDER_ID, create_oss_provider()),
        (
            BUILT_IN_GOOGLE_GENAI_MODEL_PROVIDER_ID,
            create_google_genai_provider(),
        ),
        (
            BUILT_IN_ANTHROPIC_MODEL_PROVIDER_ID,
            create_anthropic_provider(),
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub fn create_oss_provider() -> ModelProviderInfo {
    // These CODEX_OSS_ environment variables are experimental: we may
    // switch to reading values from config.toml instead.
    let codex_oss_base_url = match std::env::var("CODEX_OSS_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(url) => url,
        None => format!(
            "http://localhost:{port}/v1",
            port = std::env::var("CODEX_OSS_PORT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(DEFAULT_OLLAMA_PORT)
        ),
    };

    create_oss_provider_with_base_url(&codex_oss_base_url)
}

pub fn create_oss_provider_with_base_url(base_url: &str) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "gpt-oss".into(),
        base_url: Some(base_url.into()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

/// Creates a Google GenAI provider configuration.
///
/// Google GenAI uses the Generative Language API with Gemini models.
/// Authentication is via API key passed in the `x-goog-api-key` header.
///
/// Environment variables:
/// - `GOOGLE_GENAI_API_KEY`: Required API key for authentication
/// - `GOOGLE_GENAI_BASE_URL`: Optional base URL override (defaults to generativelanguage.googleapis.com)
pub fn create_google_genai_provider() -> ModelProviderInfo {
    ModelProviderInfo {
        name: "Google GenAI".into(),
        base_url: std::env::var("GOOGLE_GENAI_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| Some("https://generativelanguage.googleapis.com/v1beta".to_string())),
        env_key: Some("GOOGLE_GENAI_API_KEY".into()),
        env_key_instructions: Some(
            "Get your API key from https://aistudio.google.com/app/apikey".into(),
        ),
        experimental_bearer_token: None,
        wire_api: WireApi::GoogleGenAI,
        query_params: None,
        http_headers: None,
        env_http_headers: Some(
            [(
                "x-goog-api-key".to_string(),
                "GOOGLE_GENAI_API_KEY".to_string(),
            )]
            .into_iter()
            .collect(),
        ),
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

/// Creates an Anthropic provider configuration.
///
/// Anthropic uses the Messages API with Claude models.
/// Authentication is via API key passed in the `x-api-key` header.
/// The API also requires an `anthropic-version` header for API versioning.
///
/// Environment variables:
/// - `ANTHROPIC_API_KEY`: Required API key for authentication
/// - `ANTHROPIC_BASE_URL`: Optional base URL override (defaults to api.anthropic.com)
pub fn create_anthropic_provider() -> ModelProviderInfo {
    ModelProviderInfo {
        name: "Anthropic".into(),
        base_url: std::env::var("ANTHROPIC_BASE_URL")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .or_else(|| Some("https://api.anthropic.com/v1".to_string())),
        env_key: Some("ANTHROPIC_API_KEY".into()),
        env_key_instructions: Some(
            "Get your API key from https://console.anthropic.com/settings/keys".into(),
        ),
        experimental_bearer_token: None,
        wire_api: WireApi::AnthropicMessages,
        query_params: None,
        http_headers: Some(
            [("anthropic-version".to_string(), "2023-06-01".to_string())]
                .into_iter()
                .collect(),
        ),
        env_http_headers: Some(
            [("x-api-key".to_string(), "ANTHROPIC_API_KEY".to_string())]
                .into_iter()
                .collect(),
        ),
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

fn matches_azure_responses_base_url(base_url: &str) -> bool {
    let base = base_url.to_ascii_lowercase();
    const AZURE_MARKERS: [&str; 5] = [
        "openai.azure.",
        "cognitiveservices.azure.",
        "aoai.azure.",
        "azure-api.",
        "azurefd.",
    ];
    AZURE_MARKERS.iter().any(|marker| base.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_deserialize_ollama_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Ollama"
base_url = "http://localhost:11434/v1"
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Ollama".into(),
            base_url: Some("http://localhost:11434/v1".into()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_deserialize_azure_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Azure"
base_url = "https://xxxxx.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
query_params = { api-version = "2025-04-01-preview" }
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Azure".into(),
            base_url: Some("https://xxxxx.openai.azure.com/openai".into()),
            env_key: Some("AZURE_OPENAI_API_KEY".into()),
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: Some(maplit::hashmap! {
                "api-version".to_string() => "2025-04-01-preview".to_string(),
            }),
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_deserialize_example_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Example"
base_url = "https://example.com"
env_key = "API_KEY"
http_headers = { "X-Example-Header" = "example-value" }
env_http_headers = { "X-Example-Env-Header" = "EXAMPLE_ENV_VAR" }
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Example".into(),
            base_url: Some("https://example.com".into()),
            env_key: Some("API_KEY".into()),
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: Some(maplit::hashmap! {
                "X-Example-Header".to_string() => "example-value".to_string(),
            }),
            env_http_headers: Some(maplit::hashmap! {
                "X-Example-Env-Header".to_string() => "EXAMPLE_ENV_VAR".to_string(),
            }),
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn detects_azure_responses_base_urls() {
        fn provider_for(base_url: &str) -> ModelProviderInfo {
            ModelProviderInfo {
                name: "test".into(),
                base_url: Some(base_url.into()),
                env_key: None,
                env_key_instructions: None,
                experimental_bearer_token: None,
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: None,
                env_http_headers: None,
                request_max_retries: None,
                stream_max_retries: None,
                stream_idle_timeout_ms: None,
                requires_openai_auth: false,
            }
        }

        let positive_cases = [
            "https://foo.openai.azure.com/openai",
            "https://foo.openai.azure.us/openai/deployments/bar",
            "https://foo.cognitiveservices.azure.cn/openai",
            "https://foo.aoai.azure.com/openai",
            "https://foo.openai.azure-api.net/openai",
            "https://foo.z01.azurefd.net/",
        ];
        for base_url in positive_cases {
            let provider = provider_for(base_url);
            assert!(
                provider.is_azure_responses_endpoint(),
                "expected {base_url} to be detected as Azure"
            );
        }

        let named_provider = ModelProviderInfo {
            name: "Azure".into(),
            base_url: Some("https://example.com".into()),
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        };
        assert!(named_provider.is_azure_responses_endpoint());

        let negative_cases = [
            "https://api.openai.com/v1",
            "https://example.com/openai",
            "https://myproxy.azurewebsites.net/openai",
        ];
        for base_url in negative_cases {
            let provider = provider_for(base_url);
            assert!(
                !provider.is_azure_responses_endpoint(),
                "expected {base_url} not to be detected as Azure"
            );
        }
    }

    #[test]
    fn test_google_genai_provider_creation() {
        let provider = create_google_genai_provider();

        assert_eq!(provider.wire_api, WireApi::GoogleGenAI);
        assert_eq!(provider.name, "Google GenAI");
        assert!(provider.base_url.is_some());
        assert!(
            provider
                .base_url
                .as_ref()
                .unwrap()
                .contains("generativelanguage.googleapis.com")
        );
        assert_eq!(provider.env_key, Some("GOOGLE_GENAI_API_KEY".to_string()));
        assert!(provider.env_key_instructions.is_some());
        assert_eq!(provider.requires_openai_auth, false);

        // Verify env_http_headers contains x-goog-api-key
        assert!(provider.env_http_headers.is_some());
        let env_headers = provider.env_http_headers.unwrap();
        assert_eq!(
            env_headers.get("x-goog-api-key"),
            Some(&"GOOGLE_GENAI_API_KEY".to_string())
        );
    }

    #[test]
    fn test_anthropic_provider_creation() {
        let provider = create_anthropic_provider();

        assert_eq!(provider.wire_api, WireApi::AnthropicMessages);
        assert_eq!(provider.name, "Anthropic");
        assert!(provider.base_url.is_some());
        assert!(
            provider
                .base_url
                .as_ref()
                .unwrap()
                .contains("api.anthropic.com")
        );
        assert_eq!(provider.env_key, Some("ANTHROPIC_API_KEY".to_string()));
        assert!(provider.env_key_instructions.is_some());
        assert_eq!(provider.requires_openai_auth, false);

        // Verify http_headers contains anthropic-version
        assert!(provider.http_headers.is_some());
        let headers = provider.http_headers.as_ref().unwrap();
        assert_eq!(
            headers.get("anthropic-version"),
            Some(&"2023-06-01".to_string())
        );

        // Verify env_http_headers contains x-api-key
        assert!(provider.env_http_headers.is_some());
        let env_headers = provider.env_http_headers.unwrap();
        assert_eq!(
            env_headers.get("x-api-key"),
            Some(&"ANTHROPIC_API_KEY".to_string())
        );
    }

    #[test]
    fn test_url_construction_for_google_genai() {
        let provider = create_google_genai_provider();
        let url = provider.get_full_url(&None);

        assert!(url.contains("generativelanguage.googleapis.com"));
        assert!(url.contains("/v1beta/models/{model}:streamGenerateContent"));
    }

    #[test]
    fn test_url_construction_for_anthropic() {
        let provider = create_anthropic_provider();
        let url = provider.get_full_url(&None);

        assert!(url.contains("api.anthropic.com"));
        assert!(url.ends_with("/messages"));
    }

    #[test]
    fn test_built_in_providers_include_new_providers() {
        let providers = built_in_model_providers();

        // Verify all built-in providers are present
        assert!(providers.contains_key("openai"));
        assert!(providers.contains_key("oss"));
        assert!(providers.contains_key("google_genai"));
        assert!(providers.contains_key("anthropic"));

        // Verify the new providers have correct wire_api
        assert_eq!(
            providers.get("google_genai").unwrap().wire_api,
            WireApi::GoogleGenAI
        );
        assert_eq!(
            providers.get("anthropic").unwrap().wire_api,
            WireApi::AnthropicMessages
        );
    }

    #[test]
    fn test_wire_api_serialization() {
        // Test that WireApi variants serialize correctly for config
        assert_eq!(
            serde_json::to_string(&WireApi::Responses).unwrap(),
            "\"responses\""
        );
        assert_eq!(serde_json::to_string(&WireApi::Chat).unwrap(), "\"chat\"");
        assert_eq!(
            serde_json::to_string(&WireApi::GoogleGenAI).unwrap(),
            "\"google_genai\""
        );
        assert_eq!(
            serde_json::to_string(&WireApi::AnthropicMessages).unwrap(),
            "\"anthropic_messages\""
        );
    }

    #[test]
    fn test_wire_api_deserialization() {
        // Test that WireApi variants deserialize correctly from config
        assert_eq!(
            serde_json::from_str::<WireApi>("\"responses\"").unwrap(),
            WireApi::Responses
        );
        assert_eq!(
            serde_json::from_str::<WireApi>("\"chat\"").unwrap(),
            WireApi::Chat
        );
        assert_eq!(
            serde_json::from_str::<WireApi>("\"google_genai\"").unwrap(),
            WireApi::GoogleGenAI
        );
        assert_eq!(
            serde_json::from_str::<WireApi>("\"anthropic_messages\"").unwrap(),
            WireApi::AnthropicMessages
        );
    }
}
