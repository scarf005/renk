# syntax=docker/dockerfile:1

ARG FEDORA_MINIMAL_IMAGE=registry.fedoraproject.org/fedora-minimal@sha256:ee37c006cc45336b22d0f939286ad06f0a98f9bdfcb7650be1470f2a50b8e1ca
ARG RUSTUP_TOOLCHAIN=1.96.0
ARG RUSTUP_VERSION=1.29.3
ARG RUSTUP_INIT_SHA256=b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55
ARG DENO_VERSION=2.9.8
ARG DENO_SHA256=e890975bc2549807bbcc0b584ffafda72cc7c93349dd99c80483dd784f6c4609
FROM ${FEDORA_MINIMAL_IMAGE} AS rust-base
ARG RUSTUP_TOOLCHAIN
ARG RUSTUP_VERSION
ARG RUSTUP_INIT_SHA256
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin
RUN microdnf --setopt=install_weak_deps=0 install -y ca-certificates curl gcc glibc-devel tini unzip \
    && microdnf clean all \
    && curl -fsSL "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/x86_64-unknown-linux-gnu/rustup-init" -o /tmp/rustup-init \
    && echo "${RUSTUP_INIT_SHA256}  /tmp/rustup-init" | sha256sum -c - \
    && chmod +x /tmp/rustup-init \
    && /tmp/rustup-init -y --profile minimal --default-toolchain "${RUSTUP_TOOLCHAIN}" \
    && rm /tmp/rustup-init \
    && rustup target add wasm32-unknown-unknown \
    && chmod -R a+rx /usr/local/cargo /usr/local/rustup

FROM rust-base AS build
ARG DENO_VERSION
ARG DENO_SHA256
WORKDIR /app
RUN curl -fsSL "https://github.com/denoland/deno/releases/download/v${DENO_VERSION}/deno-x86_64-unknown-linux-gnu.zip" -o /tmp/deno.zip \
    && echo "${DENO_SHA256}  /tmp/deno.zip" | sha256sum -c - \
    && unzip /tmp/deno.zip -d /usr/local/bin \
    && rm /tmp/deno.zip
COPY . .
RUN cargo build --locked --release -p renk --bin renk --bin renk-web
RUN cargo build --locked --manifest-path runner/Cargo.toml --release
RUN cd web && VITE_RUN_API=1 deno task build

FROM rust-base AS runtime-rust
RUN rm -rf \
    /usr/local/cargo/registry \
    /usr/local/rustup/downloads \
    /usr/local/rustup/tmp \
    /usr/local/rustup/toolchains/*/lib/rustlib/wasm32-unknown-unknown \
    /usr/local/rustup/toolchains/*/share/doc \
    /usr/local/rustup/toolchains/*/share/man

FROM ${FEDORA_MINIMAL_IMAGE} AS runtime
ENV CARGO_HOME=/usr/local/cargo \
    RUSTUP_HOME=/usr/local/rustup \
    PATH=/usr/local/cargo/bin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    RENK_WEB_ADDR=0.0.0.0:8080 \
    RENK_WEB_DIST=/app/web/dist \
    RENK_RUN_DEPS=/app/runner/target/release/deps \
    RENK_RUN_TIMEOUT_MS=5000 \
    RENK_REQUEST_TIMEOUT_MS=10000 \
    RENK_MAX_REQUESTS=32 \
    RENK_MAX_CONCURRENT_RUNS=2 \
    RENK_RUN_RATE_LIMIT_PER_MINUTE=10
RUN microdnf --setopt=install_weak_deps=0 install -y ca-certificates gcc glibc-devel tini \
    && microdnf clean all
COPY --from=runtime-rust /usr/local/cargo /usr/local/cargo
COPY --from=runtime-rust /usr/local/rustup /usr/local/rustup
WORKDIR /app
COPY --from=build /app/target/release/renk /usr/local/bin/renk
COPY --from=build /app/target/release/renk-web /usr/local/bin/renk-web
COPY --from=build /app/web/dist /app/web/dist
COPY --from=build /app/runner/target/release /app/runner/target/release
USER 10001:10001
EXPOSE 8080
ENTRYPOINT ["/usr/bin/tini", "--"]
CMD ["/usr/local/bin/renk-web"]
