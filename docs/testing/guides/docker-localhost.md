# SEELE — Docker localhost testing guide

**Goal:** run SEELE in a **pristine container** to validate the embedder's
first-run model download from Hugging Face — i.e. prove the `hf-hub` `0.3 → 0.5`
bump fixed the relative-`307` redirect (`RelativeUrlWithoutBase`) that broke
the cold download of `all-MiniLM-L6-v2`.

A fresh container is the strongest possible test: **no seeded `~/.seele`
cache, no PATH binary, no host state**. `seele serve` builds the embedder at
startup, so a cold boot is *forced* to download the model over TLS through the
new `ureq 3+` stack. If it downloads and serves real semantic search, the fix
holds end-to-end.

> Branch under test: `fix/hf-hub-bump`. These files live at the repo root:
> [`Dockerfile`](../../../Dockerfile), [`docker-compose.yml`](../../../docker-compose.yml),
> [`.dockerignore`](../../../.dockerignore).

---

## 0. TL;DR pass criteria

| # | Check | Pass when |
|---|---|---|
| 1 | Cold boot downloads the model | logs show the HF download + `seele http listening on http://0.0.0.0:7777`, **no** `ONNX embedder unavailable … falling back to FakeEmbedder` line |
| 2 | Active embedder is real | `GET /embedder` → `model_id` does **not** contain `fake`; `dim` = `384` |
| 3 | Model files persisted | the named volume holds `…/blobs/*` + `…/snapshots/*/onnx/model.onnx` + `tokenizer.json` |
| 4 | HTTP API serves | `GET /health` → `200`; `POST /memories` → `201` with an `id` |
| 5 | Real semantic search | a paraphrase ranks above an unrelated note **and** hits carry a `vec_rank` |
| 6 | Warm restart, no re-download | second `up` reuses the cached model (no download in logs) |

If all six pass on this branch (and #1/#2 would **fail** on `main` with hf-hub
0.3.2), the bump is verified and safe to merge.

---

## 1. Prerequisites

- Docker Engine / Desktop running (tested on `29.x`).
- Outbound HTTPS to `huggingface.co` + its CDN (the download under test).
- Free TCP port `7777` on the host (change the left side of the `ports`
  mapping if taken).
- ~2 GB disk for the build + image, ~90 MB for the model.

**Windows / PowerShell gotcha #1 — the `curl` alias:** in PowerShell `curl`
is an alias for `Invoke-WebRequest`, which has different flags. Use
**`curl.exe`** explicitly for the host-side examples below, or run the `curl`
examples *inside* the container with `docker compose exec`. All `docker`
commands are identical across shells.

**PowerShell gotcha #2 — JSON bodies:** PowerShell strips embedded
double-quotes when forwarding args to a native exe, so an inline
`-d '{"k":"v"}'` reaches `curl.exe` as `{k:v}` and the server rejects it
(`Failed to parse the request body as JSON: key must be a string`). For every
`POST` below, **write the JSON to a file and pass `-d "@body.json"`**
(PowerShell: `[IO.File]::WriteAllText("body.json", '{"k":"v"}')` — note: not
`Set-Content -Encoding utf8`, which adds a BOM that also breaks the parse), or
run the commands from Git Bash / WSL where inline JSON quoting works.

---

## 2. Build the image

```bash
cd C:/dev/tools/SEELE
docker compose build
```

Multi-stage build:

- **builder** (`rust:1-bookworm`): `cargo build --release --bin seele`. ort's
  `download-binaries` feature fetches ONNX Runtime here; `native-tls` (hf-hub's
  ureq backend) needs `pkg-config` + `libssl-dev`, installed in the stage.
- **runtime** (`debian:bookworm-slim`): just the binary + `ca-certificates`,
  `libssl3` (TLS to HF), `libstdc++6` + `libgomp1` (ONNX Runtime), `curl`
  (healthcheck). Any dynamic `libonnxruntime*.so` is copied from the builder.

First build is slow (full release compile of the workspace). Re-builds reuse
layers unless `Cargo.toml`/`Cargo.lock`/sources change.

---

## 3. Test 1 — Cold-start model download (the redirect fix)

Start from a **clean slate** so the embedder cache is empty:

```bash
docker compose down -v          # -v drops the seele-data volume (cache + db)
docker compose up               # foreground, so you watch the boot live
```

**What to watch in the logs, in order:**

