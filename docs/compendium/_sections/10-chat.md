## 10. Multi-Provider Chat Backend — `seele-chat`

`seele-chat` is the smallest non-trivial crate in the workspace and the only one that talks to *remote* LLM APIs. Its single job is the **chat-with-DB** feature surfaced on the web observability page: a user asks a natural-language question, a remote model decides when to call a search tool, the server runs the search against the local SQLite memory, feeds the results back to the model, and the model produces a grounded answer. The crate owns the provider wire formats and the tool-use loop; it owns nothing about *what* the tools do. Per its own doc comment, "The crate is intentionally storage-agnostic: tool execution is supplied by the caller via a [`ToolHandler`] closure, so `seele-chat` can be reused by other binaries that bind their own tools" (`crates/seele-chat/src/lib.rs:7-9`).

### Position in the crate graph

`seele-chat` is a **leaf with no internal dependencies** — its `Cargo.toml` lists only `tokio`, `async-trait`, `reqwest`, `serde`, `serde_json`, `thiserror`, and `tracing` (`crates/seele-chat/Cargo.toml:11-18`). It does not depend on `seele-core`, `seele-storage`, or `seele-search`; its message and tool types are defined locally rather than reusing domain types, which is what keeps it storage-agnostic. The single Rust consumer is **`seele-http`** (`crates/seele-http/Cargo.toml:15` → `seele-chat = { path = "../seele-chat" }`), which wires the search tool to its own `SeeleService`. The whole crate lives in one file, `lib.rs` (476 lines). Note that `seele-mcp` also defines a type literally named `ToolHandler` (`crates/seele-mcp/src/tools.rs:60`, a `fn(&SeeleService, Value) -> Result<Value, ToolError>` pointer), but that is an unrelated, synchronous tool dispatcher and `seele-mcp` does **not** depend on `seele-chat`.

### Public API surface

| Item | Kind | Purpose |
|------|------|---------|
| `Message` | struct | One conversation turn (role/content/tool_calls/tool_call_id/name) |
| `ToolCall` | struct | A structured tool invocation requested by the assistant |
| `ToolCallFunction` | struct | `{ name, arguments }` — arguments are a JSON-encoded *string* |
| `ChatProvider` | `#[async_trait]` trait | Provider abstraction: `complete`, `name`, `model` |
| `ToolSpec` | struct | A tool advertised to the model (`name`, `description`, JSON-schema `parameters`) |
| `ToolHandler` | type alias | Boxed async closure `Fn(String) -> Future<Result<String, String>>` |
| `ChatConfig` | struct | `system_prompt` + `max_iterations` for one run |
| `OpenAICompatibleProvider` | struct + impl | `/v1/chat/completions` provider |
| `AnthropicProvider` | struct + impl | `/v1/messages` provider |
| `run_chat` | async fn | The tool-use orchestrator loop |
| `ChatError` | enum | All failure modes |

#### `Message` and its serde shape

`Message` (`crates/seele-chat/src/lib.rs:39-50`) is the canonical wire type and is **shared between request and response** — the same struct is sent to the provider and deserialized back from it. Field-by-field:

| Field | Type | serde attribute | Meaning |
|-------|------|-----------------|---------|
| `role` | `String` | (required) | `"system"`, `"user"`, `"assistant"`, or `"tool"` |
| `content` | `Option<String>` | `default`, `skip_serializing_if = "Option::is_none"` | text body; absent on pure tool-call turns |
| `tool_calls` | `Vec<ToolCall>` | `default`, `skip_serializing_if = "Vec::is_empty"` | calls requested by an assistant turn |
| `tool_call_id` | `Option<String>` | `default`, skip-if-none | on a `"tool"` turn, the call it answers |
| `name` | `Option<String>` | `default`, skip-if-none | tool name on a `"tool"` turn |

The `skip_serializing_if` attributes are load-bearing: they keep the OpenAI request body clean (e.g. a plain user message serializes to just `{"role":"user","content":"…"}`) so strict providers do not reject unexpected `null` fields. `role` is a plain `String`, not an enum — the crate never validates it, trusting both the caller and the provider to use legal values.

`ToolCall` (`:52-58`) holds `id: String`, `r#type: String` (defaulting to `"function"` via the `default_tool_type` fn at `:60-62`, so a provider that omits `type` still deserializes), and `function: ToolCallFunction`. `ToolCallFunction` (`:64-69`) is `{ name: String, arguments: String }` where `arguments` is, per the comment, a "JSON-encoded arguments string (OpenAI convention)" — i.e. the model's structured input arrives as an opaque string the caller must parse.

#### `ChatProvider`, `ToolSpec`, `ToolHandler`, `ChatConfig`

The trait (`:72-78`) is the polymorphism seam:

