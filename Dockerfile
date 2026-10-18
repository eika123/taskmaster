FROM ubuntu:24.04

ARG DX_PROFILE=debug

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY target/dx/todolist/${DX_PROFILE}/web/server /app/server
COPY target/dx/todolist/${DX_PROFILE}/web/public /app/public

ENV IP=0.0.0.0
ENV PORT=8080

EXPOSE 8080

HEALTHCHECK --interval=5s --timeout=3s --start-period=5s --retries=5 \
    CMD curl -fsS http://127.0.0.1:${PORT}/ || exit 1

CMD ["/app/server"]

