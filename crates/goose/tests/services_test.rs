use goose::config::Config;
use goose::services::{
    configured_media_tools, configured_selection, embed_text, generate_image, request_target,
    resolve_endpoint, speech, strip_openai_endpoint_suffix, transcribe_audio, vision_chat,
    OpenAiCompatEndpoint, ServiceKind,
};
use serde_json::json;
use tempfile::NamedTempFile;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config() -> (Config, NamedTempFile, NamedTempFile) {
    let config_file = NamedTempFile::new().unwrap();
    let secrets_file = NamedTempFile::new().unwrap();
    let config = Config::new_with_file_secrets(config_file.path(), secrets_file.path()).unwrap();
    (config, config_file, secrets_file)
}

fn endpoint(base_url: &str) -> OpenAiCompatEndpoint {
    OpenAiCompatEndpoint {
        api_key: String::new(),
        base_url: base_url.to_string(),
        headers: None,
    }
}

#[test]
fn strips_chat_completions_suffix_used_by_gateways() {
    assert_eq!(
        strip_openai_endpoint_suffix("https://api.vcorp.ai/v1/chat/completions"),
        "https://api.vcorp.ai"
    );
    assert_eq!(
        strip_openai_endpoint_suffix("https://api.openai.com/v1"),
        "https://api.openai.com/v1"
    );
    assert_eq!(
        strip_openai_endpoint_suffix("https://api.openai.com"),
        "https://api.openai.com"
    );
}

#[test]
fn vcorp_chat_url_targets_the_versioned_service_path() {
    let (host, query, request_path) = request_target(
        "https://api.vcorp.ai/v1/chat/completions",
        "v1/images/generations",
        "images/generations",
    )
    .unwrap();
    assert_eq!(host, "https://api.vcorp.ai");
    assert!(query.is_empty());
    assert_eq!(request_path, "v1/images/generations");
}

#[test]
fn unconfigured_services_expose_no_media_tools() {
    let (config, _config_file, _secrets_file) = test_config();
    assert!(configured_media_tools(&config).is_empty());
    assert!(configured_selection(&config, ServiceKind::Vision).is_none());
}

#[test]
fn configured_service_exposes_only_that_tool() {
    let (config, _config_file, _secrets_file) = test_config();
    config
        .set_param("GOOSE_SERVICE_IMAGE_PROVIDER", "vcorp")
        .unwrap();
    config
        .set_param("GOOSE_SERVICE_IMAGE_MODEL", "google/imagen")
        .unwrap();

    assert_eq!(
        configured_selection(&config, ServiceKind::Image),
        Some(("vcorp".to_string(), "google/imagen".to_string()))
    );
    assert_eq!(configured_media_tools(&config), vec!["generate_image"]);
}

#[test]
fn unknown_provider_is_rejected() {
    let (config, _config_file, _secrets_file) = test_config();
    let error = resolve_endpoint(&config, "not-a-real-provider").unwrap_err();
    assert!(error.to_string().contains("not supported"));
}

#[tokio::test]
async fn service_clients_hit_openai_compatible_paths() {
    let server = MockServer::start().await;
    let chat_url = format!("{}/v1/chat/completions", server.uri());
    let target = endpoint(&chat_url);

    Mock::given(method("POST"))
        .and(path("/v1/audio/transcriptions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "text": "hello" })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/audio/speech"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"AUDIO"))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/embeddings"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "embedding": [0.25, 0.5] }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/images/generations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "b64_json": "aGk=" }]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "choices": [{ "message": { "content": "a red square" } }]
        })))
        .mount(&server)
        .await;

    let text = transcribe_audio(
        &target,
        "whisper-1",
        b"wav".to_vec(),
        "wav",
        "audio/wav",
        None,
    )
    .await
    .unwrap();
    assert_eq!(text, "hello");

    let audio = speech(&target, "tts-1", "hello", "alloy", None)
        .await
        .unwrap();
    assert_eq!(audio, b"AUDIO");

    let embedding = embed_text(&target, "text-embedding-3-small", "hello", None)
        .await
        .unwrap();
    assert_eq!(embedding, vec![0.25, 0.5]);

    let image = generate_image(&target, "gpt-image-1", "a square", None, None)
        .await
        .unwrap();
    assert_eq!(image.bytes, b"hi");

    let description = vision_chat(
        &target,
        "gpt-4o",
        b"png",
        "image/png",
        "what is this?",
        None,
    )
    .await
    .unwrap();
    assert_eq!(description, "a red square");

    server.verify().await;
}
