#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 1. Ensure .env and data/.env exist (initialize from .env_example if missing)
if [[ ! -f .env && -f .env_example ]]; then
    echo "==> Initializing .env from .env_example..."
    cp .env_example .env
fi

mkdir -p data
if [[ ! -f data/.env && -f .env ]]; then
    cp .env data/.env
fi

# 2. Load environment variables
set -a
if [[ -f data/.env ]]; then
    # shellcheck disable=SC1091
    source data/.env
fi
if [[ -f .env ]]; then
    # shellcheck disable=SC1091
    source .env
fi
set +a

ACTION="${1:-up}"

case "$ACTION" in
    down|stop)
        echo "==> Stopping containerized environment..."
        docker compose down
        exit 0
        ;;
    db)
        echo "==> Starting PostgreSQL container only..."
        docker compose up -d postgres
        echo "==> Waiting for PostgreSQL to become healthy..."
        until docker compose exec -T postgres pg_isready -U "${POSTGRES_USER:-todolist}" -d "${POSTGRES_DB:-todolist}" >/dev/null 2>&1; do
            sleep 1
        done
        echo "==> PostgreSQL is ready on ${POSTGRES_HOST:-127.0.0.1}:${POSTGRES_PORT:-5432}"
        exit 0
        ;;
    logs)
        docker compose logs -f
        exit 0
        ;;
    up|start)
        echo "==> Building Dioxus fullstack web & server bundle..."
        dx build --platform web --fullstack

        echo "==> Launching PostgreSQL & Application containers..."
        docker compose up -d --build

        echo "==> Waiting for application healthcheck on http://127.0.0.1:${PORT:-8080} ..."
        for _ in {1..30}; do
            if curl --noproxy "*" -fsS "http://127.0.0.1:${PORT:-8080}/" >/dev/null 2>&1; then
                echo "==> Application is live at http://127.0.0.1:${PORT:-8080}"
                docker compose ps
                exit 0
            fi
            sleep 1
        done

        echo "WARNING: Application did not respond within 30s. Checking container logs:"
        docker compose logs --tail=50
        exit 1
        ;;
    *)
        echo "Usage: $0 [up|down|db|logs]"
        exit 1
        ;;
esac
