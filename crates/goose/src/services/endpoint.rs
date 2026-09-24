use crate::config::Config;
use anyhow::{anyhow, Result};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct OpenAiCompatEndpoint {
    pub api_key: String,
    pub base_url: String,
    pub headers: Option<HashMap<String, String>>,
}

const ENDPOINT_SUFFIXES: &[&str] = &[
    "/v1/chat/completions",
    "/chat/completions",
    "/v1/responses",
    "/responses",
    "/v1/audio/transcriptions",
    "/audio/transcriptions",
    "/v1/audio/speech",
    "/audio/speech",
    "/v1/embeddings",
    "/embeddings",
    "/v1/images/generations",
    "/images/generations",
];

/// Gateways such as vcorp store the chat completions URL as `base_url`.
/// Service calls need the API root, so a known endpoint suffix is removed first.
pub fn strip_openai_endpoint_suffix(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    for suffix in ENDPOINT_SUFFIXES {
        if let Some(prefix) = trimmed.strip_suffix(suffix) {
            let prefix = prefix.trim_end_matches('/');
            if !prefix.is_empty() {
                return prefix.to_string();
            }
        }
    }
    trimmed.to_string()
}

pub fn normalize_openrouter_base_url(base_url: &str) -> String {
    if !base_url.contains("/api") {
        format!("{}/api/v1", base_url.trim_end_matches('/'))
    } else if base_url.ends_with("/api") {
        format!("{}/v1", base_url)
    } else {
        base_url.to_string()
    }
}

pub type ServiceRequestTarget = (String, Vec<(String, String)>, String);

pub fn request_target(
    base_url: &str,
    versioned: &str,
    versionless: &str,
) -> Result<ServiceRequestTarget> {
    let stripped = strip_openai_endpoint_suffix(base_url);
    let (host, query_params, has_v1) = crate::providers::openai::parse_openai_base_url(&stripped)?;
    let path = if has_v1 { versioned } else { versionless };
    Ok((host, query_params, path.to_string()))
}

pub fn resolve_endpoint(config: &Config, provider_name: &str) -> Result<OpenAiCompatEndpoint> {
    if let Ok(loaded) = crate::config::declarative_providers::load_provider(provider_name) {
        let mut cfg = loaded.config;
        use goose_providers::declarative::ProviderEngine;
        match cfg.engine {
            ProviderEngine::OpenAI | ProviderEngine::Ollama => {}
            ProviderEngine::Anthropic => {
                anyhow::bail!(
                    "Provider '{provider_name}' uses the Anthropic engine, which does not speak \
                     the OpenAI-compatible API these services call"
                )
            }
        }
        if let Some(ref env_vars) = cfg.env_vars {
            cfg.base_url =
                crate::config::declarative_providers::expand_env_vars(&cfg.base_url, env_vars)?;
        }
        let api_key = if cfg.api_key_env.is_empty() {
            String::new()
        } else if cfg.requires_auth {
            config.get_secret::<String>(&cfg.api_key_env).map_err(|_| {
                anyhow!(
                    "API key '{}' required for provider '{provider_name}' but not configured",
                    cfg.api_key_env
                )
            })?
        } else {
            config
                .get_secret::<String>(&cfg.api_key_env)
                .unwrap_or_default()
        };
        return Ok(OpenAiCompatEndpoint {
            api_key,
            base_url: strip_openai_endpoint_suffix(&cfg.base_url),
            headers: cfg.headers.clone(),
        });
    }

    match provider_name {
        "openai" => {
            let api_key = config
                .get_secret::<String>("OPENAI_API_KEY")
                .unwrap_or_default();
            let base_url = if let Ok(host) = std::env::var("OPENAI_HOST") {
                host
            } else if let Ok(url) = config.get_param::<String>("OPENAI_BASE_URL") {
                let trimmed = url.trim().to_string();
                if trimmed.is_empty() {
                    "https://api.openai.com".to_string()
                } else {
                    trimmed
                }
            } else {
                config
                    .get_param::<String>("OPENAI_HOST")
                    .unwrap_or_else(|_| "https://api.openai.com".to_string())
            };
            let mut headers: HashMap<String, String> = config
                .get_secret::<String>("OPENAI_CUSTOM_HEADERS")
                .ok()
                .map(crate::providers::openai::parse_custom_headers)
                .unwrap_or_default();
            if let Ok(org) = config.get_param::<String>("OPENAI_ORGANIZATION") {
                headers.insert("OpenAI-Organization".to_string(), org);
            }
            if let Ok(project) = config.get_param::<String>("OPENAI_PROJECT") {
                headers.insert("OpenAI-Project".to_string(), project);
            }
            let headers = if headers.is_empty() {
                None
            } else {
                Some(headers)
            };
            Ok(OpenAiCompatEndpoint {
                api_key,
                base_url: strip_openai_endpoint_suffix(&base_url),
                headers,
            })
        }
        "openrouter" => {
            let api_key = config
                .get_secret::<String>("OPENROUTER_API_KEY")
                .unwrap_or_default();
            let base_url = normalize_openrouter_base_url(
                &config
                    .get_param::<String>("OPENROUTER_HOST")
                    .unwrap_or_else(|_| "https://openrouter.ai/api/v1".to_string()),
            );
            Ok(OpenAiCompatEndpoint {
                api_key,
                base_url: strip_openai_endpoint_suffix(&base_url),
                headers: None,
            })
        }
        "groq" => {
            let api_key = config
                .get_secret::<String>("GROQ_API_KEY")
                .unwrap_or_default();
            let base_url = config
                .get_param::<String>("GROQ_HOST")
                .unwrap_or_else(|_| "https://api.groq.com/openai".to_string());
            Ok(OpenAiCompatEndpoint {
                api_key,
                base_url: strip_openai_endpoint_suffix(&base_url),
                headers: None,
            })
        }
        "ollama" => {
            let base_url = config
                .get_param::<String>("OLLAMA_HOST")
                .unwrap_or_else(|_| "http://localhost:11434".to_string());
            Ok(OpenAiCompatEndpoint {
                api_key: String::new(),
                base_url: strip_openai_endpoint_suffix(&base_url),
                headers: None,
            })
        }
        "google" => {
            let has_custom_host = config.get_param::<String>("GOOGLE_HOST").is_ok();
            let api_key = if has_custom_host {
                config
                    .get_secret::<String>("GOOGLE_API_KEY")
                    .unwrap_or_default()
            } else {
                config.get_secret::<String>("GOOGLE_API_KEY").map_err(|_| {
                    anyhow!(
                        "GOOGLE_API_KEY required for the hosted Gemini OpenAI-compatible endpoint"
                    )
                })?
            };
            let base_url = config
                .get_param::<String>("GOOGLE_HOST")
                .unwrap_or_else(|_| {
                    "https://generativelanguage.googleapis.com/v1beta/openai".to_string()
                });
            Ok(OpenAiCompatEndpoint {
                api_key,
                base_url: strip_openai_endpoint_suffix(&base_url),
                headers: None,
            })
        }
        other => {
            anyhow::bail!(
                "Provider '{other}' is not supported for OpenAI-compatible services. \
                 Use a provider with an OpenAI-compatible chat completions endpoint."
            )
        }
    }
}
