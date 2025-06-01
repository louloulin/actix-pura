#!/bin/bash
# Script to run code quality checks on DataFlare project

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Running code quality checks for DataFlare project...${NC}"

# List of all crates to check
CRATES=(
    "dataflare-core"
    "dataflare-runtime"
    "dataflare-connector"
    "dataflare-processor"
    "dataflare-plugin"
    "dataflare-state"
    "dataflare-edge"
    "dataflare-cloud"
    "dataflare-cli"
    "dataflare"
)

# Function to run clippy on a crate
run_clippy() {
    local crate=$1
    echo -e "\n${YELLOW}Running clippy on ${crate}...${NC}"
    
    if [ -d "$crate" ]; then
        cd "$crate"
        cargo clippy -- -D warnings
        local status=$?
        cd ..
        
        if [ $status -eq 0 ]; then
            echo -e "${GREEN}✓ Clippy passed for ${crate}${NC}"
        else
            echo -e "${RED}✗ Clippy failed for ${crate}${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}Skipping ${crate} (directory not found)${NC}"
    fi
    
    return 0
}

# Function to run rustfmt on a crate
run_fmt() {
    local crate=$1
    echo -e "\n${YELLOW}Running rustfmt on ${crate}...${NC}"
    
    if [ -d "$crate" ]; then
        cd "$crate"
        cargo fmt --check
        local status=$?
        cd ..
        
        if [ $status -eq 0 ]; then
            echo -e "${GREEN}✓ Formatting is correct for ${crate}${NC}"
        else
            echo -e "${RED}✗ Formatting issues found in ${crate}${NC}"
            echo -e "${YELLOW}Run 'cargo fmt' in ${crate} to fix formatting issues${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}Skipping ${crate} (directory not found)${NC}"
    fi
    
    return 0
}

# Run clippy on all crates
echo -e "\n${YELLOW}=== Running Clippy on all crates ===${NC}"
clippy_failed=0

for crate in "${CRATES[@]}"; do
    run_clippy "$crate" || clippy_failed=1
done

# Run rustfmt on all crates
echo -e "\n${YELLOW}=== Checking code formatting with rustfmt ===${NC}"
fmt_failed=0

for crate in "${CRATES[@]}"; do
    run_fmt "$crate" || fmt_failed=1
done

# Summary
echo -e "\n${YELLOW}=== Summary ===${NC}"
if [ $clippy_failed -eq 0 ]; then
    echo -e "${GREEN}✓ All clippy checks passed${NC}"
else
    echo -e "${RED}✗ Some clippy checks failed${NC}"
fi

if [ $fmt_failed -eq 0 ]; then
    echo -e "${GREEN}✓ All formatting checks passed${NC}"
else
    echo -e "${RED}✗ Some formatting checks failed${NC}"
fi

# Exit with error if any check failed
if [ $clippy_failed -eq 1 ] || [ $fmt_failed -eq 1 ]; then
    exit 1
fi

exit 0
