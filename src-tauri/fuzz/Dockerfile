# Fuzzing needs nightly + libFuzzer, which is Unix-only; run it here instead of on Windows.
FROM rust:1-bookworm
RUN rustup toolchain install nightly --profile minimal && cargo install cargo-fuzz --locked
WORKDIR /work/src-tauri
