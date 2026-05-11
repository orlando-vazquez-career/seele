# ADR-07 — TUI con ratatui + crossterm

**Estado**: Aceptado · 2026-05-09
**Decisión**: TUI built-in en v0.1 (decisión User el 2026-05-09) usando `ratatui` 0.29+ con backend `crossterm`. Vistas Home / Browse / Search / Detail / Stats. Keybindings vi-style con discoverability.

## Contexto

El User pidió incluir TUI desde v0.1 para dogfooding inmediato — inspeccionar el state de SEELE sin SQL ni curl.

Ejecuta con:

```bash
seele tui
# o
seele
# (sin args invoca TUI por default)
```

## Vistas

### Home

```
┌─ SEELE v0.1.0 ──────────────────────────────────────────────────┐
│                                                                  │
│   ╔═══════════════════════════════════════════════════════════╗ │
│   ║  📊 Stats                                                  ║ │
│   ║    Total memories:  12,453                                 ║ │
│   ║    Active:          12,201                                 ║ │
│   ║    Soft-deleted:       252                                 ║ │
│   ║    DB size:           18.4 MB                              ║ │
│   ║    Last save:        2 min ago                             ║ │
│   ╚═══════════════════════════════════════════════════════════╝ │
│                                                                  │
│   By kind:                       By domain:                     │
│     decision      4,201            mnema           3,121         │
│     memory        3,892            dev-zen          2,892         │
│     skill         2,103            client-acme     1,520         │
│     advisor_out   1,557            (others)        4,920         │
│     verdict       700                                            │
│                                                                  │
│                                                                  │
│ [s] search   [b] browse   [a] axiomatic   [q] quit              │
└──────────────────────────────────────────────────────────────────┘
```

### Browse

```
┌─ Browse ──────────────────── filter: kind=decision · domain=* ──┐
│                                                                  │
│  > 01HW3X4Y5Z...  decision  dev-zen        "rate limit retry"   │
│    01HW3X4Y5A...  decision  dev-zen        "stripe webhook"     │
│    01HW3X4Y5B...  decision  mnema         "engine vs upstream" │
│    01HW3X4Y5C...  decision  client-acme   "neutral spanish"    │
│    01HW3X4Y5D...  skill     dev-zen        "i18n parity guard"  │
│    ...                                                           │
│                                                                  │
│  [↑↓/jk] move  [enter] open  [/] filter  [n] new  [d] delete    │
│  [a] toggle axiomatic  [esc] back                               │
└──────────────────────────────────────────────────────────────────┘
```

### Search

```
┌─ Search ─────────────────────────────────────────────────────────┐
│                                                                  │
│  Query: rate limit retry policy█                                │
│  Filters: kind=decision                                         │
│                                                                  │
│  Results (live, RRF):                                           │
│    > 01HW3X...  ★1.0  decision  dev-zen                         │
│      "We will retry rate limited requests with exp backoff..."  │
│                                                                  │
│      01HW3Y...  ★0.7  decision  dev-zen                         │
│      "Stripe API has rate limits at 100 req/min..."            │
│                                                                  │
│      01HW3Z...  ★0.4  memory    dev-zen                         │
│      "Discussion on retry policy with team"                     │
│                                                                  │
│  [↑↓] move  [enter] open  [esc] back  [/] edit query           │
└──────────────────────────────────────────────────────────────────┘
```

### Detail

```
┌─ Memory 01HW3X4Y5Z ──────────────────────────────────────────────┐
│                                                                  │
│  Created:    2026-05-09 14:23:45                                │
│  Updated:    2026-05-09 14:23:45                                │
│  Kind:       decision                                            │
│  Domain:     dev-zen                                              │
│  Score:      1.0  axiomatic: false                               │
│                                                                  │
│  Body:                                                           │
│  ┌────────────────────────────────────────────────────────────┐ │
│  │ We will retry rate-limited requests from the upstream API │ │
│  │ exponential backoff (initial 1s, max 60s, 5 retries).      │ │
│  │ This balances reliability vs API quota. Decision made by   │ │
│  │ the team after vrd_01HX... counsel.                        │ │
│  └────────────────────────────────────────────────────────────┘ │
│                                                                  │
│  Metadata:                                                       │
│    { "kind": "decision", "domain": "dev-zen     ",               │
│      "tags": ["api", "stripe", "rate-limit"],                   │
│      "verdict_id": "vrd_01HX...", "score": 1.0 }                │
│                                                                  │
│  Linked memories (3):                                           │
│    derives_from   01HW3W...  "Initial Stripe integration"      │
│    related_to     01HW3V...  "Stripe rate limit observed"      │
│    evidence_for   01HW3U...  "Retry policy in repo"            │
│                                                                  │
│ [a] toggle axiomatic  [d] delete  [l] add link  [esc] back      │
└──────────────────────────────────────────────────────────────────┘
```

