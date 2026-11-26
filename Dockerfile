FROM --platform=linux/amd64 rust:1.85.0-bookworm@sha256:16a7f242108de02f10fe4a392991679bafa7694e59f5b40a54d5af1be9b40d03

ENV SOROBAN_CLI_VERSION=22.0.0

RUN rustup target add wasm32v1-none \
    && rustup component add clippy rustfmt \
    && curl --fail --location --silent --show-error \
        "https://github.com/stellar/stellar-cli/releases/download/v22.0.0/stellar-cli-22.0.0-x86_64-unknown-linux-gnu.tar.gz" \
        --output /tmp/stellar-cli.tar.gz \
    && echo "8df55a1aeff5ad2e7a036480f42956ab91d03c05f32a9025b46b86dc9b2c66bc  /tmp/stellar-cli.tar.gz" \
        | sha256sum --check --status \
    && tar -xzf /tmp/stellar-cli.tar.gz -C /usr/local/bin stellar \
    && rm -f /tmp/stellar-cli.tar.gz

WORKDIR /workspace
COPY scripts/build-wasm.sh /usr/local/bin/build-wasm
RUN chmod 0755 /usr/local/bin/build-wasm

ENTRYPOINT ["/usr/local/bin/build-wasm"]
