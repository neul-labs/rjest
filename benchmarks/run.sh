#!/bin/bash
# rjest Benchmark Harness
# Measures performance and memory of rjest vs upstream Jest

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
RED='\033[0;31m'
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

# Helper function to get daemon memory usage (RSS in KB)
get_daemon_memory() {
    local pid=$(pgrep -f "jestd" 2>/dev/null | head -1)
    if [ -n "$pid" ]; then
        # Get RSS in KB from /proc
        local rss=$(cat /proc/$pid/status 2>/dev/null | grep VmRSS | awk '{print $2}')
        if [ -n "$rss" ]; then
            echo "$rss"
        else
            echo "0"
        fi
    else
        echo "0"
    fi
}

# Helper function to get total memory of daemon + workers
get_total_memory() {
    local total=0
    # Daemon memory
    local daemon_pid=$(pgrep -f "jestd" 2>/dev/null | head -1)
    if [ -n "$daemon_pid" ]; then
        local daemon_rss=$(cat /proc/$daemon_pid/status 2>/dev/null | grep VmRSS | awk '{print $2}')
        if [ -n "$daemon_rss" ]; then
            total=$((total + daemon_rss))
        fi
    fi
    # Worker memory (node processes spawned by jestd)
    for pid in $(pgrep -P "$daemon_pid" 2>/dev/null); do
        local worker_rss=$(cat /proc/$pid/status 2>/dev/null | grep VmRSS | awk '{print $2}')
        if [ -n "$worker_rss" ]; then
            total=$((total + worker_rss))
        fi
    done
    echo "$total"
}

# Helper to format memory
format_memory() {
    local kb=$1
    if [ "$kb" -gt 1024 ]; then
        echo "$(echo "scale=1; $kb / 1024" | bc)MB"
    else
        echo "${kb}KB"
    fi
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

# Memory measurement for rjest
echo -e "${GREEN}Benchmark 3: Memory Usage (rjest daemon + workers)${NC}"
echo "------------------------------------------------"
daemon_mem=$(get_daemon_memory)
total_mem=$(get_total_memory)
echo "  Daemon RSS:     $(format_memory $daemon_mem)"
echo "  Total (+ workers): $(format_memory $total_mem)"
echo ""

# Check if upstream Jest is available
if command -v npx &> /dev/null && [ -f "package.json" ]; then
    echo -e "${GREEN}Benchmark 4: Upstream Jest Comparison${NC}"
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

    # Measure Jest peak memory using /usr/bin/time
    echo -e "${GREEN}Benchmark 5: Jest Memory Usage${NC}"
    echo "------------------------------------------------"
    jest_mem=$( { /usr/bin/time -v npx jest 2>&1 | grep "Maximum resident set size" | awk '{print $6}'; } 2>/dev/null || echo "0" )
    if [ -n "$jest_mem" ] && [ "$jest_mem" != "0" ]; then
        echo "  Jest Peak RSS:  $(format_memory $jest_mem)"
    else
        # Fallback: try to measure with ps during run
        npx jest > /dev/null 2>&1 &
        jest_pid=$!
        sleep 0.5
        jest_mem=$(ps -o rss= -p $jest_pid 2>/dev/null || echo "0")
        wait $jest_pid 2>/dev/null || true
        if [ -n "$jest_mem" ] && [ "$jest_mem" != "0" ]; then
            echo "  Jest RSS (sampled): $(format_memory $jest_mem)"
        else
            echo "  Jest memory: (measurement unavailable)"
        fi
    fi
    echo ""

    # Calculate speedup
    speedup=$(echo "scale=2; $avg_jest / $avg_warm" | bc)

    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}             Summary                  ${NC}"
    echo -e "${BLUE}======================================${NC}"
    echo ""
    echo -e "${YELLOW}Timing:${NC}"
    echo "  Cold start (rjest):  ${avg_cold}ms"
    echo "  Warm run (rjest):    ${avg_warm}ms"
    echo "  Upstream Jest:       ${avg_jest}ms"
    echo ""
    echo -e "${YELLOW}Memory:${NC}"
    echo "  rjest daemon:        $(format_memory $daemon_mem)"
    echo "  rjest total:         $(format_memory $total_mem)"
    if [ -n "$jest_mem" ] && [ "$jest_mem" != "0" ]; then
        echo "  Jest peak:           $(format_memory $jest_mem)"
    fi
    echo ""
    echo -e "${GREEN}Speedup (warm): ${speedup}x faster than Jest${NC}"
else
    echo -e "${BLUE}======================================${NC}"
    echo -e "${BLUE}             Summary                  ${NC}"
    echo -e "${BLUE}======================================${NC}"
    echo ""
    echo -e "${YELLOW}Timing:${NC}"
    echo "  Cold start (rjest):  ${avg_cold}ms"
    echo "  Warm run (rjest):    ${avg_warm}ms"
    echo ""
    echo -e "${YELLOW}Memory:${NC}"
    echo "  rjest daemon:        $(format_memory $daemon_mem)"
    echo "  rjest total:         $(format_memory $total_mem)"
    echo ""
    echo "(Upstream Jest not available for comparison)"
fi

# Stop daemon
"$RJEST_BIN" --daemon-stop 2>/dev/null || true
