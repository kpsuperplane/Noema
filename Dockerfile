FROM rust:1.96-bookworm AS dev

ARG CODEX_CLI_VERSION=0.142.0
ARG NODE_MAJOR=22

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates unzip pkg-config libssl-dev \
    && curl -fsSL "https://deb.nodesource.com/setup_${NODE_MAJOR}.x" | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/*

RUN npm install -g @openai/codex@${CODEX_CLI_VERSION} \
    && codex --version

RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

RUN cargo install cargo-watch --locked

WORKDIR /workspace

CMD ["bash"]
