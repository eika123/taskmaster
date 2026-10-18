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

start_db() {
    echo "==> Starting PostgreSQL container..."
    docker compose up -d postgres
    echo "==> Waiting for PostgreSQL to become healthy..."
    until docker compose exec -T postgres pg_isready -U "${POSTGRES_USER:-todolist}" -d "${POSTGRES_DB:-todolist}" >/dev/null 2>&1; do
        sleep 1
    done
    echo "==> PostgreSQL is ready on ${POSTGRES_HOST:-127.0.0.1}:${POSTGRES_PORT:-5432}"
}

stop_containers() {
    echo "==> Spinning down containerized environment..."
    docker compose down
}

# 3. Parse options and action
ACTION=""
THREADS="${THREADS:-}"
EXTRA_ARGS=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --help|-h|help)
            echo "Usage: $0 [up|down|db|logs|--test-backend|--test-frontend|--test-all] [--threads <N>] [extra-cargo-flags...]"
            exit 0
            ;;
        --threads=*)
            THREADS="${1#*=}"
            shift
            ;;
        --threads|-t)
            if [[ $# -lt 2 ]]; then
                echo "Error: $1 requires an integer argument (number of threads)"
                exit 1
            fi
            THREADS="$2"
            shift 2
            ;;
        up|start|down|stop|db|logs|--test-backend|test-backend|--test-frontend|test-frontend|--test-all|test-all)
            if [[ -z "$ACTION" ]]; then
                ACTION="$1"
            else
                EXTRA_ARGS+=("$1")
            fi
            shift
            ;;
        *)
            EXTRA_ARGS+=("$1")
            shift
            ;;
    esac
done

ACTION="${ACTION:-up}"

# Prepare cargo test runner flags
TEST_FLAGS=()
if [[ -n "$THREADS" ]]; then
    TEST_FLAGS+=("--test-threads=$THREADS")
fi
if [[ ${#EXTRA_ARGS[@]} -gt 0 ]]; then
    TEST_FLAGS+=("${EXTRA_ARGS[@]}")
fi

case "$ACTION" in
    down|stop)
        stop_containers
        exit 0
        ;;
    db)
        start_db
        exit 0
        ;;
    logs)
        docker compose logs -f
        exit 0
        ;;
    --test-backend|test-backend)
        start_db
        trap 'stop_containers' EXIT
        if [[ -n "$THREADS" ]]; then
            echo "==> Running backend test suite with ${THREADS} thread(s)..."
        else
            echo "==> Running backend test suite (multi-threaded default)..."
        fi
        cargo test --features server -- backend types "${TEST_FLAGS[@]}"
        ;;
    --test-frontend|test-frontend)
        if [[ -n "$THREADS" ]]; then
            echo "==> Running frontend SSR test suite with ${THREADS} thread(s)..."
        else
            echo "==> Running frontend SSR test suite (multi-threaded default)..."
        fi
        cargo test --features server -- components::ssr_tests "${TEST_FLAGS[@]}"
        ;;
    --test-all|test-all)
        start_db
        trap 'stop_containers' EXIT
        if [[ -n "$THREADS" ]]; then
            echo "==> Running all tests with ${THREADS} thread(s)..."
        else
            echo "==> Running all tests (multi-threaded default)..."
        fi
        if [[ ${#TEST_FLAGS[@]} -gt 0 ]]; then
            cargo test --features server -- "${TEST_FLAGS[@]}"
        else
            cargo test --features server
        fi
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
        echo "Usage: $0 [up|down|db|logs|--test-backend|--test-frontend|--test-all] [--threads <N>] [extra-cargo-flags...]"
        exit 1
        ;;
esac
