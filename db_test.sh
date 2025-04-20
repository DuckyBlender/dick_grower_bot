# 1. Stop the container (if it's running)
docker stop sqlx-postgres-dev

# 2. Remove the container
#    (Use -f to force removal if it wasn't stopped properly)
docker rm sqlx-postgres-dev

# 3. Remove the named volume (THIS DELETES ALL DATA)
#    Be absolutely sure you want to do this!
docker volume rm postgres_data

# 4. Re-run your original command to start fresh
docker run -d \
  --name sqlx-postgres-dev \
  -p 5432:5432 \
  -e POSTGRES_DB=dick_data \
  -e POSTGRES_USER=dick_user \
  -e POSTGRES_PASSWORD=mysecretpassword \
  -v postgres_data:/var/lib/postgresql/data \
  --restart unless-stopped \
  postgres:alpine # Or your chosen version