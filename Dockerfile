FROM rust:1-trixie AS chef
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release -p deluge-maintain

FROM debian:trixie-slim
COPY --from=builder /app/target/release/deluge-maintain /usr/local/bin/deluge-maintain

VOLUME ["/config"]
ENV DELUGE_MAINTAIN_CONFIG=/config/deluge-maintain.toml \
    RUST_LOG=info

ENTRYPOINT ["deluge-maintain"]