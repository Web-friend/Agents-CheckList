FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bin checklister-mcp

FROM debian:bookworm-slim AS runtime
ENV DATABASE_URL=sqlite:///data/checklister.db
WORKDIR /app
COPY --from=build /build/target/release/checklister-mcp /usr/local/bin/checklister-mcp
RUN useradd --uid 1000 --create-home --shell /usr/sbin/nologin checklister \
    && mkdir -p /data \
    && chown checklister:checklister /data
USER checklister
VOLUME ["/data"]
ENTRYPOINT ["checklister-mcp"]
