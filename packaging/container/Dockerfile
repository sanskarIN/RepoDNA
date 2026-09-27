# syntax=docker/dockerfile:1
#
# The repodna command line with Git, for CI jobs and machines without an installation.
# The release workflow builds it for linux/amd64 and linux/arm64 from the static Linux
# binaries it publishes, and pushes it to ghcr.io/sanskarin/repodna. The build context holds:
#
#   amd64/repodna, arm64/repodna      the binaries
#   LICENSE, NOTICE, THIRD-PARTY-NOTICES.txt
#
# Usage: docker run --rm -v "$PWD:/work" ghcr.io/sanskarin/repodna analyze .

FROM alpine:3.24

ARG TARGETARCH
ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="RepoDNA" \
      org.opencontainers.image.description="Local-first repository intelligence and code archaeology: the repodna command line with Git." \
      org.opencontainers.image.source="https://github.com/sanskarIN/RepoDNA" \
      org.opencontainers.image.url="https://github.com/sanskarIN/RepoDNA" \
      org.opencontainers.image.documentation="https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#container-image" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.vendor="Sanskar" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}"

# Git for history analysis. Mounted repositories usually belong to another user, which Git
# refuses unless it is told to trust them.
RUN apk add --no-cache git ca-certificates \
    && git config --system --add safe.directory '*'

COPY --chmod=0755 ${TARGETARCH}/repodna /usr/local/bin/repodna
COPY LICENSE NOTICE THIRD-PARTY-NOTICES.txt /usr/share/doc/repodna/

# Storage in a directory any user can write, so the image also runs with --user.
ENV REPODNA_HOME=/tmp/repodna
WORKDIR /work
ENTRYPOINT ["repodna"]
CMD ["--help"]
