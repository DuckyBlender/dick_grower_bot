#!/bin/bash

# Pull latest code
git pull

# Build Docker image
echo "Building new Docker image..."
docker build -t dick-bot .

# Stop and remove any old container
echo "Stopping old container..."
docker stop dick-grower-bot || true
docker rm dick-grower-bot || true

# Ensure the database file exists before starting container
if [ ! -f "$(pwd)/database.sqlite" ]; then
    echo "ERROR: database.sqlite does not exist in $(pwd)."
    echo "Create the file and run migrations before deploying."
    exit 1
fi

# Run the new container, bind-mounting your local database file
echo "Starting new container..."
docker run -d \
    --name dick-grower-bot \
    -v "$(pwd)/database.sqlite:/app/database.sqlite" \
    dick-bot

echo "Deployment complete!"

