use goose::config::Config;
use goose::services::{
    check_models, configured_media_tools, configured_selection, embed_text, generate_image,
    image_list_notice, listed_model_ids, red_png, request_target, resolve_endpoint, silent_wav,
    speech, strip_openai_endpoint_suffix, transcribe_audio, vision_chat, OpenAiCompatEndpoint,
    ServiceKind, MODEL_NOT_LISTED,
};
use image::GenericImageView;
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

#[test]
fn silent_wav_is_one_second_of_16khz_silence() {
    let wav = silent_wav();
    assert_eq!(&wav[0..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
    assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 32_000);
    assert_eq!(wav.len(), 44 + 32_000);
    assert!(wav[44..].iter().all(|byte| *byte == 0));
}

#[test]
fn red_png_decodes_to_one_red_pixel() {
    let image = image::load_from_memory(red_png()).unwrap();
    assert_eq!((image.width(), image.height()), (1, 1));
    let pixel = image.get_pixel(0, 0);
    assert_eq!(pixel[0], 255);
    assert_eq!(pixel[1], 0);
    assert_eq!(pixel[2], 0);
}

#[test]
fn unknown_service_names_are_rejected() {
    assert_eq!(
        ServiceKind::from_verify_name("vision"),
        Some(ServiceKind::Vision)
    );
    assert_eq!(
        ServiceKind::from_verify_name("chat"),
        Some(ServiceKind::Chat)
    );
    assert!(ServiceKind::from_verify_name("realtime").is_none());
    assert!(ServiceKind::from_verify_name("video").is_none());
    assert!(ServiceKind::from_verify_name("nope").is_none());
}

#[test]
fn image_list_notice_only_warns_when_a_parsed_list_omits_the_model() {
    let listed = vec!["gpt-image-1".to_string()];
    assert_eq!(image_list_notice(Some(&listed), "gpt-image-1"), None);
    assert_eq!(
        image_list_notice(Some(&listed), "missing"),
        Some(MODEL_NOT_LISTED)
    );
    assert_eq!(image_list_notice(None, "missing"), None);
    assert_eq!(
        listed_model_ids(&json!({ "data": [{ "id": "a" }, "b"] })),
        Some(vec!["a".to_string(), "b".to_string()])
    );
    assert_eq!(listed_model_ids(&json!({ "error": "nope" })), None);
}

#[tokio::test]
async fn image_verify_checks_auth_without_generating() {
    let denied = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401).set_body_string("bad key"))
        .mount(&denied)
        .await;
    let error = check_models(&endpoint(&denied.uri()), "gpt-image-1", None)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("authentication failed"));

    let ready = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{ "id": "other-model" }]
        })))
        .mount(&ready)
        .await;
    let notice = check_models(&endpoint(&ready.uri()), "gpt-image-1", None)
        .await
        .unwrap();
    assert_eq!(notice.as_deref(), Some(MODEL_NOT_LISTED));
}