1. Hugging Face download progress (indicatif bars / byte counts) for
   `model.onnx` + `tokenizer.json`.
2. `seele http listening on http://0.0.0.0:7777`.

**Must NOT appear:**

```
seele: warning — ONNX embedder unavailable (...); falling back to FakeEmbedder
```

That line is the failure signature — it's exactly what hf-hub 0.3.2 produced
when the relative `307` blew up (`RelativeUrlWithoutBase`). Its **absence** +
a successful download is the fix.

> On `main` (hf-hub 0.3.2) this same cold boot prints the fallback warning and
> silently degrades to `FakeEmbedder`. That contrast *is* the regression test.

Leave it running (or `docker compose up -d` to detach). Wait for healthy:

```bash
docker compose ps              # STATUS should read "healthy" (start_period 180s)
```

---

## 4. Test 2 — Confirm the active embedder is REAL

**Over HTTP (from the host):**

```bash
curl.exe -s http://localhost:7777/embedder
```

Expected (real ONNX):

```json
{ "model_id": "sentence-transformers/all-MiniLM-L6-v2", "dim": 384, ... }
```

Pass = `model_id` does **not** contain the substring `fake`, and `dim` is
`384`. A `seele/fake-embedder` here means the download failed and the server
fell back — investigate (see Troubleshooting).

**Via the binary (inside the container):**

```bash
docker compose exec seele seele --db /data/seele.db doctor --json
```

Expected fields:

```json
{
  "status": "ok",
  "embedder_model_id": "sentence-transformers/all-MiniLM-L6-v2",
  "embedder_dim": 384,
  "fake_embedder_warning": null,
  ...
}
```

`fake_embedder_warning: null` is the binary-side confirmation.

---

## 5. Test 3 — Model artifacts landed in the volume

```bash
docker compose exec seele sh -c "find /data/embedder -maxdepth 5 -type f | sort"
```

Expect the HF cache layout under
`/data/embedder/models--sentence-transformers--all-MiniLM-L6-v2/`:

- `refs/main`
- `blobs/<sha…>` (the actual bytes)
- `snapshots/<commit>/onnx/model.onnx`
- `snapshots/<commit>/tokenizer.json`

Non-empty `blobs/` = real bytes were downloaded into this container.

---

## 6. Test 4 — HTTP API smoke tests

Public routes (no auth):

```bash
curl.exe -s http://localhost:7777/health      # -> {"status":"ok",...}  (200)
curl.exe -s http://localhost:7777/version     # -> {"version":"0.2.0",...}
curl.exe -s http://localhost:7777/openapi.json | more   # OpenAPI spec; Swagger UI at /docs
```

