#!/bin/bash
# Script to generate documentation for DataFlare project

set -e

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Generating documentation for DataFlare project...${NC}"

# List of all crates to document
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

# Function to generate documentation for a crate
generate_docs() {
    local crate=$1
    echo -e "\n${YELLOW}Generating documentation for ${crate}...${NC}"
    
    if [ -d "$crate" ]; then
        cd "$crate"
        cargo doc --no-deps
        local status=$?
        cd ..
        
        if [ $status -eq 0 ]; then
            echo -e "${GREEN}✓ Documentation generated for ${crate}${NC}"
        else
            echo -e "${RED}✗ Failed to generate documentation for ${crate}${NC}"
            return 1
        fi
    else
        echo -e "${YELLOW}Skipping ${crate} (directory not found)${NC}"
    fi
    
    return 0
}

# Generate documentation for all crates
echo -e "\n${YELLOW}=== Generating documentation for all crates ===${NC}"
docs_failed=0

for crate in "${CRATES[@]}"; do
    generate_docs "$crate" || docs_failed=1
done

# Generate workspace documentation
echo -e "\n${YELLOW}=== Generating workspace documentation ===${NC}"
cargo doc --workspace --no-deps
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Workspace documentation generated${NC}"
else
    echo -e "${RED}✗ Failed to generate workspace documentation${NC}"
    docs_failed=1
fi

# Open documentation in browser
echo -e "\n${YELLOW}=== Opening documentation in browser ===${NC}"
cargo doc --open
if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Documentation opened in browser${NC}"
else
    echo -e "${RED}✗ Failed to open documentation in browser${NC}"
    docs_failed=1
fi

# Summary
echo -e "\n${YELLOW}=== Summary ===${NC}"
if [ $docs_failed -eq 0 ]; then
    echo -e "${GREEN}✓ All documentation generated successfully${NC}"
else
    echo -e "${RED}✗ Some documentation generation failed${NC}"
fi

# Exit with error if any generation failed
if [ $docs_failed -eq 1 ]; then
    exit 1
fi

exit 0
