#!/bin/sh
# Exit immediately if a command exits with a non-zero status.
set -e

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
  echo "Error: DATABASE_URL environment variable is not set."
  exit 1
fi

# Wait for the database to be ready (optional, adjust timeout and command as needed)
# Example for PostgreSQL:
# echo "Waiting for database..."
# while ! pg_isready -q -h $DB_HOST -p $DB_PORT -U $DB_USER; do
#   sleep 1
# done

# Attempt to create the database.
# The || true prevents the script from exiting if the database already exists.
echo "Attempting to create database..."
sqlx database create || echo "Database already exists or could not be created (continuing)."

# Run migrations
echo "Running database migrations..."
sqlx migrate run --database-url="$DATABASE_URL"

# Execute the main application (passed as CMD in Dockerfile or arguments to `docker run`)
echo "Starting application..."
exec "$@"