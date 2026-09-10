#!/usr/bin/env bash
# Idempotent Cloud Agent bootstrap for Tuxedo (Rust crate + tuxedo-r Ruby gem).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# --- System packages: Ruby (for the tuxedo-r gem) + a C toolchain (native gems) ---
if ! command -v ruby >/dev/null 2>&1; then
  sudo DEBIAN_FRONTEND=noninteractive apt-get update -qq
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    ruby-full build-essential
fi

if ! command -v bundle >/dev/null 2>&1; then
  sudo gem install --no-document bundler
fi

# --- Rust: edition 2024 requires a recent stable toolchain (>= 1.85) ---
rustup toolchain install stable --profile minimal --component rustfmt --component clippy
rustup default stable

# --- Build the tuxedo binary so target/ is warm ---
cargo build --release

# --- Ruby gem dependencies (installed project-locally, gitignored) ---
cd "$ROOT/ruby"
bundle config set --local path vendor/bundle
bundle install
