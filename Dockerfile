# syntax=docker/dockerfile:1
#
# SEELE — Docker test image (localhost).
#
# Purpose: run SEELE in a PRISTINE container to validate the embedder's
# first-run model download from Hugging Face (the hf-hub 0.5 redirect fix).
# A fresh container has no seeded cache and no PATH binary, so `seele serve`
# is forced to download `all-MiniLM-L6-v2` on startup — the definitive test.
#
# Build:  docker compose build
# Run:    docker compose up
# See:    docs/testing/guides/docker-localhost.md

# ----------------------------------------------------------------------------
# Stage 1 — builder: compile the release binary + fetch ONNX Runtime.
# ----------------------------------------------------------------------------
FROM rust:1-bookworm AS builder
WORKDIR /build

# native-tls (used by hf-hub's ureq sync backend) needs OpenSSL headers at
# build time. ort's `download-binaries` feature fetches ONNX Runtime over the
# network during `cargo build`.
RUN apt-get update && apt-get install -y --no-install-recommends \
      pkg-config \
      libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the workspace. Heavy/irrelevant paths (target/, web/, .git/) are
# excluded via .dockerignore so the build context stays small.
COPY . .

# Build only the `seele` binary in release mode.
RUN cargo build --release --bin seele

# Stage the ONNX Runtime shared library if ort linked dynamically. If ort
# linked statically, /staged-libs ends up empty and the COPY below is a no-op.
RUN mkdir -p /staged-libs \
    && find target/release -name 'libonnxruntime*.so*' -exec cp -av {} /staged-libs/ \; \
    && ls -la /staged-libs/ || true

# ----------------------------------------------------------------------------
# Stage 2 — runtime: slim image with just the binary + its system deps.
# ----------------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates + libssl3 → TLS to huggingface.co (native-tls / ureq).
# libstdc++6 + libgomp1 → ONNX Runtime C++ + OpenMP.
# curl → healthcheck + manual smoke tests from inside the container.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      libssl3 \
      libstdc++6 \
      libgomp1 \
      curl \
    && rm -rf /var/lib/apt/lists/*

# ONNX Runtime shared lib (if dynamic). Made discoverable via ldconfig +
# LD_LIBRARY_PATH so the binary finds it regardless of rpath strategy.
COPY --from=builder /staged-libs/ /usr/local/lib/
RUN ldconfig
ENV LD_LIBRARY_PATH=/usr/local/lib

COPY --from=builder /build/target/release/seele /usr/local/bin/seele

# Single data dir for the SQLite DB + the embedder model cache. Mount a
# volume here (see docker-compose.yml) to persist across restarts — or start
# with it empty to reproduce the first-run download.
ENV SEELE_EMBEDDER_DIR=/data/embedder
ENV RUST_LOG=info
RUN mkdir -p /data

EXPOSE 7777

# ENTRYPOINT is the binary, so `docker compose run seele <subcommand>` works
# (e.g. `... run --rm seele --db /data/seele.db doctor --json`). The default
# CMD serves the HTTP API on all interfaces so the host can reach :7777.
ENTRYPOINT ["seele"]
CMD ["--db", "/data/seele.db", "serve", "--bind", "0.0.0.0", "--port", "7777"]
