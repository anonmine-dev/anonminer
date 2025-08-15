# anonminer

A lightweight, high-performance Monero (XMR) CPU miner designed for simplicity and efficiency. Built with Rust for safety, speed, and concurrency.

> **⚠️ Important Warning: Nicehash Not Supported**
>
> Please be aware that **Nicehash is not currently supported** by anonminer. Attempting to use anonminer with Nicehash will result in errors. We are actively working on adding Nicehash support, and it is planned for a future release. Thank you for your patience!

![anonminer screenshot](https://github.com/anonmine-dev/anonminer/blob/main/docs/image.png)

## Features

- **High Performance**: Optimized for modern CPUs with efficient RandomX execution.
- **Low Memory Usage**: Designed with memory-conscious algorithms to reduce footprint.
- **Stratum Support**: Connects directly to mining pools using the Stratum protocol.
- **Real-Time Hashrate Monitoring**: Built-in display for current performance metrics.
- **Multi-Threaded Workers**: Fully concurrent mining using Rust's async runtime.
- **Simple Configuration**: Easy setup with minimal dependencies.

## Getting Started

### Prerequisites

- Rust 1.70+ (`cargo`, `rustc`)
- Reasonable modern CPU (x86_64 is recomended)
- Stable internet connection

### Installation

```bash
git clone https://github.com/anonmine-dev/anonminer.git
cd anonminer
cargo build --release
```

### Usage

```bash
sudo ./anonminer -o gulf.moneroocean.stream:10001 -u YOUR_WALLET_ADDRESS
```

#### Optional Arguments

| Flag | Description | Default |
|------|-------------|---------|
| `-o`/`--url` | Pool address (URL:PORT) | `de.monero.herominers.com:1111` |
| `-u`/`--user` | Wallet address | `41p5Kuj5V4qbkxZ6385kFyWgmwFF3EC5FjmL5JyGoVLbi8wSJBFZPi83cAf5moRrkehu8Bk7dtm9UcsT1662U7Wt7vsysCx` |
| `-p`/`--pass` | Worker name (password) | `x` |
| `-t`/`--threads` | Number of CPU threads | Number of CPU threads |
| `--light` | Switch to light mode | Disabled |
| `--gui` | Enable GUI mode (BETA) | Disabled |
| `--debug_all` | Enable ultra detailed debug output | Disabled |
| `--donate_level` | Developer donation level (percentage, minimum 1%) | `1` |

Example with custom settings:
```bash
./target/release/anonminer \
  -o gulf.moneroocean.stream:10001 \
  -u 46BeWrHpwXmHDpDEUmZBWZfoQpdc6HaERCNmx1pEYLs3rMtrJk2UHwZxNBfLQcMp7uzb7Fq1QgE9Tw4pnNrqGuh6QbA \
  -t 8 \
  --light \
  --donate_level 2
```

## Build from Source

We highly recommend building from source to get a native optimized binary for your system. Prebuilt binaries are available for convenience, but compiling from source ensures the best performance.

Ensure you have the latest stable Rust toolchain and `make` installed:

```bash
# Install Rust (if not already installed)
curl https://sh.rustup.rs | sh

# On Debian/Ubuntu
sudo apt-get update
sudo apt-get install build-essential make

# On Fedora/CentOS/RHEL
sudo dnf groupinstall "Development Tools" # or yum groupinstall "Development Tools"

# On macOS (using Homebrew)
brew install make
```

### Using the Makefile

A `Makefile` is provided for easy compilation across different platforms. It includes highly optimized build settings. The compiled binaries will be placed in the `./bin/` directory.

**Available Make Targets:**

| Command | Description |
|---------|-------------|
| `make build-native` | Builds for the current platform with native CPU optimizations (fastest for the host machine). Binary in `./bin/`. |
| `make build-windows` | Cross-compiles for Windows x86_64 (requires `x86_64-pc-windows-gnu` target). Binary in `./bin/windows/`. |
| `make build-linux` | Cross-compiles for Linux x86_64 (statically linked with musl, requires `x86_64-unknown-linux-musl` target). Binary in `./bin/linux/`. |
| `make build-arm64` | Cross-compiles for ARM64 Linux (statically linked with musl, requires `aarch64-unknown-linux-musl` target). Binary in `./bin/arm64/`. |
| `make clean` | Removes the `target` directory and all build artifacts. |
| `make help` | Shows a list of all available make commands. |

**Example Usage:**

To build for your current machine with maximum optimizations:
```bash
make build-native
```
The optimized binary will be located at `./bin/anonminer`.

To cross-compile for Windows:
```bash
make build-windows
```
The Windows binary will be located at `./bin/windows/anonminer.exe`.

**Note:** For cross-compilation, you may need to install additional linkers or toolchains. For example, on Debian/Ubuntu, you can install the MinGW toolchain for Windows builds with `sudo apt-get install mingw-w64`.

## Performance Tips

- **Linux Recommended**: For the best performance, especially when utilizing huge pages and MSR modifications, we highly recommend running anonminer on a Linux operating system. Linux provides better access and control over these low-level system features, which can significantly impact mining performance.
- Use a local or low-latency mining pool.
- Use the `--light` flag on systems with memory or power constraints.
- Avoid over-threading; usually, CPU core count is optimal. Over-threading can cause context switching and potentially slower performance.
- Run on a system with minimal background load for consistent hashrate.
- For maximum performance, run the miner with `sudo` privileges. This allows the miner to:
  - Automatically configure huge pages (improving memory access speed)
  - Attempt MSR (Model Specific Register) modifications for optimal CPU performance
- Running with `sudo` is optional but may result in substantially higher hash rates, especially on systems where huge pages are not pre-configured.
- Alternatively, you can manually configure your system to allocate sufficient huge pages to achieve similar performance benefits without `sudo`. See your operating system's documentation for instructions on configuring huge pages.
##### NOTE:
- The first 45 seconds is a warmup period where mining occurs but stats are not reported. This time is used to initialize memory, set CPU flags if available, ensure we have a valid job from your pool, and begin hashing. The first 15-30 seconds of RandomX mining have artificially slow hash speeds, so we skip past this period for accurate statistics and to avoid confusing race conditions. You are still hashing, and will be credited for any shares found during this warmup window.
- Some basic parts of this program (debug prints, hash rate calculations, stratum implementations) may have been written with the assistance of AI for code generation and optimization. However, absolutely no aspect of the code that runs with `sudo` permissions (memory optimization, MSR modifications, or any other privileged operations) was written by AI. These security-critical components were written and reviewed entirely by humans to ensure safety and reliability.

## Developer Donation

anonminer includes a default 1% developer donation to support ongoing development and maintenance of the project. This means that for every 200 minutes of mining, 2 minutes of mining time will be directed to a developer-controlled wallet.

- **Default Donation Level**: 1%
- **Adjusting Donation**: You can increase the donation level using the `--donate_level` flag (e.g., `--donate_level 2` for 2%). The minimum donation level is 1%.
- **Removing Donation**: The donation can be removed entirely by modifying the source code. Please refer to `src/main.rs` for details on how the donation mechanism is implemented. We kindly ask that you consider supporting the project if you find the miner useful.

The donation is handled by periodically switching to a pool with the developer's wallet address for a calculated duration. Until our own mining infrastructure is fully operational and Nicehash support is implemented for xmrig-proxy, we have selected a pool that is not among the top 5 largest for these donation periods.

## Future Developments

We are excited to announce that our next-generation mining pools are coming soon at **anonmine.com**. Stay tuned for more details!

## Credits & Acknowledgments

This project was heavily inspired by and derives significant base mining logic from:

- [**emjomi/ministo**](https://github.com/emjomi/ministo) – Much of the core miner architecture and Stratum implementation is based on this work. Special thanks to the author for the great project and amazing reference.

While no code is directly taken, the following project was instrumental in understanding mining optimizations and control flow:

- [**xmrig/xmrig**](https://github.com/xmrig/xmrig) – The gold standard in Monero mining software. Its design patterns, optimization strategies, and protocol handling provided invaluable guidance during development.

## License

Distributed under the Apache License V2 License. See `LICENSE.txt` for more information.

## Donations

If you find this miner useful and choose to remove the built-in donation, or if you'd just like to contribute extra, consider supporting the project directly:

`41p5Kuj5V4qbkxZ6385kFyWgmwFF3EC5FjmL5JyGoVLbi8wSJBFZPi83cAf5moRrkehu8Bk7dtm9UcsT1662U7Wt7vsysCx`

Your support helps maintain and improve the software.
