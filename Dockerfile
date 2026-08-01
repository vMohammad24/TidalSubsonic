FROM lukemathwalker/cargo-chef:latest-rust-1-alpine AS chef
RUN apk add --no-cache musl-dev build-base cmake ninja
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --target x86_64-unknown-linux-musl --recipe-path recipe.json
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release --target x86_64-unknown-linux-musl --bin tss && \
    cp /app/target/x86_64-unknown-linux-musl/release/tss /app/tss

FROM alpine:3.24 AS runtime
RUN apk add --no-cache ca-certificates \
    && adduser -S -D -H -s /sbin/nologin appuser
WORKDIR /app

COPY --from=builder /app/tss /app/tss
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/templates /app/templates

ENV HOME=/tmp
USER appuser

ENV HOST=0.0.0.0
ENV PORT=3000
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=3s --start-period=20s --retries=3 \
    CMD wget -q -O /dev/null "http://127.0.0.1:${PORT}/health" || exit 1

ENTRYPOINT ["/app/tss"]