```rust
// crates/seele-chat/src/lib.rs:72-78
#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(&self, messages: &[Message]) -> Result<Message, ChatError>;
    fn name(&self) -> &str;
    fn model(&self) -> &str;
}
```

`#[async_trait]` (the `async-trait 0.1` crate) is required because Rust's native `async fn` in traits cannot be made into a `dyn` trait object, and `run_chat` takes `provider: &dyn ChatProvider`. The bound `Send + Sync` lets the provider be shared across tokio tasks.

`ToolSpec` (`:83-88`) carries `name`, `description`, and a `serde_json::Value` `parameters` holding the input JSON-schema. `ToolHandler` (`:91-93`) is the storage-agnostic hook:

```rust
// crates/seele-chat/src/lib.rs:91-93
pub type ToolHandler = Box<
    dyn Fn(String) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send>> + Send + Sync,
>;
```

This hand-rolled boxed-future signature (rather than `async_trait`) is what decouples the crate from any storage: the loop hands the handler the raw `arguments` JSON string and gets back either an `Ok(result_string)` to feed the model or an `Err(message)` that the loop turns into a tool-error string (it never aborts the loop). Importantly, the handler receives **only** `arguments` — not the tool name — so a multi-tool caller must dispatch on the parsed args itself. In practice the HTTP layer binds exactly one tool (`seele_search`), sidestepping that limitation.

`ChatConfig` (`:96-108`) holds a `system_prompt` and a `max_iterations: u32`. Its `Default` (`:101-108`) sets `max_iterations: 5` (line `:105`) and a baked English prompt instructing the assistant to use `seele_search`, "Cite retrieved memories by `id` and `project`," and "Respond in the language of the user's question." (full string at `:104`).

#### `ChatError` variants

| Variant | `#[from]` | When |
|---------|-----------|------|
| `Http(reqwest::Error)` | yes | transport/connection failure |
| `ProviderStatus { status: u16, body: String }` | no | non-2xx HTTP status; body captured for diagnostics |
| `Parse(serde_json::Error)` | yes | response body failed to deserialize |
| `Tool(String)` | no | tool-layer error; also reused for "provider returned zero choices" |
| `LoopBudget { max_iterations: u32 }` | no | tool-use loop hit the iteration cap |
| `UnsupportedProvider(String)` | no | declared but **never constructed** anywhere in the crate |

Defined at `:19-33`. `UnsupportedProvider` is dead surface today — provider selection happens in the caller (`seele-http`), which treats any non-`anthropic` label as OpenAI-compatible and so never needs to reject one.

### OpenAICompatibleProvider

`OpenAICompatibleProvider` (`:114-121`) holds `provider_name`, `endpoint`, `api_key`, `model`, `tools: Vec<ToolSpec>`, and a private `reqwest::Client` (one client per provider instance — providers are short-lived, built per request by the HTTP handler). `new` (`:123-139`) takes `impl Into<String>` for ergonomics. The crate-level doc comment lists Minimax, OpenAI, OpenRouter, Together, Groq, DeepSeek as targets (`:3-4`); the section banner comment above the struct (`:110-112`) repeats the same list but truncates it with "…" (it does not spell out DeepSeek).

`complete` (`:170-207`):
1. Builds `{ "model", "messages", "stream": false }`. `messages` serializes the `&[Message]` slice directly — the OpenAI schema *is* the internal schema, so no translation step exists.
2. If `tools` is non-empty, adds `tools` (via `tools_json`, `:141-157`, which wraps each `ToolSpec` as `{"type":"function","function":{name,description,parameters}}`) and `"tool_choice":"auto"`.
3. POSTs with `bearer_auth(&self.api_key)` (i.e. `Authorization: Bearer …`).
4. On non-success status, reads the body text (`unwrap_or_default()`) and returns `ProviderStatus`.
5. Deserializes into the private `OpenAIResponse { choices: Vec<OpenAIChoice> }` (`:210-218`), takes the first choice's `message`, or errors with `ChatError::Tool("provider returned zero choices")` if `choices` is empty. Only the first choice is ever used.

### AnthropicProvider

`AnthropicProvider` (struct `:224-230`, `new` `:232-242`) hard-codes `endpoint = "https://api.anthropic.com/v1/messages"` in `new` (`:237`), so unlike the OpenAI path there is no endpoint override. Its `complete` (`:254-302`) performs a real **schema translation** because `/v1/messages` differs from `/v1/chat/completions`:

