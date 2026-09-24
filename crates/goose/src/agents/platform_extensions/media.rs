use std::path::{Path, PathBuf};

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rmcp::model::{
    CallToolResult, ContentBlock, Implementation, InitializeResult, JsonObject, ListToolsResult,
    ServerCapabilities, Tool,
};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::tool_execution::ToolCallContext;
use crate::config::tls::provider_tls_config_from_config;
use crate::config::Config;
use crate::services::{
    configured_media_tools, configured_selection, embed_text, generate_image, resolve_endpoint,
    speech, vision_chat, ServiceKind,
};

pub static EXTENSION_NAME: &str = "media";

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const INLINE_EMBEDDING_LIMIT: usize = 64;

#[derive(Debug, Deserialize, JsonSchema)]
struct DescribeImageParams {
    /// Local file path or http(s) URL of the image to describe.
    source: String,
    /// Question to ask about the image.
    question: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct TextToSpeechParams {
    /// Text to speak.
    text: String,
    /// Voice name understood by the configured speech model. Defaults to alloy.
    voice: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct EmbedTextParams {
    /// Text to embed.
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct GenerateImageParams {
    /// Description of the image to generate.
    prompt: String,
    /// Optional size such as 1024x1024, when the model accepts it.
    size: Option<String>,
}

pub struct MediaClient {
    info: InitializeResult,
    #[allow(dead_code)]
    context: PlatformExtensionContext,
}

impl MediaClient {
    pub fn new(context: PlatformExtensionContext) -> anyhow::Result<Self> {
        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(EXTENSION_NAME.to_string(), "1.0.0".to_string())
                    .with_title("Media"),
            );
        Ok(Self { info, context })
    }

    fn tools() -> Vec<Tool> {
        let available = configured_media_tools(Config::global());
        let mut tools = Vec::new();
        if available.contains(&"describe_image") {
            tools.push(tool(
                "describe_image",
                "Describe an image with the configured vision service. Use this when the chat model cannot see images.",
                schema_of::<DescribeImageParams>(),
            ));
        }
        if available.contains(&"text_to_speech") {
            tools.push(tool(
                "text_to_speech",
                "Speak text with the configured text-to-speech service and save an audio file.",
                schema_of::<TextToSpeechParams>(),
            ));
        }
        if available.contains(&"embed_text") {
            tools.push(tool(
                "embed_text",
                "Embed text with the configured embedding service.",
                schema_of::<EmbedTextParams>(),
            ));
        }
        if available.contains(&"generate_image") {
            tools.push(tool(
                "generate_image",
                "Generate an image from a text prompt with the configured image service.",
                schema_of::<GenerateImageParams>(),
            ));
        }
        tools
    }
}

fn schema_of<T: schemars::JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("schema")
}

fn tool(name: &'static str, description: &'static str, schema: serde_json::Value) -> Tool {
    Tool::new(
        name,
        description,
        schema.as_object().expect("schema object").clone(),
    )
}

fn error_result(message: impl Into<String>) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.into())])
}

fn output_dir(working_dir: Option<&str>) -> PathBuf {
    let base = working_dir
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let dir = base.join("goose-media");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn mime_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "image/png",
    }
}

async fn load_image(source: &str, working_dir: Option<&str>) -> anyhow::Result<(Vec<u8>, String)> {
    let bytes = if source.starts_with("http://") || source.starts_with("https://") {
        let response = reqwest::get(source)
            .await
            .map_err(|error| anyhow::anyhow!("failed to download image: {error}"))?;
        if !response.status().is_success() {
            anyhow::bail!("image download failed ({})", response.status());
        }
        response
            .bytes()
            .await
            .map_err(|error| anyhow::anyhow!("image download could not be read: {error}"))?
            .to_vec()
    } else {
        let path = Path::new(source);
        let path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            working_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
                .join(path)
        };
        std::fs::read(&path).map_err(|error| anyhow::anyhow!("failed to read {source}: {error}"))?
    };
    if bytes.len() > MAX_IMAGE_BYTES {
        anyhow::bail!("image exceeds the 20MB limit");
    }
    let mime = if source.starts_with("http://") || source.starts_with("https://") {
        "image/png".to_string()
    } else {
        mime_for_path(Path::new(source)).to_string()
    };
    Ok((bytes, mime))
}

#[async_trait]
impl McpClientTrait for MediaClient {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        Ok(ListToolsResult {
            tools: Self::tools(),
            next_cursor: None,
            meta: None,
            ..Default::default()
        })
    }

    async fn call_tool(
        &self,
        ctx: &ToolCallContext,
        name: &str,
        arguments: Option<JsonObject>,
        _cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        let arguments = arguments.unwrap_or_default();
        let working_dir = ctx.working_dir_str();
        match name {
            "describe_image" => Ok(describe_image(working_dir, arguments).await),
            "text_to_speech" => Ok(text_to_speech(working_dir, arguments).await),
            "embed_text" => Ok(embed(working_dir, arguments).await),
            "generate_image" => Ok(generate(working_dir, arguments).await),
            other => Ok(error_result(format!("Unknown tool: {other}"))),
        }
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        Some(&self.info)
    }
}

