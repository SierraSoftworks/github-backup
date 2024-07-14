FROM clux/muslrust:stable as builder

RUN apt-get update && apt-get install -y \
    libssl-dev \
    pkg-config \
    protobuf-compiler

ADD . /volume

RUN cargo build --release

FROM alpine

COPY --from=builder /volume/target/*/release/github-backup /usr/local/bin/github-backup

CMD ["github-backup"]