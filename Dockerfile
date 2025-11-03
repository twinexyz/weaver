FROM ubuntu:24.04 AS builder

ARG FEATURES
ARG GITHUB_ORGANIZATION
ARG ARCH

ARG RSP_BRANCH
ARG SOLANA_STUB_PROVER_BRANCH

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    apt update && \
    apt install -y \
    build-essential \
    clang \
    libssl-dev \
    pkg-config \
    cmake \
    gcc \
    wget \
    bash \
    curl \
    git \
    jq \
    m4 && \
    git config --global credential.helper store && \
    echo "https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com" > ~/.git-credentials && \
    chmod 600 ~/.git-credentials

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y
ENV PATH="/root/.cargo/bin:${PATH}"

RUN rustup toolchain install nightly --allow-downgrade --profile minimal --component clippy 
RUN wget -c https://github.com/mikefarah/yq/releases/download/v4.45.1/yq_linux_${ARCH} -O /usr/bin/yq && \
    chmod +x /usr/bin/yq && \
    curl -OL https://go.dev/dl/go1.24.0.linux-${ARCH}.tar.gz && \
    tar -C /usr/local -xzf go1.24.0.linux-${ARCH}.tar.gz && \
    rm go1.24.0.linux-${ARCH}.tar.gz && \
    curl -L https://sp1.succinct.xyz | bash && ~/.sp1/bin/sp1up

WORKDIR /app

COPY . .

RUN cargo build --release --bin twine-node --features $FEATURES && \
    cargo build --release --bin twine-proof-scheduler-bin --features l2-proof-scheduler && \
    mv target/release/twine-proof-scheduler-bin target/release/twine-l2-proof-scheduler-bin && \
    cargo build --release --bin twine-proof-scheduler-bin --features solana-proof-scheduler && \
    mv target/release/twine-proof-scheduler-bin target/release/twine-solana-proof-scheduler-bin && \
    cargo build --release --bin twine-l2-execution-prover-worker && \
    cargo build --release --bin twine-aggregator && \
    cargo build --release --bin twine-egressa-bin && \
    cargo install tomq sqlx-cli

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    git clone --branch staging https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com/${GITHUB_ORGANIZATION}/twine-rsp.git && \
    cd twine-rsp && \
    cargo update && \
    cd bin/client && \
    cargo update && \
    cd ../.. && \
    cargo build --release --bin rsp --features $FEATURES 

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    git clone --branch v0.1.0-devnet https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com/${GITHUB_ORGANIZATION}/solana-stub-prover.git && \
    cd solana-stub-prover && \
    cargo build --release

RUN --mount=type=secret,id=github_token,env=GITHUB_TOKEN \
    --mount=type=secret,id=github_username,env=GITHUB_USERNAME \
    git clone --branch v0.1.0-testnet https://${GITHUB_USERNAME}:${GITHUB_TOKEN}@github.com/${GITHUB_ORGANIZATION}/merlin.git && \
    cd merlin && \
    cargo build --release

##############################
# final with all
##############################
FROM nvidia/cuda:12.9.1-cudnn-runtime-ubuntu24.04 AS final

ARG RSP_FILENAME
ARG SOLANA_STUB_PROVER_FILENAME

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt install ca-certificates curl -y && \
     install -m 0755 -d /etc/apt/keyrings && \
     curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc && \
     chmod a+r /etc/apt/keyrings/docker.asc && \
     echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
      $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}") stable" | \
       tee /etc/apt/sources.list.d/docker.list > /dev/null && \
     apt update && \
     apt install -y \
     docker-ce \
     docker-ce-cli \
     containerd.io \
     docker-buildx-plugin \
     cmake \
     nginx \
     wget \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /root/.sp1/bin/sp1up /usr/local/bin/sp1up
COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/twine-rsp/$RSP_FILENAME /usr/local/bin/rsp
COPY --from=builder /app/merlin/target/release/withdraw-prover /usr/local/bin/withdraw-prover
COPY --from=builder /app/merlin/target/release/l1-txns-prover /usr/local/bin/l1-txns-prover
COPY --from=builder /app/merlin/target/release/refund-prover /usr/local/bin/refund-prover
COPY --from=builder /app/solana-stub-prover/$SOLANA_STUB_PROVER_FILENAME /usr/local/bin/solana-stub-prover