- `split_system` (`:304-320`) extracts all `"system"` messages, concatenates their `content` with newlines into a top-level `system` string (Anthropic carries system text out of the message array).
- `anthropic_translate` (`:322-361`) maps each remaining message:
  - a `"tool"` message becomes a `"user"` message containing a `tool_result` block keyed by `tool_use_id` (from `tool_call_id`);
  - an assistant turn with `tool_calls` becomes content **blocks**: an optional `text` block plus one `tool_use` block per call, where `arguments` is parsed from string back into a JSON object (`serde_json::from_str(...).unwrap_or(json!({}))` at `:343-344` — malformed args silently degrade to `{}`);
  - anything else becomes `{ role, content }`.
- The request adds `"max_tokens": 2048` (hard-coded, `:272`) and tools as `{name, description, input_schema}` (note the key is `input_schema`, vs OpenAI's nested `parameters`). Auth uses headers `x-api-key`, `anthropic-version: 2023-06-01`, and `content-type: application/json` (`:283-285`) — **not** bearer auth.
- The response (`AnthropicResponse { content: Vec<AnthropicContent> }`, `:363-379`) is an internally-tagged enum (`#[serde(tag = "type", rename_all = "snake_case")]`) over `Text` and `ToolUse`. `anthropic_to_internal` (`:381-410`) folds it back to a single internal assistant `Message`: text parts are joined with `\n` (or `None` if empty), and each `tool_use` becomes a `ToolCall` whose `arguments` is `input.to_string()` (re-serialized JSON), restoring the OpenAI string convention.

This round-trip is the crux of multi-provider support: both providers ingest and emit the *same* internal `Message`, so `run_chat` is provider-agnostic.

### The tool-use loop: `run_chat` and `LoopBudget`

```rust
// crates/seele-chat/src/lib.rs:416-421
pub async fn run_chat(
    provider: &dyn ChatProvider,
    tool_handler: &ToolHandler,
    user_messages: Vec<Message>,
    config: &ChatConfig,
) -> Result<Vec<Message>, ChatError>
```

Control flow (`:416-476`):
1. Seed `history` with a `system` message from `config.system_prompt`, then append the caller's `user_messages` (`:422-430`).
2. Loop `for iteration in 0..config.max_iterations` (`:439`):
   - call `provider.complete(&history)` and push the assistant reply into `history`;
   - if the reply has **no** `tool_calls`, the model is done — return the full `history` (success, `:443-446`);
   - otherwise, for each tool call, invoke `tool_handler(tc.function.arguments.clone()).await`. On `Ok`, use the result; on `Err(e)`, synthesize `format!("(tool error) {}", e)` (`:457`) so the model can recover rather than the run aborting. Push each result as a `"tool"` message carrying `tool_call_id` and `name` (`:459-465`), then loop again.
3. If the loop exits without an answer, log a `warn!` (`:469-472`) and return `Err(ChatError::LoopBudget { max_iterations })`.

`LoopBudget` (default 5 iterations) is the safety cap that prevents an infinite request→tool→request cycle from a model that keeps calling tools and never settles, bounding both cost and latency. Tool calls within one iteration are executed **sequentially** (a `for` loop with `.await`, `:453-466`), not concurrently. The function returns the *entire* `history` including the prepended system turn and all intermediate tool turns — the caller is responsible for trimming for display.

### Concurrency, async, error handling

Everything is `async` over tokio + `reqwest`. There is no shared mutable state and no locking inside the crate; concurrency safety comes from the `Send + Sync` bounds on `ChatProvider` and `ToolHandler`. Errors are typed via `thiserror`; the two `#[from]` conversions (`reqwest::Error`, `serde_json::Error`) let `?` propagate transport and parse failures, while status and budget errors are constructed explicitly. The crate logs via `tracing` at `debug`/`info`/`warn` (e.g. `info!` at loop start `:432-437`, `debug!` per-iteration `:444`/`:448-452`, `warn!` on budget exhaustion `:469-472`) but never `error!`.

### How it connects to the rest of SEELE

The sole binding lives in `crates/seele-http/src/handlers.rs`, which imports the chat types as `use seele_chat::{ AnthropicProvider, ChatConfig, ChatProvider, Message as ChatMessage, OpenAICompatibleProvider, ToolHandler, ToolSpec }` (`handlers.rs:200-203`) — note `Message` is locally aliased to `ChatMessage`. `ChatProviderConfig` (`crates/seele-http/src/server.rs:61-73`) carries `provider`, `api_key`, `model`, and optional `endpoint`, and is stored in `AppState.chat: Option<Arc<ChatProviderConfig>>` (`server.rs:30`), exposed to handlers through a `FromRef` impl (`server.rs:39-43`). Two routes are registered: `POST /chat` and `GET /chat/info` (`server.rs:151-152`), both inside the auth-protected router group. The CLI `seele serve` populates the config from `--chat-provider`/`--chat-key`/`--chat-model`/`--chat-endpoint` (clap flags at `crates/seele-cli/src/commands/serve.rs:28-46`; config assembled at `serve.rs:53-72`). `--chat-key` is resolved by `resolve_key` (`serve.rs:88-95`): if the value starts with `$`, the remainder is read from that env var, otherwise the value is used literally. `--chat-provider` and `--chat-key` must be supplied together or both omitted, else `serve` bails.

The `chat` handler (`handlers.rs:254-376`) resolves provider/key/model/endpoint in priority order **per-request override > CLI config** (`handlers.rs:261-288`), then:
- builds a single `ToolSpec` named `seele_search` with a query/limit/project JSON-schema (`required: ["query"]`) (`handlers.rs:290-302`);
- selects the provider — `AnthropicProvider` when the name equals `anthropic` (case-insensitive via `eq_ignore_ascii_case`), else `OpenAICompatibleProvider` with a `default_endpoint_for` fallback when no endpoint override is present (`handlers.rs:304-315`);
- constructs the `ToolHandler` closure (`handlers.rs:318-360`) that parses `{query, limit, project}`, clamps `limit` to `unwrap_or(5).min(20)` (default 5, max 20), builds a `SearchRequest`, calls `svc.search_observations(...)`, and returns a JSON summary `{query, count, results:[{id,type,title,project,score,snippet}]}` where `snippet` is the hit content truncated to the first 280 chars (`handlers.rs:355`) — this is the line where storage re-enters; the closure captures an `Arc<SeeleService>` clone (`handlers.rs:317-319`);
- applies an optional per-request `system_prompt` override onto `ChatConfig::default()` (`handlers.rs:362-365`);
- runs `seele_chat::run_chat(...)` and returns `ChatResponse { messages, provider, model }` (`messages` is `Vec<ChatMessage>`, i.e. `Vec<seele_chat::Message>`), mapping any `ChatError` to `ApiError::Internal` (`handlers.rs:367-375`).

Default models and endpoints for each provider family are tabulated in `default_model_for`/`default_endpoint_for` (`handlers.rs:378-401`; e.g. minimax → `MiniMax-M2` at `https://api.minimax.io/v1/chat/completions`, anthropic → `claude-haiku-4-5-20251001`, with `gpt-4o-mini` / `https://api.openai.com/v1/chat/completions` as the catch-all defaults). An identical `default_model_for` table is duplicated in the CLI at `serve.rs:97-108` (so the CLI can compute a default model before constructing `ChatProviderConfig`).

The browser side is `web/src/components/ChatPanel.astro`, embedded on `web/src/pages/observability.astro` (imported at `observability.astro:5`, rendered at `:35`). It probes `GET /chat/info` (`ChatPanel.astro:250`) to learn whether chat is enabled and which provider/model is set, stores the user's provider/key/model/endpoint in `localStorage` under the key `seele-chat-settings` (`ChatPanel.astro:164`; forwarded only to the local SEELE server, never a third party — per the doc-comment privacy note at `ChatPanel.astro:8-10`), and POSTs the running `history` to `/chat`. On reply it slices `data.messages.slice(history.length + 1)` to skip the server-prepended system prompt and renders the new turns (`ChatPanel.astro:385-393`). The panel also strips reasoning traces (`<think>`, `<thinking>`, `<|thinking|>`) in `stripThinking` (`ChatPanel.astro:476-487`) and applies light client-side markdown — bold and inline code — in `renderAssistantBody` (`ChatPanel.astro:489-498`), and shows a compact `N results` summary for tool turns (`ChatPanel.astro:452-466`).

### Edge cases, gotchas, invariants

- **`arguments` is always a string.** Both providers normalize tool input to a JSON-encoded string; callers must `serde_json::from_str` it. Anthropic's translation parses it back to an object and the response re-serializes it, so a malformed arguments string silently becomes `{}` (`lib.rs:343-344`) — a lossy edge.
- **Single tool name is invisible to the handler.** `ToolHandler` receives only `arguments`, so the design assumes one tool (or caller-side disambiguation). With the HTTP layer's single `seele_search` tool this is fine.
- **`UnsupportedProvider` is dead** — declared but never constructed (`lib.rs:31-32`).
- **No streaming.** OpenAI requests pin `"stream": false` (`lib.rs:174`); Anthropic hard-codes `"max_tokens": 2048` (`lib.rs:272`). Both are non-configurable from the public API.
- **Only the first choice** of an OpenAI response is read; `n`-completions are unsupported.
- **API keys never persist server-side** — `ChatProviderConfig`'s doc note (`server.rs:59-60`) states none of its fields ever leave the machine and "the API key stays on the box running `seele serve`"; per-request keys are documented on `ChatRequest.api_key` as used "once and discards it — never persisted" (`handlers.rs:214-215`).
- No `TODO`/`FIXME`/`XXX`/`HACK` markers exist in `crates/seele-chat/src/lib.rs`.
