# Fuzzing the PPTX importer

`pptx_import` feeds arbitrary bytes to `pptx::import_pptx` (zip + OOXML parsing, with the import limits). Run it in Docker:

```bash
docker build -t waylamp-present-fuzz src-tauri/fuzz
docker run --rm -v "$PWD:/work" -v waylamp-present-fuzz-target:/fuzz-target -v waylamp-present-fuzz-corpus:/corpus \
  -e CARGO_TARGET_DIR=/fuzz-target waylamp-present-fuzz \
  cargo +nightly fuzz run pptx_import /corpus fuzz/seeds/pptx_import -- -max_total_time=300
```

A crash is saved under `src-tauri/fuzz/artifacts/pptx_import/`; reproduce with `cargo +nightly fuzz run pptx_import <file>`.
