# Git pull
git pull

# Stop and remove the old container if it exists
docker stop dick-grower-bot || true && docker rm dick-grower-bot || true

# Build the new Docker image
docker build -t dick-bot .

# Run the new container
docker run -d --name dick-grower-bot -v dick_data:/app/data dick-bot