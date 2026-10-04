#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"
git pull

if [ ! -f .env ]; then
    echo "ERROR: .env does not exist in $(pwd)."
    echo "Create it with DISCORD_TOKEN and DATABASE_URL before deploying."
    exit 1
fi

if [ ! -f database.sqlite ]; then
    echo "ERROR: database.sqlite does not exist in $(pwd)."
    echo "Create it and run the migrations before deploying (see README)."
    exit 1
fi

# Build first so a failed build leaves the running bot untouched.
echo "Building new Docker image..."
docker build -t dick-bot .

echo "Stopping old container..."
docker stop dick-grower-bot || true
docker rm dick-grower-bot || true

# Back up while the bot is stopped so the copy is consistent. Keep the 10 newest backups.
backup_file="database.sqlite.bak.$(date +%Y%m%d%H%M%S)"
echo "Backing up database to ${backup_file}..."
cp database.sqlite "${backup_file}"
ls -1t database.sqlite.bak.* | tail -n +11 | xargs -r rm --

echo "Starting new container..."
docker run -d \
    --name dick-grower-bot \
    --restart unless-stopped \
    --env-file "$(pwd)/.env" \
    -v "$(pwd)/database.sqlite:/app/database.sqlite" \
    dick-bot

echo "Deployment complete!"
