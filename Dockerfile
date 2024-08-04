# NOTE: This Dockerfile depends on you building the github-backup binary first.
# It will then package that binary into the image, and use that as the entrypoint.
# This means that running `docker build` is not a repeatable way to build the same
# image, but the benefit is much faster cross-platform builds; a net win.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
  openssl

ARG ARTIFACT=./github-backup

ADD ${ARTIFACT} /usr/local/bin/github-backup

ENTRYPOINT ["/usr/local/bin/github-backup"]
