FROM rust:bookworm as builder

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    protobuf-compiler

ADD . /volume

WORKDIR /volume
RUN cargo build --release

FROM debian:bookworm-slim

COPY --from=builder /volume/target/release/github-backup /usr/local/bin/github-backup

CMD ["github-backup"]