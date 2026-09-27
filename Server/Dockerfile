# syntax=docker/dockerfile:1
# Build: docker build --target api -t healoo/care-api .   (targets: api, worker)
FROM rust:1.94-slim AS build
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential cmake pkg-config perl clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*
RUN apt-get update && apt-get install -y --no-install-recommends cmake pkg-config libssl-dev libsasl2-dev zlib1g-dev libcurl4-openssl-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY care-api/Cargo.toml care-api/Cargo.lock* ./care-api/
COPY care-api/src ./care-api/src
WORKDIR /src/care-api
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/care-api/target \
    cargo build --release --locked 2>/dev/null || cargo build --release; \
    mkdir -p /out && cp target/release/care-api target/release/care-worker target/release/care-seed target/release/devtoken /out/

FROM debian:trixie-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libsasl2-2 libcurl4 curl && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 healoo && mkdir -p /data && chown healoo /data
COPY --from=build /out/ /usr/local/bin/
USER healoo
WORKDIR /data
ENV FILES_DIR=/data/files TLS_DIR=/data/tls

FROM runtime AS api
EXPOSE 8080 8443
HEALTHCHECK --interval=15s --timeout=3s CMD curl -fsk http://localhost:8080/healthz || curl -fsk https://localhost:8443/healthz || exit 1
CMD ["care-api"]

FROM runtime AS worker
CMD ["care-worker"]