async fn describe_image(working_dir: Option<&str>, arguments: JsonObject) -> CallToolResult {
    let params: DescribeImageParams =
        match serde_json::from_value(serde_json::Value::Object(arguments)) {
            Ok(params) => params,
            Err(error) => return error_result(format!("Invalid arguments: {error}")),
        };
    let (bytes, mime) = match load_image(&params.source, working_dir).await {
        Ok(image) => image,
        Err(error) => return error_result(error.to_string()),
    };
    let config = Config::global();
    let Some((provider, model)) = configured_selection(config, ServiceKind::Vision) else {
        return error_result("Vision service provider and model are not configured");
    };
    let endpoint = match resolve_endpoint(config, &provider) {
        Ok(endpoint) => endpoint,
        Err(error) => return error_result(error.to_string()),
    };
    let tls = match provider_tls_config_from_config(config) {
        Ok(tls) => tls,
        Err(error) => return error_result(error.to_string()),
    };
    match vision_chat(&endpoint, &model, &bytes, &mime, &params.question, tls).await {
        Ok(text) => CallToolResult::success(vec![ContentBlock::text(text)]),
        Err(error) => error_result(error.to_string()),
    }
}

async fn text_to_speech(working_dir: Option<&str>, arguments: JsonObject) -> CallToolResult {
    let params: TextToSpeechParams =
        match serde_json::from_value(serde_json::Value::Object(arguments)) {
            Ok(params) => params,
            Err(error) => return error_result(format!("Invalid arguments: {error}")),
        };
    let config = Config::global();
    let Some((provider, model)) = configured_selection(config, ServiceKind::Tts) else {
        return error_result("TTS service provider and model are not configured");
    };
    let endpoint = match resolve_endpoint(config, &provider) {
        Ok(endpoint) => endpoint,
        Err(error) => return error_result(error.to_string()),
    };
    let tls = match provider_tls_config_from_config(config) {
        Ok(tls) => tls,
        Err(error) => return error_result(error.to_string()),
    };
    let voice = params.voice.as_deref().unwrap_or("alloy");
    let audio = match speech(&endpoint, &model, &params.text, voice, tls).await {
        Ok(audio) => audio,
        Err(error) => return error_result(error.to_string()),
    };
    let path = output_dir(working_dir).join(format!("tts-{}.mp3", uuid::Uuid::new_v4()));
    if let Err(error) = std::fs::write(&path, &audio) {
        return error_result(format!("failed to save audio: {error}"));
    }
    CallToolResult::success(vec![ContentBlock::text(format!(
        "Saved speech audio to {}",
        path.display()
    ))])
}

async fn embed(working_dir: Option<&str>, arguments: JsonObject) -> CallToolResult {
    let params: EmbedTextParams = match serde_json::from_value(serde_json::Value::Object(arguments))
    {
        Ok(params) => params,
        Err(error) => return error_result(format!("Invalid arguments: {error}")),
    };
    let config = Config::global();
    let Some((provider, model)) = configured_selection(config, ServiceKind::Embedding) else {
        return error_result("Embedding service provider and model are not configured");
    };
    let endpoint = match resolve_endpoint(config, &provider) {
        Ok(endpoint) => endpoint,
        Err(error) => return error_result(error.to_string()),
    };
    let tls = match provider_tls_config_from_config(config) {
        Ok(tls) => tls,
        Err(error) => return error_result(error.to_string()),
    };
    let values = match embed_text(&endpoint, &model, &params.text, tls).await {
        Ok(values) => values,
        Err(error) => return error_result(error.to_string()),
    };
    if values.len() <= INLINE_EMBEDDING_LIMIT {
        return CallToolResult::success(vec![ContentBlock::text(
            json!({ "dimensions": values.len(), "embedding": values }).to_string(),
        )]);
    }
    let path = output_dir(working_dir).join(format!("embedding-{}.json", uuid::Uuid::new_v4()));
    let payload = json!({ "dimensions": values.len(), "embedding": values });
    if let Err(error) = std::fs::write(&path, payload.to_string()) {
        return error_result(format!("failed to save embedding: {error}"));
    }
    CallToolResult::success(vec![ContentBlock::text(format!(
        "Saved a {}-dimension embedding to {}",
        values.len(),
        path.display()
    ))])
}

async fn generate(working_dir: Option<&str>, arguments: JsonObject) -> CallToolResult {
    let params: GenerateImageParams =
        match serde_json::from_value(serde_json::Value::Object(arguments)) {
            Ok(params) => params,
            Err(error) => return error_result(format!("Invalid arguments: {error}")),
        };
    let config = Config::global();
    let Some((provider, model)) = configured_selection(config, ServiceKind::Image) else {
        return error_result("Image service provider and model are not configured");
    };
    let endpoint = match resolve_endpoint(config, &provider) {
        Ok(endpoint) => endpoint,
        Err(error) => return error_result(error.to_string()),
    };
    let tls = match provider_tls_config_from_config(config) {
        Ok(tls) => tls,
        Err(error) => return error_result(error.to_string()),
    };
    let image = match generate_image(
        &endpoint,
        &model,
        &params.prompt,
        params.size.as_deref(),
        tls,
    )
    .await
    {
        Ok(image) => image,
        Err(error) => return error_result(error.to_string()),
    };
    let extension = match image.mime_type.as_str() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/gif" => "gif",
        _ => "png",
    };
    let path = output_dir(working_dir).join(format!("image-{}.{extension}", uuid::Uuid::new_v4()));
    if let Err(error) = std::fs::write(&path, &image.bytes) {
        return error_result(format!("failed to save image: {error}"));
    }
    CallToolResult::success(vec![
        ContentBlock::text(format!("Saved generated image to {}", path.display())),
        ContentBlock::image(BASE64.encode(&image.bytes), image.mime_type),
    ])
}
