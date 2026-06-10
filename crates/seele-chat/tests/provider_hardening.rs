//! Q9 — first test suite of seele-chat: bounded retries (429/5xx),
//! fail-fast on plain 4xx, server-side think-stripping, and tool dispatch
//! by name in the run_chat loop.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use seele_chat::{
    run_chat, ChatConfig, ChatError, ChatProvider, Message, OpenAICompatibleProvider, ToolCall,
    ToolCallFunction, ToolHandler,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn user(text: &str) -> Message {
    Message {
        role: "user".to_string(),
        content: Some(text.to_string()),
        tool_calls: Vec::new(),
        tool_call_id: None,
        name: None,
    }
}

fn ok_body(text: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [{ "message": { "role": "assistant", "content": text } }]
    })
}

fn provider_for(server: &MockServer) -> OpenAICompatibleProvider {
    OpenAICompatibleProvider::new(
        "test",
        format!("{}/v1/chat/completions", server.uri()),
        "test-key",
        "test-model",
        vec![],
    )
    .with_retry_pause(Duration::from_millis(20))
}

#[tokio::test]
async fn retries_on_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("hola")))
        .expect(1)
        .mount(&server)
        .await;

    let msg = provider_for(&server)
        .complete(&[user("hola")])
        .await
        .expect("429 then 200 must succeed");
    assert_eq!(msg.content.as_deref(), Some("hola"));
}

#[tokio::test]
async fn retries_on_500_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ok_body("ok")))
        .expect(1)
        .mount(&server)
        .await;

    let msg = provider_for(&server)
        .complete(&[user("x")])
        .await
        .expect("two 503s then 200 within the 3-attempt budget");
    assert_eq!(msg.content.as_deref(), Some("ok"));
}

#[tokio::test]
async fn plain_4xx_fails_fast_without_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string("bad request"))
        .expect(1) // exactly one request: no retries on non-429 4xx
        .mount(&server)
        .await;

    let err = provider_for(&server)
        .complete(&[user("x")])
        .await
        .expect_err("400 must fail");
    match err {
        ChatError::ProviderStatus { status, .. } => assert_eq!(status, 400),
        other => panic!("expected ProviderStatus, got {other:?}"),
    }
}

#[tokio::test]
async fn exhausted_retries_surface_the_last_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .expect(3) // attempt budget is 3
        .mount(&server)
        .await;

    let err = provider_for(&server)
        .complete(&[user("x")])
        .await
        .expect_err("persistent 429 must fail after 3 attempts");
    match err {
        ChatError::ProviderStatus { status, .. } => assert_eq!(status, 429),
        other => panic!("expected ProviderStatus, got {other:?}"),
    }
}

// -------- run_chat: dispatch + strip --------

struct ScriptedProvider {
    turn: AtomicUsize,
}

#[async_trait::async_trait]
impl ChatProvider for ScriptedProvider {
    async fn complete(&self, _messages: &[Message]) -> Result<Message, ChatError> {
        if self.turn.fetch_add(1, Ordering::SeqCst) == 0 {
            Ok(Message {
                role: "assistant".to_string(),
                content: Some("<think>plan secreto</think>voy a llamar una tool".to_string()),
                tool_calls: vec![ToolCall {
                    id: "t1".to_string(),
                    r#type: "function".to_string(),
                    function: ToolCallFunction {
                        name: "bogus_tool".to_string(),
                        arguments: "{}".to_string(),
                    },
                }],
                tool_call_id: None,
                name: None,
            })
        } else {
            Ok(Message {
                role: "assistant".to_string(),
                content: Some("<THINKING>otra vez</THINKING>listo".to_string()),
                tool_calls: Vec::new(),
                tool_call_id: None,
                name: None,
            })
        }
    }
    fn name(&self) -> &str {
        "scripted"
    }
    fn model(&self) -> &str {
        "scripted-1"
    }
}

#[tokio::test]
async fn run_chat_dispatches_by_name_and_strips_thinking() {
    let provider = ScriptedProvider {
        turn: AtomicUsize::new(0),
    };
    let handler: ToolHandler = Box::new(|name: String, _args: String| {
        Box::pin(async move {
            if name == "seele_search" {
                Ok("[]".to_string())
            } else {
                Err(format!("unknown tool '{name}'"))
            }
        })
    });

    let history = run_chat(
        &provider,
        &handler,
        vec![user("hola")],
        &ChatConfig::default(),
    )
    .await
    .expect("loop completes");

    // Hallucinated tool name → in-band tool error, not a misparse.
    let tool_msg = history
        .iter()
        .find(|m| m.role == "tool")
        .expect("tool turn");
    assert!(
        tool_msg
            .content
            .as_deref()
            .unwrap()
            .contains("unknown tool 'bogus_tool'"),
        "got: {:?}",
        tool_msg.content
    );

    // Think-blocks never enter history (server-side strip).
    for m in history.iter().filter(|m| m.role == "assistant") {
        let c = m.content.clone().unwrap_or_default();
        assert!(
            !c.to_lowercase().contains("<think"),
            "thinking leaked into history: {c}"
        );
    }
    assert_eq!(history.last().unwrap().content.as_deref(), Some("listo"));
}