Create + read a memory (canonical SEELE routes). Body via file (see gotcha #2):

```powershell
# PowerShell — write a no-BOM body file, then POST it.
[IO.File]::WriteAllText("$PWD\body.json", '{"title":"WAL recovery","content":"Postgres uses a write-ahead log for crash recovery.","type":"learning","project":"docker-smoke"}')

# POST /memories -> 201 {"id":"01...","outcome":"created"}
curl.exe -s -X POST http://localhost:7777/memories -H "Content-Type: application/json" -d "@body.json"

# GET /memories?project=docker-smoke -> the row back
curl.exe -s "http://localhost:7777/memories?project=docker-smoke"
```

```bash
# bash / WSL — inline JSON works.
curl -s -X POST http://localhost:7777/memories -H "Content-Type: application/json" \
  -d '{"title":"WAL recovery","content":"Postgres uses a write-ahead log for crash recovery.","type":"learning","project":"docker-smoke"}'
```

> Request body fields match `SaveRequest` (`title`, `content`, `type`,
> `project`, `scope`, `topic_key`, `metadata`). `type` defaults to `memory`.

---

## 7. Test 5 — Real semantic search (the embedder actually embeds)

This separates a real embedder from the fake one: with real vectors, a
**paraphrase** retrieves the original and the hit carries a populated
`vec_rank`; the fake embedder produces hash-deterministic vectors with no
semantic ordering.

```powershell
# PowerShell — body files (gotcha #2).
[IO.File]::WriteAllText("$PWD\wal.json",   '{"title":"WAL","content":"Postgres uses a write-ahead log for crash recovery.","project":"sem"}')
[IO.File]::WriteAllText("$PWD\beach.json", '{"title":"Beach","content":"Tropical beaches in Bora Bora are beautiful.","project":"sem"}')
[IO.File]::WriteAllText("$PWD\q.json",     '{"query":"how does PostgreSQL recover after a crash","project":"sem","limit":5}')

curl.exe -s -X POST http://localhost:7777/memories -H "Content-Type: application/json" -d "@wal.json"
curl.exe -s -X POST http://localhost:7777/memories -H "Content-Type: application/json" -d "@beach.json"
# Query a paraphrase that shares no distinctive keywords with the WAL note:
curl.exe -s -X POST http://localhost:7777/search  -H "Content-Type: application/json" -d "@q.json"
```

**Pass criteria** on the `/search` response (`{ "hits": [...], "count": N }`):

- The WAL note ranks **above** the Bora Bora note.
- Every hit carries a non-null `vec_rank` (vector retrieval contributed —
  impossible with meaningful ordering under `FakeEmbedder`).

Verified output (real embedder, this branch):

```json
{"hits":[
  {"id":"01K…MNF","title":"WAL","project":"sem","type":"memory","score":0.0163,"vec_rank":1, …},
  {"id":"01K…R5Q","title":"Beach","project":"sem","type":"memory","score":0.0161,"vec_rank":2, …}
],"count":2}
```

WAL (`vec_rank:1`) outranks Beach (`vec_rank:2`) on a keyword-free paraphrase —
semantic retrieval is live.

---

## 8. Test 6 — Warm restart (cache reuse, no re-download)

```bash
docker compose restart           # or: down (without -v) then up
docker compose logs --since 2m seele
```

Pass = the server comes back up and listens **without** a new HF download in
the logs (the model is read from the persisted `seele-data` volume). This
proves the cache path works too, not just the cold download.

To force a cold download again, drop the volume: `docker compose down -v`.

---

## 9. Test 7 — FakeEmbedder contrast (optional, negative control)

Run the same image with the embedder forced fake to *see* the failure
signature on purpose:

```bash
docker compose run --rm -e SEELE_FAKE_EMBEDDER=1 seele --db /data/seele.db doctor --json
# -> embedder_model_id "seele/fake-embedder", fake_embedder_warning NON-null
```

This is what a broken download *looks like* downstream — useful to confirm
your Test 2 pass is meaningful.

---

## 10. Teardown

```bash
docker compose down              # stop + remove the container (keeps the volume)
docker compose down -v           # also delete the model cache + db volume
docker image rm seele:hf-hub-test    # optional: reclaim the image
```

---

## 11. Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `error while loading shared libraries: libonnxruntime.so…` | ort linked dynamically and the `.so` wasn't staged | confirm the builder's `find target/release -name 'libonnxruntime*.so*'` found it; it's copied to `/usr/local/lib` + `ldconfig`. Rebuild `--no-cache` if stale. |
| Fallback to `FakeEmbedder` on cold boot | no network, TLS trust, or (the bug under test) a redirect failure | check outbound HTTPS to `huggingface.co`; confirm `ca-certificates` + `libssl3` are installed in runtime; on `main` this is expected (hf-hub 0.3.2 bug). |
| `tls handshake` / certificate errors | missing CA bundle | ensure runtime stage installed `ca-certificates` (it does); corporate MITM proxies need their root CA added. |
| Healthcheck stuck `starting`/`unhealthy` | first download still running past `start_period` | the model is ~90 MB; raise `start_period` in `docker-compose.yml` on slow links, or watch `docker compose logs -f`. |
| `bind: address already in use` (7777) | host port taken | change the mapping to e.g. `"8777:7777"` and use that port from the host. |
| PowerShell `curl` returns an `Invoke-WebRequest` object | `curl` alias | use `curl.exe`, or `docker compose exec seele curl -s http://127.0.0.1:7777/health`. |

---

## 12. What this validates for the merge

- **Root cause closed:** hf-hub 0.5.0's `ureq 3+` follows HF's relative `307`;
  the cold download completes where 0.3.2 raised `RelativeUrlWithoutBase`.
- **No code change required:** the `hf_hub::api::sync` surface is unchanged;
  only `Cargo.toml`/`Cargo.lock` moved.
- **No regression:** real semantic search works end-to-end inside a container
  that never saw a seeded cache.

Once Tests 1–6 pass here, merge `fix/hf-hub-bump` → `main` and (optionally)
rebuild + reinstall the PATH binary so day-to-day `seele` uses the fixed
downloader instead of relying on a previously-seeded cache.
