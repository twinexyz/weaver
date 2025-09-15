FROM rust:1.86 AS builder

#ARG GITHUB_TOKEN
#ARG GITHUB_USERNAME
RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    apt-get update && \
    apt-get install -y \
    build-essential \
    clang \
    libssl-dev \
    cmake \
    gcc \
    pkg-config && \
    git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

WORKDIR /app

COPY . .

RUN cargo build --release --bin twine-node
RUN cargo build --release --bin twine-proof-scheduler-bin --features l2-proof-scheduler
RUN mv target/release/twine-proof-scheduler-bin target/release/twine-l2-proof-scheduler-bin
RUN cargo build --release --bin twine-proof-scheduler-bin --features solana-proof-scheduler
RUN mv target/release/twine-proof-scheduler-bin target/release/twine-solana-proof-scheduler-bin
RUN cargo build --release --bin twine-l2-execution-prover-worker
RUN cargo build --release --bin twine-aggregator

FROM ubuntu:24.04 AS runtime

RUN apt-get update && \
    apt-get install -y \
    build-essential \
    clang \
    libssl-dev \
    pkg-config \
    ca-certificates \
    curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/twine-node /usr/local/bin/node
COPY --from=builder /app/target/release/twine-l2-proof-scheduler-bin /usr/local/bin/scheduler
COPY --from=builder /app/target/release/twine-solana-proof-scheduler-bin /usr/local/bin/solana-scheduler
COPY --from=builder /app/target/release/twine-l2-execution-prover-worker /usr/local/bin/prover
COPY --from=builder /app/target/release/twine-aggregator /usr/local/bin/aggregator
COPY ./entrypoint.sh /entrypoint.sh