COPY --from=builder /app/target/release/twine-node /usr/local/bin/twine-node
COPY --from=builder /app/target/release/twine-aggregator /usr/local/bin/aggregator
COPY --from=builder /app/target/release/twine-l2-proof-scheduler-bin /usr/local/bin/scheduler
COPY --from=builder /app/target/release/twine-solana-proof-scheduler-bin /usr/local/bin/solana-scheduler
COPY --from=builder /app/target/release/twine-l2-execution-prover-worker /usr/local/bin/prover
COPY --from=builder /app/target/release/twine-egressa-bin /usr/local/bin/egressa
COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./entrypoint.sh /entrypoint.sh
COPY ./crates/egressa/src/database/migrations /migrations

##############################
# prover
##############################
FROM nvidia/cuda:12.9.1-cudnn-runtime-ubuntu24.04 AS prover 

ARG RSP_FILENAME
ARG SOLANA_STUB_PROVER_FILENAME

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt install ca-certificates curl -y && \
     install -m 0755 -d /etc/apt/keyrings && \
     curl -fsSL https://download.docker.com/linux/ubuntu/gpg -o /etc/apt/keyrings/docker.asc && \
     chmod a+r /etc/apt/keyrings/docker.asc && \
     echo \
      "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.asc] https://download.docker.com/linux/ubuntu \
      $(. /etc/os-release && echo "${UBUNTU_CODENAME:-$VERSION_CODENAME}") stable" | \
       tee /etc/apt/sources.list.d/docker.list > /dev/null && \
     apt update && \
     apt install -y \
     docker-ce \
     docker-ce-cli \
     containerd.io \
     docker-buildx-plugin \
     cmake \
     nginx \
     wget \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /root/.sp1/bin/sp1up /usr/local/bin/sp1up
COPY --from=builder /app/twine-rsp/$RSP_FILENAME /usr/local/bin/rsp
COPY --from=builder /app/solana-stub-prover/$SOLANA_STUB_PROVER_FILENAME /usr/local/bin/solana-stub-prover

COPY --from=builder /app/target/release/twine-l2-execution-prover-worker /usr/local/bin/prover

##############################
# egressa
##############################
FROM ubuntu:24.04 AS egressa

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt update && \
     apt install -y \
     cmake \
     nginx \
     wget \
     curl \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /root/.sp1/bin/sp1up /usr/local/bin/sp1up
COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/merlin/target/release/withdraw-prover /usr/local/bin/withdraw-prover
COPY --from=builder /app/merlin/target/release/l1-txns-prover /usr/local/bin/l1-txns-prover
COPY --from=builder /app/merlin/target/release/refund-prover /usr/local/bin/refund-prover

COPY --from=builder /app/target/release/twine-egressa-bin /usr/local/bin/egressa
COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./entrypoint.sh /entrypoint.sh
COPY ./crates/egressa/src/database/migrations /migrations

##############################
# scheduler
##############################
FROM ubuntu:24.04 AS scheduler

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt update && \
     apt install -y \
     cmake \
     nginx \
     wget \
     curl \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/target/release/twine-l2-proof-scheduler-bin /usr/local/bin/scheduler
COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./entrypoint.sh /entrypoint.sh

##############################
# aggregator
##############################
FROM ubuntu:24.04 AS aggregator

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt update && \
     apt install -y \
     cmake \
     nginx \
     wget \
     curl \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/target/release/twine-aggregator /usr/local/bin/aggregator
COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./entrypoint.sh /entrypoint.sh

##############################
# solana-scheduler
##############################
FROM ubuntu:24.04 AS solana-scheduler

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt update && \
     apt install -y \
     cmake \
     nginx \
     wget \
     curl \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/target/release/twine-solana-proof-scheduler-bin /usr/local/bin/solana-scheduler
COPY ./nginx.conf /etc/nginx/nginx.conf
COPY ./entrypoint.sh /entrypoint.sh

##############################
# twine-node
##############################
FROM ubuntu:24.04 AS twine-node

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:${PATH}"

RUN  apt update && \
     apt update && \
     apt install -y \
     cmake \
     nginx \
     wget \
     curl \
     ca-certificates && \
     rm -rf /var/lib/apt/lists/*

# Install Rust (stable)
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y

COPY --from=builder /usr/bin/yq /usr/local/bin/yq
COPY --from=builder /root/.cargo/bin/tomq /usr/local/bin/tomq
COPY --from=builder /root/.cargo/bin/sqlx /usr/local/bin/sqlx

COPY --from=builder /app/target/release/twine-node /usr/local/bin/twine-node
