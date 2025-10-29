# Benchmarks

This directory contains Criterion patch application benchmarks.

## Running Benchmarks

This runs a high number of samples (100) for accurate results, but it will take a while:

```bash
cargo bench --bench patches
```

## Short Mode

For faster iteration during development, use short mode (10 samples):

```bash
LOVELY_BENCH_MODE=short cargo bench --bench patches
```

## Benchmark Groups

- **pattern_no_match**: Pattern patches that won't find matches
- **pattern_with_match**: Pattern patches with matches
- **regex_no_match**: Regex patches that won't find matches
- **regex_with_match**: Regex patches with matches
- **pattern_position**: Pattern patches targeting beginning/middle/end markers
- **regex_position**: Regex patches targeting beginning/middle/end markers

Each group has both long buffer (620KB) and short buffer (50KB) variants.

Additionally, each benchmark sample buffer includes a BEGINNING, MIDDLE, and END marker to test patch apply performance at different positions.

## Visualization

Generate static charts:

```bash
cd crates/lovely-core/benches/scripts
python visualize_results.py
```

Generate comparison report:

This uses the benchmark results from two different runs (e.g., before and after a change) to create an HTML report comparing them. The charts are rendered using Plotly, so they're interactive.

```bash
python compare_benchmarks.py target/criterion/base target/criterion/new report.html
```
