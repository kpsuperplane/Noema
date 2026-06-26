FROM rust:1.96-bookworm AS dev

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates unzip pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://bun.sh/install | bash
ENV PATH="/root/.bun/bin:${PATH}"

RUN cargo install cargo-watch --locked

WORKDIR /workspace

CMD ["bash"]
