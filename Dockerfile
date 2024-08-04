# NOTE: This Dockerfile depends on you building the github-backup binary first.
# It will then package that binary into the image, and use that as the entrypoint.
# This means that running `docker build` is not a repeatable way to build the same
# image, but the benefit is much faster cross-platform builds; a net win.
FROM debian:bookworm-slim

LABEL org.opencontainers.image.source=https://github.com/SierraSoftworks/github-backup
LABEL org.opencontainers.image.description="Backup your GitHub repositories and releases automatically"
LABEL org.opencontainers.image.licenses=MIT

RUN apt-get update && apt-get install -y \
  openssl

ADD ./github-backup /usr/local/bin/github-backup

ENTRYPOINT ["/usr/local/bin/github-backup"]
