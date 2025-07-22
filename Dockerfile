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
COPY --from=builder /app/bin/node/res/dev-genesis.json /root/genesis.json
RUN chmod +x /usr/local/bin/twine-node

ENTRYPOINT ["./twine-node", "node", "--dev", "--chain /root/genesis.json", "--http", "--http.api", "debug,eth,net,trace,web3,rpc,reth,ots", "--http.port=8545" ,"--http.addr=0.0.0.0", "--http.corsdomain=*", "--ws", "--ws.addr=0.0.0.0", "--ws.api","admin,debug,eth,net,trace,txpool,web3,rpc,reth,ots", "--ws.origins", "127.0.0.1", "--datadir /tmp/reth", "--rpc.eth-proof-window 1000", "--rpc.proof-permits 1000", "--dev.block-time 5sec"]

