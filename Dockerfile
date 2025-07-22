FROM rust:1.85 AS builder

ARG GITHUB_TOKEN
ENV GITHUB_TOKEN=${GITHUB_TOKEN}

RUN apt-get update && \
    apt-get install -y build-essential clang libssl-dev pkg-config && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY . .

RUN cargo build --release

FROM ubuntu:24.04 AS runtime

RUN apt-get update && apt-get install -y build-essential clang libssl-dev pkg-config \
    ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/twine-node /usr/local/bin/twine-node
RUN chmod +x /usr/local/bin/twine-node

ENTRYPOINT ["./twine-node", "node", "--dev", "--chain bin/node/res/dev-genesis.json", "--http", "--http.port 8570", "--ws", "--ws.port 8571", "--port 8572", "--authrpc.port 8573", "--datadir /tmp/reth", "--rpc.eth-proof-window 1000", "--rpc.proof-permits 1000", "--dev.block-time 5sec"]

