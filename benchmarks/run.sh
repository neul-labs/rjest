#!/bin/bash
# rjest Benchmark Harness
# Measures performance of rjest vs upstream Jest

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RJEST_BIN="$PROJECT_ROOT/target/release/jest"
RJEST_DAEMON="$PROJECT_ROOT/target/release/jestd"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/basic-ts"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
WARMUP_RUNS=2
BENCHMARK_RUNS=5

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}       rjest Benchmark Suite          ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# Check if release build exists
if [ ! -f "$RJEST_BIN" ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release
fi

# Stop any running daemon
"$RJEST_BIN" --daemon-stop 2>/dev/null || true
sleep 1

cd "$FIXTURE_DIR"

# Helper function to measure time in milliseconds
measure_time() {
    local start=$(date +%s%N)
    "$@" > /dev/null 2>&1
    local end=$(date +%s%N)
    echo $(( (end - start) / 1000000 ))
}

# Array to store results
declare -a cold_times
declare -a warm_times
declare -a jest_times

echo -e "${GREEN}Benchmark 1: Cold Start (daemon startup + test run)${NC}"
echo "------------------------------------------------"
for i in $(seq 1 $BENCHMARK_RUNS); do
    # Stop daemon
    "$RJEST_BIN" --daemon-stop 2>/dev/null || true
    sleep 0.5

    # Measure cold start
    time_ms=$(measure_time "$RJEST_BIN")
    cold_times+=($time_ms)
    echo "  Run $i: ${time_ms}ms"
done

# Calculate average
sum=0
for t in "${cold_times[@]}"; do
    sum=$((sum + t))
done
avg_cold=$((sum / BENCHMARK_RUNS))
echo -e "  ${YELLOW}Average: ${avg_cold}ms${NC}"
echo ""

echo -e "${GREEN}Benchmark 2: Warm Run (daemon already running)${NC}"
echo "------------------------------------------------"

# Warmup runs
for i in $(seq 1 $WARMUP_RUNS); do
    "$RJEST_BIN" > /dev/null 2>&1
done

# Benchmark runs
for i in $(seq 1 $BENCHMARK_RUNS); do
    time_ms=$(measure_time "$RJEST_BIN")
    warm_times+=($time_ms)
    echo "  Run $i: ${time_ms}ms"
done

sum=0
for t in "${warm_times[@]}"; do
    sum=$((sum + t))
done
avg_warm=$((sum / BENCHMARK_RUNS))
echo -e "  ${YELLOW}Average: ${avg_warm}ms${NC}"
echo ""

# Check if upstream Jest is available
if command -v npx &> /dev/null && [ -f "package.json" ]; then
    echo -e "${GREEN}Benchmark 3: Upstream Jest Comparison${NC}"
    echo "------------------------------------------------"

    for i in $(seq 1 $BENCHMARK_RUNS); do
        time_ms=$(measure_time npx jest)
        jest_times+=($time_ms)
        echo "  Run $i: ${time_ms}ms"
    done

    sum=0
    for t in "${jest_times[@]}"; do
        sum=$((sum + t))
    done
    avg_jest=$((sum / BENCHMARK_RUNS))
    echo -e "  ${YELLOW}Average: ${avg_jest}ms${NC}"
    echo ""

    # Calculate speedup
    speedup=$(echo "scale=2; $avg_jest / $avg_warm" | bc)
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}             Summary                  ${NC}"
    echo -e "${BLUE}======================================${NC}"
    echo ""
    echo "Cold start (rjest):  ${avg_cold}ms"
    echo "Warm run (rjest):    ${avg_warm}ms"
    echo "Upstream Jest:       ${avg_jest}ms"
    echo ""
    echo -e "${GREEN}Speedup (warm): ${speedup}x faster than Jest${NC}"
else
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}             Summary                  ${NC}"
    echo -e "${BLUE}======================================${NC}"
    echo ""
    echo "Cold start (rjest):  ${avg_cold}ms"
    echo "Warm run (rjest):    ${avg_warm}ms"
    echo ""
    echo "(Upstream Jest not available for comparison)"
fi

# Stop daemon
"$RJEST_BIN" --daemon-stop 2>/dev/null || true
