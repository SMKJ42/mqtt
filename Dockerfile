FROM rust:latest AS builder

WORKDIR /usr/src

# Install required system dependencies for diesel_cli (for postgres support)
# RUN apt-get update && apt-get install -y libpq-dev 

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y sqlite3 --no-install-recommends && rm -rf /var/lib/apt/lists/*

# Copy executable
COPY --from=builder /usr/src/target/release/mqtt-broker /usr/local/bin/mqtt-broker

ENTRYPOINT [ "sh", "-c", "mqtt-broker" ]