## Keybindings globales

```
?       help (overlay con todos los keybindings)
q       quit
esc     back / cancel
1       go home
2       go browse
3       go search
:       command mode (typing comandos como vim)
```

## Keybindings por vista

### Browse

```
↑ ↓ k j     move selection
g G         top / bottom
/           filter (open prompt)
n           new memory (open editor)
d           soft delete current (with confirm)
a           toggle axiomatic flag
enter       open detail
```

### Search

```
typing      append to query (no live debounce in v0.1; see note below)
enter       run search
↑ ↓         move selection in result list
esc         clear query when non-empty, else back to Browse
```

**v0.1 note (2026-05-11):** the originally specified live-debounced
search was deferred to Sprint-05, when the real ONNX embedder becomes
the default backend. With `FakeEmbedder` the cost of running a query
per keystroke is negligible but the result is non-meaningful, so the
explicit Enter trip is the better UX for now. Cloven flagged the
spec/code drift on 2026-05-11.

### Detail

```
e           edit body (open $EDITOR)
m           edit metadata (open $EDITOR with JSON)
a           toggle axiomatic
d           soft delete (confirm)
l           add link (prompt)
L           remove link (prompt)
y           yank ID to clipboard
esc         back
```

## Layout strategy

`ratatui` layouts con `Layout::default().direction(Direction::Vertical)` etc. Footer hint bar siempre visible (1 línea), header con app name + version (1 línea), main content fills middle.

Resize-aware: la TUI re-renders en cada `Resize` event de crossterm.

## Async + event loop

```rust
async fn run_tui() -> Result<()> {
    let mut terminal = ratatui::init();
    let mut state = AppState::new();
    let mut event_stream = crossterm::event::EventStream::new();

    loop {
        terminal.draw(|f| render(f, &state))?;

        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(Ok(Event::Key(key))) => {
                        if let Some(action) = handle_key(&state, key) {
                            apply_action(&mut state, action).await?;
                        }
                    }
                    _ => {}
                }
            }
            tick = tokio::time::sleep(Duration::from_millis(200)) => {
                state.tick();
            }
        }

        if state.should_quit { break; }
    }

    ratatui::restore();
    Ok(())
}
```

## Theming

v0.1: dark theme único, hardcoded. Colors:

- Bg: black
- Fg primary: white
- Accent: cyan (selection, focused)
- Score badge: green (high) → yellow (mid) → grey (low)
- Axiomatic: gold

v0.2: themes via config (light, custom).

## Editing externo

Para edit body o metadata, abrir `$EDITOR` con archivo temporal:

```rust
fn open_editor(initial_content: &str) -> Result<String> {
    let mut tmp = tempfile::NamedTempFile::new()?;
    tmp.write_all(initial_content.as_bytes())?;
    let path = tmp.path().to_path_buf();

    // Suspend ratatui
    crossterm::execute!(stdout(), LeaveAlternateScreen)?;
    disable_raw_mode()?;

    // Spawn editor
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
    Command::new(&editor).arg(&path).status()?;

    // Resume ratatui
    enable_raw_mode()?;
    crossterm::execute!(stdout(), EnterAlternateScreen)?;

    Ok(std::fs::read_to_string(&path)?)
}
```

Soporta vim, nano, code (cualquier $EDITOR). Default `vi` en Linux/Mac, `notepad` en Windows.

## Performance

- Render < 16ms (60 fps target).
- Search live debounced 200ms para no saturar embedder.
- Browse paginate cada 50 items (DB query con LIMIT).

## Testing

- Snapshot tests con `insta` y `ratatui::Terminal::backend(TestBackend)`.
- Property tests para state transitions (proptest).
- Manual smoke test pre-release.

## Out of scope v0.1

- **Mouse support**: keyboard only.
- **Visualización gráfica de links**: v0.2 con `petgraph` + ASCII rendering.
- **Multi-pane / tabs**: v0.2.
- **Theming custom**: v0.2.
- **Undo stack**: v0.3.
- **Plugins / extensions**: nunca.

## Referencias

- ratatui showcase: https://ratatui.rs/showcase/
- crossterm event loop patterns.
- vim keybinding philosophy aplicada a TUI tools (lazygit, k9s, atuin).
