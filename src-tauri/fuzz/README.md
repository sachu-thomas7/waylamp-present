# Fuzzing the PPTX importer

`pptx_import` feeds arbitrary bytes to `pptx::import_pptx` (zip + OOXML parsing, with the import limits). Run it in Docker:

```bash
docker build -t streamdeck-fuzz src-tauri/fuzz
docker run --rm -v "$PWD:/work" -v streamdeck-fuzz-target:/fuzz-target -v streamdeck-fuzz-corpus:/corpus \
  -e CARGO_TARGET_DIR=/fuzz-target streamdeck-fuzz \
  cargo +nightly fuzz run pptx_import /corpus fuzz/seeds/pptx_import -- -max_total_time=300
```

A crash is saved under `src-tauri/fuzz/artifacts/pptx_import/`; reproduce with `cargo +nightly fuzz run pptx_import <file>`.
