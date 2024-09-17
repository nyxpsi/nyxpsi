[![Rust](https://github.com/nyxpsi/nyxpsi/actions/workflows/rust.yml/badge.svg)](https://github.com/nyxpsi/nyxpsi/actions/workflows/rust.yml)
# nyx-ψ

**nyx-ψ** _(nyxpsi)_ is a next-generation network implementation designed for resilience and efficiency in lossy and unstable network environments. Through innovative networking strategies and error correction mechanisms, **nyx-ψ** delivers reliable data transfer where traditional protocols like TCP and UDP fall short.

Built with scalability and robustness in mind, **nyx-ψ** aims to empower applications that demand high reliability and performance, even in the face of extreme packet loss.
Results Summary

## Prerequisites

Before building and running **nyx-ψ**, ensure that your development environment meets the following requirements:

- **Rust Compiler**: Version **1.74** or newer is required.

*Update* your toolchain using rustup if necessary. 
```bash
rustup update
rustc --version  // >= 1.74
```

## Benchmark Results

We conducted benchmarks comparing **nyx-ψ**, TCP, and UDP under various packet loss scenarios. The test involved transferring 1MB of data under different network conditions. You can conduct your own with `cargo bench`

### Results Summary

| Protocol | 0% Loss | 10% Loss | 50% Loss |
|----------|---------|----------|----------|
| nyx-ψ    | 1.07s (100%) | 1.07s (100%) | 1.07s (100%) |
| TCP      | 1.04s (100%) | 0.93s (0%)   | 0.52s (0%)   |
| UDP      | 1.07s (100%) | 5.05s (0%)   | 5.64s (0%)   |

*Note: Values represent average transfer time. Percentages in parentheses indicate transfer success rate.*

For more information or to contact us open a PR or email us at nyxpsi@skill-issue.dev
