# syntax=docker/dockerfile:1

# ---------- frontend build ----------
FROM node:20-slim AS frontend
WORKDIR /app/web
COPY web/package.json ./
RUN npm install
COPY web/ ./
RUN npm run build

# ---------- backend build ----------
FROM rust:1-slim-bookworm AS backend
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./
COPY crates/ ./crates/
RUN cargo build --release -p rib-server

# ---------- runtime ----------
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/run-it-back-server /app/run-it-back-server
COPY --from=frontend /app/web/dist /app/static

ENV STATIC_DIR=/app/static
ENV PORT=8080
EXPOSE 8080

CMD ["/app/run-it-back-server"]
