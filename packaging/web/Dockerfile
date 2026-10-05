# syntax=docker/dockerfile:1
#
# The RepoDNA web interface on nginx, for teams that run the web version on their own
# network or without an internet connection. It is the interface published at
# https://sanskarin.github.io/RepoDNA/: analyses are opened from files in the browser and
# never uploaded. The release workflow builds it for linux/amd64 and linux/arm64 and pushes
# it to ghcr.io/sanskarin/repodna-web. The build context holds:
#
#   dist/        the built interface (apps/web/dist)
#   nginx.conf   the server configuration from this directory
#   LICENSE, NOTICE, THIRD-PARTY-NOTICES.txt
#
# Usage: docker run --rm -p 8080:8080 ghcr.io/sanskarin/repodna-web, then open
# http://localhost:8080.

FROM nginxinc/nginx-unprivileged:stable-alpine-slim

ARG VERSION=dev
ARG REVISION=unknown

LABEL org.opencontainers.image.title="RepoDNA web interface" \
      org.opencontainers.image.description="The RepoDNA web interface on nginx: open analyses and the demo in your browser, on your own network." \
      org.opencontainers.image.source="https://github.com/sanskarIN/RepoDNA" \
      org.opencontainers.image.url="https://github.com/sanskarIN/RepoDNA" \
      org.opencontainers.image.documentation="https://github.com/sanskarIN/RepoDNA/blob/main/docs/installation.md#web-interface-image" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.vendor="Sanskar" \
      org.opencontainers.image.version="${VERSION}" \
      org.opencontainers.image.revision="${REVISION}"

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY dist/ /usr/share/nginx/html/
COPY LICENSE NOTICE THIRD-PARTY-NOTICES.txt /usr/share/doc/repodna/

# The base image runs nginx as an unprivileged user, so it listens on 8080.
EXPOSE 8080
