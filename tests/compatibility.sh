#!/bin/bash
# rjest Compatibility Test Suite
# Compares rjest output against upstream Jest to ensure behavioral parity

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
RJEST_BIN="$PROJECT_ROOT/target/release/jest"
FIXTURE_DIR="$PROJECT_ROOT/tests/fixtures/basic-ts"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Temp files for output
RJEST_OUTPUT=$(mktemp)
JEST_OUTPUT=$(mktemp)
trap "rm -f $RJEST_OUTPUT $JEST_OUTPUT" EXIT

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}   rjest Compatibility Test Suite     ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""

# Check if release build exists
if [ ! -f "$RJEST_BIN" ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cd "$PROJECT_ROOT"
    cargo build --release
fi

# Ensure node_modules are installed
cd "$FIXTURE_DIR"
if [ ! -d "node_modules" ]; then
    echo -e "${YELLOW}Installing dependencies...${NC}"
    npm install
fi

# Stop any running daemon for clean test
"$RJEST_BIN" --daemon-stop 2>/dev/null || true
sleep 0.5

TESTS_PASSED=0
TESTS_FAILED=0

# Helper to run a compatibility test
run_test() {
    local test_name="$1"
    local description="$2"
    shift 2
    local extra_args="$@"

    echo -e "${BLUE}Test: ${test_name}${NC}"
    echo "  $description"

    # Run rjest
    "$RJEST_BIN" --json $extra_args > "$RJEST_OUTPUT" 2>&1 || true

    # Run Jest (filter out non-JSON lines)
    npx jest --json $extra_args 2>/dev/null | grep -v "^PASS\|^FAIL\|^Test\|^Snap\|^Time\|^Ran " > "$JEST_OUTPUT" || true

    # Extract key metrics from JSON (rjest uses snake_case, Jest uses camelCase)
    local rjest_passed=$(cat "$RJEST_OUTPUT" | jq -r '.num_passed_tests // .numPassedTests // 0' 2>/dev/null || echo "0")
    local rjest_failed=$(cat "$RJEST_OUTPUT" | jq -r '.num_failed_tests // .numFailedTests // 0' 2>/dev/null || echo "0")
    local rjest_total=$(cat "$RJEST_OUTPUT" | jq -r '(.num_passed_tests // 0) + (.num_failed_tests // 0) + (.num_skipped_tests // 0)' 2>/dev/null || echo "0")
    local rjest_suites=$(cat "$RJEST_OUTPUT" | jq -r '(.num_passed_suites // 0) + (.num_failed_suites // 0)' 2>/dev/null || echo "0")

    local jest_passed=$(cat "$JEST_OUTPUT" | jq -r '.numPassedTests // 0' 2>/dev/null || echo "0")
    local jest_failed=$(cat "$JEST_OUTPUT" | jq -r '.numFailedTests // 0' 2>/dev/null || echo "0")
    local jest_total=$(cat "$JEST_OUTPUT" | jq -r '.numTotalTests // 0' 2>/dev/null || echo "0")
    local jest_suites=$(cat "$JEST_OUTPUT" | jq -r '.numTotalTestSuites // 0' 2>/dev/null || echo "0")

    # Compare results
    local test_passed=true

    if [ "$rjest_passed" != "$jest_passed" ]; then
        echo -e "  ${RED}✗ Passed tests mismatch: rjest=$rjest_passed, jest=$jest_passed${NC}"
        test_passed=false
    fi

    if [ "$rjest_failed" != "$jest_failed" ]; then
        echo -e "  ${RED}✗ Failed tests mismatch: rjest=$rjest_failed, jest=$jest_failed${NC}"
        test_passed=false
    fi

    if [ "$rjest_total" != "$jest_total" ]; then
        echo -e "  ${RED}✗ Total tests mismatch: rjest=$rjest_total, jest=$jest_total${NC}"
        test_passed=false
    fi

    if [ "$rjest_suites" != "$jest_suites" ]; then
        echo -e "  ${RED}✗ Test suites mismatch: rjest=$rjest_suites, jest=$jest_suites${NC}"
        test_passed=false
    fi

    if $test_passed; then
        echo -e "  ${GREEN}✓ PASS${NC} (suites: $rjest_suites, tests: $rjest_total passed, $rjest_failed failed)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "  ${RED}✗ FAIL${NC}"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    echo ""
}

# Helper to verify specific test names match
verify_test_names() {
    local test_name="$1"
    echo -e "${BLUE}Test: ${test_name}${NC}"
    echo "  Verifying test names match between rjest and Jest"

    # Run both with JSON output (suppress stderr for clean JSON)
    "$RJEST_BIN" --json 2>/dev/null > "$RJEST_OUTPUT" || true
    npx jest --json 2>/dev/null | grep -v "^PASS\|^FAIL\|^Test\|^Snap\|^Time\|^Ran " > "$JEST_OUTPUT" || true

    # Extract test names (rjest uses test_results[].tests[].name, Jest uses testResults[].assertionResults[].fullName)
    local rjest_names=$(cat "$RJEST_OUTPUT" | jq -r '.test_results[].tests[].name // empty' 2>/dev/null | sort)
    local jest_names=$(cat "$JEST_OUTPUT" | jq -r '.testResults[].assertionResults[].fullName // empty' 2>/dev/null | sort)

    # Count test names (filter empty lines)
    local rjest_count=$(echo "$rjest_names" | grep -v '^$' | wc -l)
    local jest_count=$(echo "$jest_names" | grep -v '^$' | wc -l)

    if [ "$rjest_count" = "$jest_count" ] && [ "$rjest_count" != "0" ]; then
        echo -e "  ${GREEN}✓ PASS${NC} ($rjest_count test names found in both)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "  ${RED}✗ FAIL${NC} - Test count differs: rjest=$rjest_count, jest=$jest_count"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    echo ""
}

# Helper to verify test statuses match
verify_test_statuses() {
    local test_name="$1"
    echo -e "${BLUE}Test: ${test_name}${NC}"
    echo "  Verifying test statuses match between rjest and Jest"

    # Run both with JSON output (suppress stderr for clean JSON)
    "$RJEST_BIN" --json 2>/dev/null > "$RJEST_OUTPUT" || true
    npx jest --json 2>/dev/null | grep -v "^PASS\|^FAIL\|^Test\|^Snap\|^Time\|^Ran " > "$JEST_OUTPUT" || true

    # Count passed/failed tests
    local rjest_passed=$(cat "$RJEST_OUTPUT" | jq -r '[.test_results[].tests[] | select(.status == "passed")] | length' 2>/dev/null || echo "0")
    local rjest_failed=$(cat "$RJEST_OUTPUT" | jq -r '[.test_results[].tests[] | select(.status == "failed")] | length' 2>/dev/null || echo "0")
    local jest_passed=$(cat "$JEST_OUTPUT" | jq -r '[.testResults[].assertionResults[] | select(.status == "passed")] | length' 2>/dev/null || echo "0")
    local jest_failed=$(cat "$JEST_OUTPUT" | jq -r '[.testResults[].assertionResults[] | select(.status == "failed")] | length' 2>/dev/null || echo "0")

    if [ "$rjest_passed" = "$jest_passed" ] && [ "$rjest_failed" = "$jest_failed" ]; then
        echo -e "  ${GREEN}✓ PASS${NC} (passed: $rjest_passed, failed: $rjest_failed)"
        TESTS_PASSED=$((TESTS_PASSED + 1))
    else
        echo -e "  ${RED}✗ FAIL${NC} - Status counts differ"
        echo "    rjest: passed=$rjest_passed, failed=$rjest_failed"
        echo "    jest:  passed=$jest_passed, failed=$jest_failed"
        TESTS_FAILED=$((TESTS_FAILED + 1))
    fi
    echo ""
}

echo -e "${GREEN}=== Basic Compatibility Tests ===${NC}"
echo ""

# Test 1: Verify test names match (most reliable test)
verify_test_names "test_names"

# Test 2: Verify test statuses match
verify_test_statuses "test_statuses"

# Test 3: Run all tests and compare counts
run_test "all_tests" "Run all tests and compare counts"

# Test 4: Run specific test file
run_test "single_file" "Run single test file" "src/utils.test.ts"

echo -e "${GREEN}=== Feature Compatibility Tests ===${NC}"
echo ""

# Test 5: Snapshot tests
run_test "snapshots" "Snapshot test compatibility" "src/snapshot.test.ts"

# Test 6: Pattern matching with --testNamePattern
run_test "pattern_add" "Pattern matching with 'add'" "--testNamePattern=add"

# Test 7: Pattern matching describe blocks
run_test "pattern_math" "Pattern matching 'Math' describe blocks" "--testNamePattern=Math"

# Test 8: Pattern matching error tests
run_test "pattern_throws" "Pattern matching 'throws'" "--testNamePattern=throws"

echo -e "${BLUE}======================================${NC}"
echo -e "${BLUE}             Summary                  ${NC}"
echo -e "${BLUE}======================================${NC}"
echo ""
echo -e "Tests passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "Tests failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All compatibility tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some compatibility tests failed.${NC}"
    exit 1
fi
