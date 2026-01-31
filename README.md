# AlgoJudge

A competitive programming judge system that benchmarks algorithmic solution submissions with accurate performance metrics.

## Features

- **Multi-language Support**: C, C++, Rust, Go, Zig, and Python
- **Accurate Benchmarking**: Runs solutions multiple times with statistical analysis
- **Contest System**: ICPC, Codeforces, and IOI scoring modes
- **Docker Isolation**: Each submission runs in an isolated container
- **Real-time Judging**: Redis-based queue for instant feedback

## Quick Start

### Prerequisites

- Docker and Docker Compose
- Rust 1.82+ (for development)

### Running with Docker Compose

```bash
# Clone the repository
git clone https://github.com/your-org/algojudge.git
cd algojudge

# Copy environment template
cp .env.example .env

# Build benchmark images
chmod +x docker/images/build-all.sh
./docker/images/build-all.sh

# Start all services
docker-compose up -d

# Check logs
docker-compose logs -f api
```

The API will be available at `http://localhost:8080`.

### Development Setup

```bash
# Install dependencies
cargo build

# Run database migrations
sqlx database create
sqlx migrate run

# Start Redis and PostgreSQL (or use docker-compose)
docker-compose up -d postgres redis

# Run the server
cargo run
```

## API Endpoints

### Authentication
- `POST /api/v1/auth/register` - Register new user
- `POST /api/v1/auth/login` - Login
- `POST /api/v1/auth/refresh` - Refresh access token
- `POST /api/v1/auth/logout` - Logout

### Contests
- `GET /api/v1/contests` - List contests
- `POST /api/v1/contests` - Create contest
- `GET /api/v1/contests/{id}` - Get contest details
- `PUT /api/v1/contests/{id}` - Update contest
- `DELETE /api/v1/contests/{id}` - Delete contest
- `POST /api/v1/contests/{id}/register` - Register for contest
- `GET /api/v1/contests/{id}/leaderboard` - Get leaderboard

### Problems
- `GET /api/v1/problems` - List problems
- `POST /api/v1/problems` - Create problem
- `GET /api/v1/problems/{id}` - Get problem details
- `PUT /api/v1/problems/{id}` - Update problem

### Submissions
- `POST /api/v1/submissions` - Submit solution
- `GET /api/v1/submissions/{id}` - Get submission details
- `GET /api/v1/submissions/{id}/results` - Get detailed results with benchmarks

## Benchmark Output

Each submission receives detailed performance metrics:

```json
{
  "verdict": "accepted",
  "benchmark_summary": {
    "iterations": 5,
    "time_avg_ms": 42.5,
    "time_median_ms": 41.0,
    "time_min_ms": 38.0,
    "time_max_ms": 52.0,
    "time_stddev_ms": 5.2,
    "memory_avg_kb": 2048,
    "memory_peak_kb": 2560,
    "time_outliers": [
      {"iteration": 4, "value": 52.0, "deviation": 2.1}
    ]
  }
}
```

## Project Structure

```
src/
├── main.rs                 # Application entry point
├── config.rs               # Configuration management
├── constants.rs            # Application constants
├── error.rs                # Error types
├── state.rs                # Application state
├── handlers/               # HTTP handlers
├── services/               # Business logic
├── db/                     # Database layer
├── models/                 # Domain models
├── middleware/             # HTTP middleware
├── benchmark/              # Benchmark engine
└── utils/                  # Utilities
```

## Scoring Modes

### ICPC Style
- Binary scoring (solved/not solved)
- Penalty time for wrong submissions
- Ranked by problems solved, then penalty time

### Codeforces Style
- Points decay over time
- Partial scoring for each problem
- Penalty for failed attempts

### IOI Style
- Partial scoring based on test cases
- No penalty for wrong attempts
- Best submission counts

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'feat: add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.
