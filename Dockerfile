FROM rust:1.95-slim-bookworm AS build
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
COPY src ./src
COPY migrations ./migrations
COPY templates ./templates
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates tzdata \
    libssl3 curl \
  && rm -rf /var/lib/apt/lists/*

RUN useradd -m -U -s /usr/sbin/nologin appuser
WORKDIR /home/appuser

COPY --from=build /app/target/release/tss /usr/local/bin/tss

ENV RUST_LOG=info
USER appuser


EXPOSE 3000
CMD ["/usr/local/bin/tss"]
