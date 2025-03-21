# Git pull to get the latest code
git pull

# Build the new Docker image first
echo "Building new Docker image..."
docker build -t dick-bot .

# Only stop and remove the old container after the new image is ready
echo "Stopping old container..."
docker stop dick-grower-bot || true 
docker rm dick-grower-bot || true

# Run the new container with the same volume mount to preserve the database
echo "Starting new container..."
docker run -d --name dick-grower-bot -v dick_data:/app/data dick-bot

echo "Deployment complete!"