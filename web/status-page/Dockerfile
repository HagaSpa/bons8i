# syntax=docker/dockerfile:1

FROM node:24-alpine AS frontend
WORKDIR /app/frontend
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend/ ./
RUN npm run build

# rust:alpine の既定ターゲットは musl = 静的リンク。ring のビルドに musl-dev が要る
FROM rust:1.95-alpine AS server
RUN apk add --no-cache musl-dev
WORKDIR /app/server
# 依存だけ先にビルドしてレイヤーキャッシュに乗せる
COPY server/Cargo.toml server/Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs \
    && cargo build --release \
    && rm -rf src
COPY server/src ./src
# rust-embed が ../frontend/dist/ をコンパイル時に読む
COPY --from=frontend /app/frontend/dist /app/frontend/dist
RUN touch src/main.rs && cargo build --release

# CA 証明書は rustls-tls-webpki-roots でバイナリ同梱済みなので scratch でよい
FROM scratch
COPY --from=server /app/server/target/release/status-page /status-page
EXPOSE 8080
ENTRYPOINT ["/status-page"]
