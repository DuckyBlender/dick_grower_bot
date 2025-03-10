docker build -t dick-bot .
docker run -d --restart unless-stopped --name dick-grower-bot -v dick_data:/app/data dick-bot