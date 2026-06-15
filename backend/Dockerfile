FROM rust:1.85-slim AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p agenticos-api-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/agenticos-api-server /app/server
COPY --from=builder /app/migrations /app/migrations
EXPOSE 8000
CMD ["/app/server"]
