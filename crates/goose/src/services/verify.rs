use super::client::{prepare, read_error};
use super::{
    embed_text, resolve_endpoint, speech, transcribe_audio, vision_chat, OpenAiCompatEndpoint,
    ServiceKind,
};
use crate::config::tls::provider_tls_config_from_config;
use crate::config::Config;
use crate::providers::api_client::TlsConfig;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{DynamicImage, ImageOutputFormat, Rgb, RgbImage};
use serde_json::Value;
use std::io::Cursor;
use std::sync::OnceLock;
use std::time::Duration;

const HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const DETAIL_CHARS: usize = 200;
const SAMPLE_RATE: u32 = 16_000;

/// Detail value when the image endpoint accepts the key but does not list the model.
pub const MODEL_NOT_LISTED: &str = "model_not_listed";

pub(crate) struct VerifyAudio {
    pub data: String,
    pub mime_type: String,
}

pub(crate) struct VerifyMediaOutput {
    pub detail: Option<String>,
    pub audio: Option<VerifyAudio>,
}

pub(crate) fn preview(text: &str, max_chars: usize) -> String {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let short: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{short}…")
    } else {
        short
    }
}

/// One second of 16 kHz 16-bit mono silence. A transcription call only needs a
/// well-formed file; an empty transcript still means the model accepted it.
pub fn silent_wav() -> Vec<u8> {
    let data_len = SAMPLE_RATE * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    bytes.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    bytes
}

pub fn red_png() -> &'static [u8] {
    static PNG: OnceLock<Vec<u8>> = OnceLock::new();
    PNG.get_or_init(|| {
        let image = RgbImage::from_pixel(1, 1, Rgb([255, 0, 0]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_to(&mut Cursor::new(&mut bytes), ImageOutputFormat::Png)
            .expect("1x1 png");
        bytes
    })
    .as_slice()
}

pub fn listed_model_ids(body: &Value) -> Option<Vec<String>> {
    let items = body
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| body.get("models").and_then(Value::as_array))?;
    Some(items.iter().filter_map(model_id).collect())
}

pub fn image_list_notice(ids: Option<&[String]>, model: &str) -> Option<&'static str> {
    let ids = ids?;
    if ids.iter().any(|id| id == model) {
        None
    } else {
        Some(MODEL_NOT_LISTED)
    }
}

fn model_id(item: &Value) -> Option<String> {
    item.as_str()
        .or_else(|| item.get("id").and_then(Value::as_str))
        .or_else(|| item.get("name").and_then(Value::as_str))
        .map(str::to_string)
}

/// Image verification checks the OpenAI-compatible models endpoint instead of
/// generating a picture, which would spend money and time on every click.
pub async fn check_models(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    tls: Option<TlsConfig>,
) -> Result<Option<String>> {
    let (client, path) = prepare(endpoint, "v1/models", "models", tls, HTTP_TIMEOUT)?;
    let response = client.response_get(&path).await?;
    let status = response.status();
    if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
        return Err(anyhow!(
            "authentication failed: {}",
            read_error(response).await
        ));
    }
    if !status.is_success() {
        return Err(anyhow!(read_error(response).await));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| anyhow!("model list was not JSON: {error}"))?;
    if body.get("error").is_some() && listed_model_ids(&body).is_none() {
        return Err(anyhow!(
            "model list request failed: {}",
            preview(&body.to_string(), 512)
        ));
    }
    Ok(image_list_notice(listed_model_ids(&body).as_deref(), model).map(str::to_string))
}

pub(crate) async fn verify_media(
    provider_name: &str,
    model: &str,
    kind: ServiceKind,
) -> Result<VerifyMediaOutput> {
    let config = Config::global();
    let endpoint = resolve_endpoint(config, provider_name)?;
    let tls = provider_tls_config_from_config(config)?;
    match kind {
        ServiceKind::Vision => {
            let text = vision_chat(
                &endpoint,
                model,
                red_png(),
                "image/png",
                "What color is this image? Answer in one word.",
                tls,
            )
            .await?;
            Ok(text_output(&text))
        }
        ServiceKind::Stt => {
            let text =
                transcribe_audio(&endpoint, model, silent_wav(), "wav", "audio/wav", tls).await?;
            Ok(text_output(&text))
        }
        ServiceKind::Tts => {
            let audio = speech(&endpoint, model, "OK", "alloy", tls).await?;
            if audio.is_empty() {
                anyhow::bail!("speech response was empty");
            }
            Ok(VerifyMediaOutput {
                detail: Some(audio.len().to_string()),
                audio: Some(VerifyAudio {
                    mime_type: sniff_audio_mime(&audio).to_string(),
                    data: BASE64.encode(&audio),
                }),
            })
        }
        ServiceKind::Embedding => {
            let values = embed_text(&endpoint, model, "hello", tls).await?;
            Ok(VerifyMediaOutput {
                detail: Some(values.len().to_string()),
                audio: None,
            })
        }
        ServiceKind::Image => Ok(VerifyMediaOutput {
            detail: check_models(&endpoint, model, tls).await?,
            audio: None,
        }),
        ServiceKind::Chat | ServiceKind::Realtime | ServiceKind::Video => {
            anyhow::bail!("service '{kind:?}' is not verified through the media client")
        }
    }
}

fn text_output(text: &str) -> VerifyMediaOutput {
    let detail = preview(text, DETAIL_CHARS);
    VerifyMediaOutput {
        detail: if detail.is_empty() {
            None
        } else {
            Some(detail)
        },
        audio: None,
    }
}

fn sniff_audio_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"RIFF") {
        "audio/wav"
    } else if bytes.starts_with(b"OggS") {
        "audio/ogg"
    } else if bytes.starts_with(b"fLaC") {
        "audio/flac"
    } else {
        "audio/mpeg"
    }
}
