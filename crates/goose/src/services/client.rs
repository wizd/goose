use super::OpenAiCompatEndpoint;
use super::{configured_selection, request_target, resolve_endpoint, ServiceKind};
use crate::config::tls::provider_tls_config_from_config;
use crate::config::Config;
use crate::providers::api_client::{ApiClient, AuthMethod, TlsConfig};
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde_json::{json, Value};
use std::time::Duration;

const TEXT_TIMEOUT: Duration = Duration::from_secs(60);
const MEDIA_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_ERROR_PREVIEW: usize = 512;

pub struct GeneratedImage {
    pub bytes: Vec<u8>,
    pub mime_type: String,
}

fn prepare(
    endpoint: &OpenAiCompatEndpoint,
    versioned: &str,
    versionless: &str,
    tls: Option<TlsConfig>,
    timeout: Duration,
) -> Result<(ApiClient, String)> {
    let (host, query_params, path) = request_target(&endpoint.base_url, versioned, versionless)?;
    let auth = if endpoint.api_key.is_empty() {
        AuthMethod::NoAuth
    } else {
        AuthMethod::BearerToken(endpoint.api_key.clone())
    };
    let mut client = ApiClient::with_timeout_and_tls(host, auth, timeout, tls)?;
    if !query_params.is_empty() {
        client = client.with_query(query_params);
    }
    if let Some(headers) = &endpoint.headers {
        for (key, value) in headers {
            client = client.with_header(key, value)?;
        }
    }
    Ok((client, path))
}

async fn read_error(response: reqwest::Response) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let mut chars = body.chars();
    let truncated: String = chars.by_ref().take(MAX_ERROR_PREVIEW).collect();
    let body = if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    };
    format!("service request failed ({status}): {body}")
}

async fn json_post(
    endpoint: &OpenAiCompatEndpoint,
    versioned: &str,
    versionless: &str,
    body: &Value,
    tls: Option<TlsConfig>,
    timeout: Duration,
) -> Result<Value> {
    let (client, path) = prepare(endpoint, versioned, versionless, tls, timeout)?;
    let response = client.response_post(&path, body).await?;
    if !response.status().is_success() {
        return Err(anyhow!(read_error(response).await));
    }
    response
        .json()
        .await
        .map_err(|error| anyhow!("service response was not JSON: {error}"))
}

pub async fn transcribe_audio(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    audio: Vec<u8>,
    extension: &str,
    mime_type: &str,
    tls: Option<TlsConfig>,
) -> Result<String> {
    let (client, path) = prepare(
        endpoint,
        "v1/audio/transcriptions",
        "audio/transcriptions",
        tls,
        TEXT_TIMEOUT,
    )?;
    let part = reqwest::multipart::Part::bytes(audio)
        .file_name(format!("audio.{extension}"))
        .mime_str(mime_type)
        .map_err(|error| anyhow!(error))?;
    let form = reqwest::multipart::Form::new()
        .part("file", part)
        .text("model", model.to_string());
    let response = client.request(&path).multipart_post(form).await?;
    if !response.status().is_success() {
        return Err(anyhow!(read_error(response).await));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| anyhow!("transcription response was not JSON: {error}"))?;
    body.get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| anyhow!("transcription response did not include text"))
}

pub async fn transcribe_configured(
    audio: Vec<u8>,
    extension: &str,
    mime_type: &str,
) -> Result<String> {
    let config = Config::global();
    let (provider, model) = configured_selection(config, ServiceKind::Stt)
        .ok_or_else(|| anyhow!("STT service provider and model are not configured"))?;
    let endpoint = resolve_endpoint(config, &provider)?;
    let tls = provider_tls_config_from_config(config)?;
    transcribe_audio(&endpoint, &model, audio, extension, mime_type, tls).await
}

pub async fn speech(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    text: &str,
    voice: &str,
    tls: Option<TlsConfig>,
) -> Result<Vec<u8>> {
    let (client, path) = prepare(
        endpoint,
        "v1/audio/speech",
        "audio/speech",
        tls,
        MEDIA_TIMEOUT,
    )?;
    let response = client
        .response_post(
            &path,
            &json!({
                "model": model,
                "input": text,
                "voice": voice,
            }),
        )
        .await?;
    if !response.status().is_success() {
        return Err(anyhow!(read_error(response).await));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| anyhow!("speech response could not be read: {error}"))?;
    Ok(bytes.to_vec())
}

pub async fn embed_text(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    input: &str,
    tls: Option<TlsConfig>,
) -> Result<Vec<f64>> {
    let body = json_post(
        endpoint,
        "v1/embeddings",
        "embeddings",
        &json!({ "model": model, "input": input }),
        tls,
        TEXT_TIMEOUT,
    )
    .await?;
    body.pointer("/data/0/embedding")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_f64).collect::<Vec<_>>())
        .filter(|values| !values.is_empty())
        .ok_or_else(|| anyhow!("embedding response did not include a vector"))
}

pub async fn generate_image(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    prompt: &str,
    size: Option<&str>,
    tls: Option<TlsConfig>,
) -> Result<GeneratedImage> {
    let mut body = json!({ "model": model, "prompt": prompt, "n": 1 });
    if let Some(size) = size {
        body["size"] = json!(size);
    }
    let value = json_post(
        endpoint,
        "v1/images/generations",
        "images/generations",
        &body,
        tls,
        MEDIA_TIMEOUT,
    )
    .await?;
    let item = value
        .pointer("/data/0")
        .ok_or_else(|| anyhow!("image response did not include data"))?;
    if let Some(encoded) = item.get("b64_json").and_then(Value::as_str) {
        let bytes = BASE64
            .decode(encoded)
            .map_err(|error| anyhow!("image response was not valid base64: {error}"))?;
        return Ok(GeneratedImage {
            mime_type: sniff_image_mime(&bytes).to_string(),
            bytes,
        });
    }
    if let Some(url) = item.get("url").and_then(Value::as_str) {
        let response = reqwest::get(url)
            .await
            .map_err(|error| anyhow!("failed to download generated image: {error}"))?;
        if !response.status().is_success() {
            return Err(anyhow!(read_error(response).await));
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|error| anyhow!("generated image could not be read: {error}"))?
            .to_vec();
        return Ok(GeneratedImage {
            mime_type: sniff_image_mime(&bytes).to_string(),
            bytes,
        });
    }
    Err(anyhow!("image response did not include b64_json or url"))
}

pub async fn vision_chat(
    endpoint: &OpenAiCompatEndpoint,
    model: &str,
    image: &[u8],
    mime_type: &str,
    question: &str,
    tls: Option<TlsConfig>,
) -> Result<String> {
    let data_url = format!("data:{mime_type};base64,{}", BASE64.encode(image));
    let body = json_post(
        endpoint,
        "v1/chat/completions",
        "chat/completions",
        &json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": [
                    { "type": "text", "text": question },
                    { "type": "image_url", "image_url": { "url": data_url } }
                ]
            }]
        }),
        tls,
        TEXT_TIMEOUT,
    )
    .await?;
    body.pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("vision response did not include text"))
}

fn sniff_image_mime(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/png"
    }
}
