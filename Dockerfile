FROM node:22-bookworm-slim AS web
WORKDIR /app
COPY package.json package-lock.json ./
COPY apps/web/package.json apps/web/package.json
RUN npm ci
COPY apps/web apps/web
RUN npm run build

FROM rust:1-bookworm AS api
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY apps/api apps/api
COPY apps/domain apps/domain
COPY clippy.toml ./
RUN cargo build --release --locked --bin appshell-api

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends libpq5 ca-certificates && rm -rf /var/lib/apt/lists/* && useradd --system --uid 10001 app
WORKDIR /app
COPY --from=api /app/target/release/appshell-api /usr/local/bin/appshell-api
COPY --from=web /app/apps/web/dist /app/public
USER app
ENV PORT=8080 STATIC_DIR=/app/public RUN_MODE=combined
EXPOSE 8080
CMD ["appshell-api"]
