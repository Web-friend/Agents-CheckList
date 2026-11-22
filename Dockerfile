FROM rust:1-bookworm AS rust-build
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --release --bin checklister

FROM node:22-bookworm-slim AS node-deps
WORKDIR /app/mcp
COPY mcp/package.json mcp/package-lock.json ./
RUN npm ci --omit=dev

FROM node:22-bookworm-slim AS runtime
ENV NODE_ENV=production \
    CHECKLISTER_BIN=/usr/local/bin/checklister \
    DATABASE_URL=sqlite:///data/checklister.db
WORKDIR /app/mcp
COPY --from=rust-build /build/target/release/checklister /usr/local/bin/checklister
COPY --from=node-deps /app/mcp/node_modules ./node_modules
COPY mcp/package.json mcp/index.js mcp/server.js mcp/cli.js ./
RUN mkdir -p /data && chown node:node /data
USER node
VOLUME ["/data"]
ENTRYPOINT ["node", "/app/mcp/index.js"]
