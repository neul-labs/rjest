#!/bin/bash

# Benchmark script comparing rjest vs Jest performance

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURE_DIR="$SCRIPT_DIR/fixtures/basic-ts"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}   rjest vs Jest Benchmark Suite     ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# Stop any running daemon
pkill -f "jestd" 2>/dev/null || true
sleep 0.5

# Ensure we're in the fixture directory
cd "$FIXTURE_DIR"

# Warm up - run once to populate caches
echo -e "${YELLOW}Warming up caches...${NC}"
npx jest --testPathPattern="sum" --ci --no-cache 2>/dev/null >/dev/null || true

# Function to run Jest and measure time
run_jest() {
    local pattern="$1"
    local output
    local exit_code

    output=$( { time npx jest --testPathPattern="$pattern" --ci --no-cache 2>&1; } 2>&1 )
    exit_code=$?

    # Extract real time from time output
    local real_time=$(echo "$output" | grep "^real" | awk '{print $2}')
    echo "$real_time $exit_code"
}

# Function to run rjest and measure time
run_rjest() {
    local pattern="$1"
    local output
    local exit_code

    output=$( { time /Users/dipankarsarkar/Code/rjest/target/release/jest --testPathPattern="$pattern" 2>&1; } 2>&1 )
    exit_code=$?

    # Extract real time from time output
    local real_time=$(echo "$output" | grep "^real" | awk '{print $2}')
    echo "$real_time $exit_code"
}

# Convert time string to seconds
time_to_seconds() {
    local time_str="$1"
    # Format: 0m0.123s or 1m23.456s
    local minutes=$(echo "$time_str" | sed 's/m.*//')
    local seconds=$(echo "$time_str" | sed 's/.*m//' | sed 's/s//')
    echo "scale=3; $minutes * 60 + $seconds" | bc
}

# Number of iterations
ITERATIONS=${1:-5}

echo -e "${BLUE}Running benchmark with $ITERATIONS iterations...${NC}"
echo ""

jest_total=0
rjest_total=0

# Benchmark: Full test suite
echo -e "${BLUE}=== Full Test Suite ===${NC}"
echo ""

for i in $(seq 1 $ITERATIONS); do
    echo -e "  ${YELLOW}Iteration $i${NC}"

    # Jest
    jest_result=$(run_jest "")
    jest_time=$(echo "$jest_result" | awk '{print $1}')
    jest_exit=$(echo "$jest_result" | awk '{print $2}')
    jest_sec=$(time_to_seconds "$jest_time")
    jest_total=$(echo "$jest_total + $jest_sec" | bc)

    # Stop daemon between runs to get cold start timing
    pkill -f "jestd" 2>/dev/null || true
    sleep 0.1

    # rjest
    rjest_result=$(run_rjest "")
    rjest_time=$(echo "$rjest_result" | awk '{print $1}')
    rjest_exit=$(echo "$rjest_result" | awk '{print $2}')
    rjest_sec=$(time_to_seconds "$rjest_time")
    rjest_total=$(echo "$rjest_total + $rjest_sec" | bc)

    echo -e "    Jest:  ${jest_time}s (${jest_sec}s)"
    echo -e "    rjest: ${rjest_time}s (${rjest_sec}s)"
    echo ""
done

# Calculate averages
jest_avg=$(echo "scale=3; $jest_total / $ITERATIONS" | bc)
rjest_avg=$(echo "scale=3; $rjest_total / $ITERATIONS" | bc)

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}           Results                   ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""
echo "Average times over $ITERATIONS iterations:"
echo ""
echo -e "  ${RED}Jest:  ${jest_avg}s${NC}"
echo -e "  ${GREEN}rjest: ${rjest_avg}s${NC}"
echo ""

if (( $(echo "$rjest_avg < $jest_avg" | bc -l) )); then
    speedup=$(echo "scale=2; $jest_avg / $rjest_avg" | bc)
    echo -e "${GREEN}✓ rjest is ${speedup}x faster than Jest!${NC}"
else
    slowdown=$(echo "scale=2; $rjest_avg / $jest_avg" | bc)
    echo -e "${YELLOW}⚠ rjest is ${slowdown}x slower than Jest${NC}"
fi

echo ""

# Run a detailed single pass for comparison
echo -e "${BLUE}=== Detailed Comparison (single run) ===${NC}"
echo ""

echo "Jest output:"
npx jest --ci 2>&1 | tail -10
echo ""

echo "rjest output:"
/Users/dipankarsarkar/Code/rjest/target/release/jest 2>&1 | tail -10

# Cleanup
echo ""
pkill -f "jestd" 2>/dev/null || true
