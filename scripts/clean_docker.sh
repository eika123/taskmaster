#!/usr/bin/env bash
set -euo pipefail

# clean_docker.sh
# Safely reclaims disk space from Docker containers, PostgreSQL volumes, and build cache.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
cd "$PROJECT_ROOT"

PRINT_STATS=true
CLEAN_VOLUMES=true
CLEAN_BUILD_CACHE=false
CLEAN_DANGLING_IMAGES=false
DEEP_CLEAN=false

print_usage() {
    cat <<EOF
Usage: $0 [OPTIONS]

Options:
  --volumes, -v       Remove project containers and PostgreSQL volumes (docker compose down -v) [Default]
  --cache, -c         Also prune Docker build cache (reclaims large build caches)
  --images, -i        Also prune dangling images
  --all, -a           Full cleanup: volumes, orphan containers, dangling volumes, build cache, and dangling images
  --help, -h          Show this help message

Examples:
  $0                  # Reset project containers and PostgreSQL volumes
  $0 --all            # Complete cleanup of volumes, build cache, and dangling artifacts
EOF
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --volumes|-v)
            CLEAN_VOLUMES=true
            shift
            ;;
        --cache|-c)
            CLEAN_BUILD_CACHE=true
            shift
            ;;
        --images|-i)
            CLEAN_DANGLING_IMAGES=true
            shift
            ;;
        --all|-a|all)
            CLEAN_VOLUMES=true
            CLEAN_BUILD_CACHE=true
            CLEAN_DANGLING_IMAGES=true
            DEEP_CLEAN=true
            shift
            ;;
        --help|-h|help)
            print_usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            print_usage
            exit 1
            ;;
    esac
done

echo "=================================================="
echo "          Docker Disk Space Cleanup               "
echo "=================================================="

echo "==> Current Docker resource usage:"
docker system df || true
echo ""

echo "==> Stopping project containers and removing PostgreSQL volumes..."
docker compose down -v --remove-orphans || true

if [[ "$CLEAN_VOLUMES" == true ]]; then
    echo "==> Pruning unused dangling volumes..."
    docker volume prune -f || true
fi

if [[ "$CLEAN_BUILD_CACHE" == true || "$DEEP_CLEAN" == true ]]; then
    echo "==> Pruning Docker build cache..."
    docker builder prune -f || true
fi

if [[ "$CLEAN_DANGLING_IMAGES" == true || "$DEEP_CLEAN" == true ]]; then
    echo "==> Pruning dangling Docker images..."
    docker image prune -f || true
fi

echo ""
echo "=================================================="
echo "          Cleanup Completed Successfully          "
echo "=================================================="
echo "==> Updated Docker resource usage:"
docker system df || true
echo ""
echo "==> Current filesystem disk space:"
df -h . | awk 'NR==1 || NR==2'

