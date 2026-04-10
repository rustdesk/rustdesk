#!/bin/bash
# BVAiDesk Branding Patch - Run once after forking
set -e
echo "=== BVAiDesk Branding Patch ==="

cd "$(dirname "$0")/.."

# Patch APP_NAME in config
if [ -f "libs/hbb_common/src/config.rs" ]; then
    sed -i 's/RwLock::new("RustDesk"/RwLock::new("BVAiDesk"/g' libs/hbb_common/src/config.rs
    echo "Patched: libs/hbb_common/src/config.rs"
fi

# Cargo.toml (if needed) - handled by submodule commit
echo "=== Done ==="
echo "Commit the changes: git add libs/hbb_common && git commit -m 'Apply BVAiDesk branding'"