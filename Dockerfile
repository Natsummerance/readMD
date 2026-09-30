# ReadMD headless server image: the Rust kernel without the desktop window
# (`--no-default-features`), serving the web UI on port 8080.
#
#   docker build -t readmd .
#   docker run --rm -p 127.0.0.1:8080:8080 -v "$PWD/docs:/data/workspace" readmd
#   open http://localhost:8080/
#
# The kernel only answers API calls whose Host is localhost / 127.0.0.1 on the
# port it is bound to, so publish the same port number and browse via
# localhost.  There is no login: keep the port bound to 127.0.0.1 (as above)
# unless the network in front of it is trusted.

# ---- build ------------------------------------------------------------------
# Pin by digest in CI (`docker buildx imagetools inspect rust:1.85-slim-bookworm`).
FROM rust:1.85-slim-bookworm AS build
WORKDIR /src
COPY rust/Cargo.toml rust/Cargo.lock rust/
COPY rust/readmd-kernel rust/readmd-kernel
COPY rust/xtask rust/xtask
# Release profile already strips symbols (rust/Cargo.toml).
RUN cargo build --release --locked --manifest-path rust/Cargo.toml -p readmd-kernel --no-default-features

# ---- runtime ----------------------------------------------------------------
FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl fonts-noto-cjk \
 && rm -rf /var/lib/apt/lists/* \
 && useradd --system --create-home --home-dir /data readmd
COPY --from=build /src/rust/target/release/readmd /usr/local/bin/readmd
COPY assets /usr/share/readmd/assets
COPY VERSION /usr/share/readmd/VERSION

ENV READMD_ASSETS_DIR=/usr/share/readmd/assets \
    READMD_DATA_DIR=/data/readmd \
    READMD_WORKSPACE=/data/workspace
USER readmd
WORKDIR /data
RUN mkdir -p /data/readmd /data/workspace
VOLUME ["/data"]
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS -H "Host: 127.0.0.1:8080" http://127.0.0.1:8080/ >/dev/null || exit 1

ENTRYPOINT ["readmd", "--no-window", "--host", "0.0.0.0", "--port", "8080"]
