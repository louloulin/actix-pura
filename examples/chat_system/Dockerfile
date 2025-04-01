FROM rust:1.75 as builder

WORKDIR /usr/src/actix-pura
COPY . .

# Build the chat system example
RUN cd examples/chat_system && cargo build --release

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/actix-pura/target/release/chat_system /usr/local/bin/chat_system

# Set up environment
ENV RUST_LOG=info,actix_cluster=debug

# Expose the default port
EXPOSE 8080

# Run the example
ENTRYPOINT ["chat_system"]
CMD ["--address", "0.0.0.0:8080"] 