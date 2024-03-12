#!/usr/bin/env bash
set -x
set -eo pipefail

# Start Redis container
RUNNING_CONTAINER=$(docker ps -q --filter "name=redis" --format '{{.ID}}')
if [[ -n $RUNNING_CONTAINER ]]; then
    echo >&2 "Redis container is already running, kill it with"
    echo >&2 "     docker kill ${RUNNING_CONTAINER}"
    exit 1
fi
docker run -d --name "redis_$(date '+%s')" -p 6379:6379 redis:7

# Redis is ready
echo >&2 "Redis container is up and running!"
