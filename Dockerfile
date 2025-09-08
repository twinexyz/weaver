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

RUN cargo build --release

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

COPY --from=builder /app/target/release/twine-node /usr/local/bin/twine-node
COPY --from=builder /app/target/release/twine-l2-proof-scheduler-bin /usr/local/bin/twine-l2-proof-scheduler-bin
COPY --from=builder /app/target/release/twine-l2-execution-prover-worker /usr/local/bin/twine-l2-execution-prover-worker

COPY --from=builder /app/bin/node/res/dev-genesis.json /root/genesis.json
