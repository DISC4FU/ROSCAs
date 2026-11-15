FROM rust:1.88-slim-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY .sqlx ./.sqlx
COPY src ./src
ENV SQLX_OFFLINE=true
RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --no-install-recommends -y ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN useradd --system --uid 10001 fintech
COPY --from=builder /app/target/release/fintech-core /usr/local/bin/fintech-core

USER 10001:10001
EXPOSE 8000
ENTRYPOINT ["/usr/local/bin/fintech-core"]
