.PHONY: all build-native build-windows build-linux build-arm64 clean

# Default target: build for the native platform
all: build-native

# Define binary name and output directories
BINARY_NAME = anonminer
BIN_DIR = ./bin
NATIVE_BIN_DIR = $(BIN_DIR)
WINDOWS_BIN_DIR = $(BIN_DIR)/windows
LINUX_BIN_DIR = $(BIN_DIR)/linux
ARM64_BIN_DIR = $(BIN_DIR)/arm64

# Create output directories
$(shell mkdir -p $(NATIVE_BIN_DIR) $(WINDOWS_BIN_DIR) $(LINUX_BIN_DIR) $(ARM64_BIN_DIR))

# Build for the native platform (highly optimized)
build-native:
	CARGO_BUILD_RUSTFLAGS="-C target-cpu=native" cargo build --release
	cp target/release/$(BINARY_NAME) $(NATIVE_BIN_DIR)/

# Build for Windows x86_64 using GNU toolchain
build-windows:
	rustup target add x86_64-pc-windows-gnu
	CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-g++ RUSTFLAGS="-C target-feature=+crt-static -l stdc++ -l advapi32" cargo build --release --target x86_64-pc-windows-gnu
	cp target/x86_64-pc-windows-gnu/release/$(BINARY_NAME).exe $(WINDOWS_BIN_DIR)/

# Build for Linux x86_64 (dynamically linked)
build-linux:
	cargo build --release --target x86_64-unknown-linux-gnu
	cp target/x86_64-unknown-linux-gnu/release/$(BINARY_NAME) $(LINUX_BIN_DIR)/

# Build for ARM64 Linux (musl for static linking)
build-arm64:
	rustup target add aarch64-unknown-linux-musl
	cargo build --release --target aarch64-unknown-linux-musl
	cp target/aarch64-unknown-linux-musl/release/$(BINARY_NAME) $(ARM64_BIN_DIR)/

# Clean the project
clean:
	cargo clean
	rm -rf $(BIN_DIR)

# Help target to show available commands
help:
	@echo "Makefile for anonminer"
	@echo "Usage:"
	@echo "  make build-native  - Build for the current platform (optimized). Binary in $(NATIVE_BIN_DIR)/"
	@echo "  make build-windows - Build for Windows x86_64 (GNU). Binary in $(WINDOWS_BIN_DIR)/"
	@echo "  make build-linux   - Build for Linux x86_64 (musl). Binary in $(LINUX_BIN_DIR)/"
	@echo "  make build-arm64   - Build for ARM64 Linux (musl). Binary in $(ARM64_BIN_DIR)/"
	@echo "  make clean         - Remove the target directory and the $(BIN_DIR) directory"
	@echo "  make help          - Show this help message"
