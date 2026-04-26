#!/bin/bash

# Pull latest code
git pull

# Ensure the environment file exists before building/running container
if [ ! -f "$(pwd)/.env" ]; then
    echo "ERROR: .env does not exist in $(pwd)."
    echo "Create it with DISCORD_TOKEN and DATABASE_URL before deploying."
    exit 1
fi

# Ensure the database file exists before starting container
if [ ! -f "$(pwd)/database.sqlite" ]; then
    echo "ERROR: database.sqlite does not exist in $(pwd)."
    echo "Create the file and run migrations before deploying."
    exit 1
fi

# Back up the database before replacing the running container
backup_file="$(pwd)/database.sqlite.bak.$(date +%Y%m%d%H%M%S)"
echo "Backing up database to ${backup_file}..."
cp "$(pwd)/database.sqlite" "${backup_file}"

# Build Docker image
echo "Building new Docker image..."
docker build -t dick-bot .

# Stop and remove any old container
echo "Stopping old container..."
docker stop dick-grower-bot || true
docker rm dick-grower-bot || true

# Run the new container, bind-mounting your local database file
echo "Starting new container..."
docker run -d \
    --name dick-grower-bot \
    --restart unless-stopped \
    --env-file "$(pwd)/.env" \
    -v "$(pwd)/database.sqlite:/app/database.sqlite" \
    dick-bot

echo "Deployment complete!"
