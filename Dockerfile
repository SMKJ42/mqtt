FROM rust:latest AS builder

WORKDIR /usr/src

# Install required system dependencies for diesel_cli (for postgres support)
RUN apt-get update && apt-get install -y libpq-dev 
RUN cargo install diesel_cli --no-default-features --features postgres

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl3 libpq5 ca-certificates --no-install-recommends && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates


# Copy executable
COPY --from=builder /usr/src/target/release/mqtt-broker /usr/local/bin/mqtt-broker

ENTRYPOINT [ "sh", "-c", "diesel migration run && web-api" ]