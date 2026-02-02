#!/bin/bash
# Olympus Stack Startup Script
set -e

echo "🏛️  Starting Olympus Stack..."

# Check if .env exists
if [ ! -f .env ]; then
    echo "📝 Creating .env from .env.example..."
    cp .env.example .env
fi

# Create storage directories
echo "📁 Creating storage directories..."
mkdir -p /mnt/data/{submissions,binaries/users,binaries/problems,testcases,temp}

# Start infrastructure first
echo "🚀 Starting infrastructure services..."
docker compose up -d postgres redis

# Wait for postgres to be ready
echo "⏳ Waiting for PostgreSQL..."
until docker compose exec -T postgres pg_isready -U olympus > /dev/null 2>&1; do
    sleep 1
done
echo "✅ PostgreSQL ready"

# Wait for redis to be ready
echo "⏳ Waiting for Redis..."
until docker compose exec -T redis redis-cli ping > /dev/null 2>&1; do
    sleep 1
done
echo "✅ Redis ready"

# Run database migrations
echo "📊 Running database migrations..."
if command -v sqlx &> /dev/null; then
    sqlx migrate run --source crates/vanguard/migrations
else
    echo "⚠️  sqlx-cli not installed. Install with: cargo install sqlx-cli"
    echo "   Then run: sqlx migrate run --source crates/vanguard/migrations"
fi

# Start application services
echo "🚀 Starting application services..."
docker compose up -d vanguard sisyphus minos horus

# Start monitoring (optional)
echo "📊 Starting monitoring services..."
docker compose up -d prometheus grafana

echo ""
echo "✅ Olympus Stack is running!"
echo ""
echo "📡 Services:"
echo "   API Gateway (Vanguard):  http://localhost:8080"
echo "   Prometheus:              http://localhost:9090"
echo "   Grafana:                 http://localhost:3001 (admin/admin)"
echo ""
echo "🔧 Useful commands:"
echo "   docker compose logs -f vanguard    # View API logs"
echo "   docker compose logs -f sisyphus    # View compiler logs"
echo "   docker compose logs -f minos       # View judge logs"
echo "   docker compose ps                  # Check service status"
echo "   docker compose down                # Stop all services"
