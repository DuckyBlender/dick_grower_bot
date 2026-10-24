#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")"
git pull

if [ ! -f .env ]; then
    echo "ERROR: .env does not exist in $(pwd)."
    echo "Create it with DISCORD_TOKEN and DATABASE_URL before deploying."
    exit 1
fi

if [ ! -f data/database.sqlite ]; then
    echo "ERROR: data/database.sqlite does not exist in $(pwd)."
    echo "Create it and run the migrations before deploying (see README)."
    exit 1
fi

# Build first so a failed build leaves the running bot untouched.
echo "Building new Docker image..."
docker build -t dick-bot .

echo "Stopping old container..."
docker stop dick-grower-bot || true
docker rm dick-grower-bot || true

# Back up while the bot is stopped so the copy is consistent. A clean shutdown
# folds the WAL into the database; if the bot was killed, the WAL is copied too
# so the backup keeps the latest writes. Keep the 10 newest backups.
backup_file="database.sqlite.bak.$(date +%Y%m%d%H%M%S)"
echo "Backing up database to ${backup_file}..."
cp data/database.sqlite "${backup_file}"
if [ -s data/database.sqlite-wal ]; then
    cp data/database.sqlite-wal "${backup_file}-wal"
fi
ls -1t database.sqlite.bak.* | grep -v -- '-wal$' | tail -n +11 | while read -r old; do
    rm -f -- "$old" "$old-wal"
done

echo "Starting new container..."
docker run -d \
    --name dick-grower-bot \
    --restart unless-stopped \
    --env-file "$(pwd)/.env" \
    --add-host host.docker.internal:host-gateway \
    --user "$(id -u):$(id -g)" \
    -e DATABASE_URL=sqlite:/app/data/database.sqlite \
    -v "$(pwd)/data:/app/data" \
    dick-bot

echo "Deployment complete!"
