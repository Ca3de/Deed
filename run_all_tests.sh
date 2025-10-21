#!/bin/bash

# Comprehensive Test Runner for Deed Database
# Runs all tests and generates a summary report

set -e

echo "╔════════════════════════════════════════════════╗"
echo "║   Deed Database - Comprehensive Test Suite    ║"
echo "╚════════════════════════════════════════════════╝"
echo ""

cd deed-rust

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Track results
TOTAL_PASSED=0
TOTAL_FAILED=0

echo "━━━ Running Unit Tests ━━━"
echo ""
if cargo test --lib 2>&1 | tee /tmp/deed_unit_tests.log; then
    UNIT_PASSED=$(grep -c "test result: ok" /tmp/deed_unit_tests.log || echo "0")
    echo -e "${GREEN}✓ Unit tests passed${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + UNIT_PASSED))
else
    echo -e "${RED}✗ Unit tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi
echo ""

echo "━━━ Running Integration Tests ━━━"
echo ""
if cargo test --test integration_tests 2>&1 | tee /tmp/deed_integration_tests.log; then
    echo -e "${GREEN}✓ Integration tests passed${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
else
    echo -e "${RED}✗ Integration tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi
echo ""

echo "━━━ Running Stress Tests ━━━"
echo ""
if cargo test --test transaction_stress_tests 2>&1 | tee /tmp/deed_stress_tests.log; then
    echo -e "${GREEN}✓ Stress tests passed${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
else
    echo -e "${RED}✗ Stress tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi
echo ""

echo "━━━ Running Crash Recovery Tests ━━━"
echo ""
if cargo test --test crash_recovery_tests 2>&1 | tee /tmp/deed_recovery_tests.log; then
    echo -e "${GREEN}✓ Crash recovery tests passed${NC}"
    TOTAL_PASSED=$((TOTAL_PASSED + 1))
else
    echo -e "${RED}✗ Crash recovery tests failed${NC}"
    TOTAL_FAILED=$((TOTAL_FAILED + 1))
fi
echo ""

echo "━━━ Running Performance Benchmarks ━━━"
echo ""
if cargo run --release --bin transaction_benchmarks 2>&1 | tee /tmp/deed_benchmarks.log; then
    echo -e "${GREEN}✓ Benchmarks completed${NC}"
else
    echo -e "${YELLOW}⚠ Benchmarks failed (non-critical)${NC}"
fi
echo ""

# Summary
echo "╔════════════════════════════════════════════════╗"
echo "║   Test Summary                                 ║"
echo "╚════════════════════════════════════════════════╝"
echo ""
echo "Test Suites Passed: $TOTAL_PASSED"
echo "Test Suites Failed: $TOTAL_FAILED"
echo ""

if [ $TOTAL_FAILED -eq 0 ]; then
    echo -e "${GREEN}✓ ALL TESTS PASSED!${NC}"
    echo ""
    echo "Deed database is ready for production use! 🎉"
    exit 0
else
    echo -e "${RED}✗ SOME TESTS FAILED${NC}"
    echo ""
    echo "Please review the test logs for details."
    exit 1
fi
