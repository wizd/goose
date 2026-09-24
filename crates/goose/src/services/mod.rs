mod client;
mod endpoint;

pub use client::{
    embed_text, generate_image, speech, transcribe_audio, transcribe_configured, vision_chat,
};
pub use endpoint::{
    normalize_openrouter_base_url, request_target, resolve_endpoint, strip_openai_endpoint_suffix,
    OpenAiCompatEndpoint,
};

use crate::config::Config;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceKind {
    Chat,
    Vision,
    Stt,
    Realtime,
    Tts,
    Embedding,
    Image,
    Video,
}

impl ServiceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "CHAT",
            Self::Vision => "VISION",
            Self::Stt => "STT",
            Self::Realtime => "REALTIME",
            Self::Tts => "TTS",
            Self::Embedding => "EMBEDDING",
            Self::Image => "IMAGE",
            Self::Video => "VIDEO",
        }
    }

    /// Services the agent can call. Chat uses the existing default provider,
    /// and realtime voice and video only store configuration.
    pub fn wired(self) -> bool {
        matches!(
            self,
            Self::Vision | Self::Stt | Self::Tts | Self::Embedding | Self::Image
        )
    }
}

pub fn provider_key(kind: ServiceKind) -> String {
    format!("GOOSE_SERVICE_{}_PROVIDER", kind.as_str())
}

pub fn model_key(kind: ServiceKind) -> String {
    format!("GOOSE_SERVICE_{}_MODEL", kind.as_str())
}

pub fn configured_selection(config: &Config, kind: ServiceKind) -> Option<(String, String)> {
    let provider = non_empty_param(config, &provider_key(kind))?;
    let model = non_empty_param(config, &model_key(kind))?;
    Some((provider, model))
}

fn non_empty_param(config: &Config, key: &str) -> Option<String> {
    config
        .get_param::<String>(key)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub fn configured_media_tools(config: &Config) -> Vec<&'static str> {
    let mut tools = Vec::new();
    if configured_selection(config, ServiceKind::Vision).is_some() {
        tools.push("describe_image");
    }
    if configured_selection(config, ServiceKind::Tts).is_some() {
        tools.push("text_to_speech");
    }
    if configured_selection(config, ServiceKind::Embedding).is_some() {
        tools.push("embed_text");
    }
    if configured_selection(config, ServiceKind::Image).is_some() {
        tools.push("generate_image");
    }
    tools
}
