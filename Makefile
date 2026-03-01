# ==============================================================================
# pico-socketeer — Makefile
# Raspberry Pi Pico 2W firmware (Embassy async, no_std, thumbv8m.main-none-eabihf)
# ==============================================================================

# ── Variables (override on command line, e.g. make flash LOG_LEVEL=debug) ───

CHIP            := RP235x
LOG_LEVEL       := info
PORT            := 4242
EMBEDDED_TARGET := thumbv8m.main-none-eabihf
HOST_TARGET     := $(shell rustc -vV | sed -n 's/host: //p')

FIRMWARE_ELF    := target/$(EMBEDDED_TARGET)/release/pico-socketeer
FIRMWARE_DBG    := target/$(EMBEDDED_TARGET)/debug/pico-socketeer
FIRMWARE_UF2    := pico-socketeer.uf2

CYW43_DIR       := cyw43-firmware
CYW43_BASE      := https://raw.githubusercontent.com/embassy-rs/embassy/main/cyw43-firmware

# Flash budget: 4 MB total minus 8 KB CREDENTIALS region (matches CI check)
FLASH_MAX       := 4186112

# ── Default target ───────────────────────────────────────────────────────────

.DEFAULT_GOAL := help

# ── Phony declarations ───────────────────────────────────────────────────────

.PHONY: help
.PHONY: setup setup-env setup-firmware setup-tools
.PHONY: build build-release build-debug
.PHONY: flash run run-debug uf2
.PHONY: test test-host test-client test-integration
.PHONY: lint fmt fmt-check clippy clippy-client
.PHONY: size check-size
.PHONY: ci debug clean

# ── Help ─────────────────────────────────────────────────────────────────────

help:
	@printf '\nUsage: make <target> [VARIABLE=value ...]\n'
	@printf '\nVariables:\n'
	@printf '  CHIP=RP235x          Probe-rs chip name (default: RP235x)\n'
	@printf '  LOG_LEVEL=info       defmt log level: trace|debug|info|warn|error (default: info)\n'
	@printf '  PORT=4242            TCP port the Pico listens on (default: 4242)\n'
	@printf '  PICO_WIFI_SSID=...   Compile-time Wi-Fi SSID (optional, or use .env)\n'
	@printf '  PICO_WIFI_PASS=...   Compile-time Wi-Fi password (optional, or use .env)\n'
	@printf '  PICO_IP=...          Pico IP address (required for test-integration)\n'
	@printf '\nSetup:\n'
	@printf '  setup                Full one-time setup: firmware blobs + .env + tools\n'
	@printf '  setup-firmware       Download CYW43 firmware blobs (idempotent)\n'
	@printf '  setup-env            Copy .env.example → .env (skips if .env exists)\n'
	@printf '  setup-tools          cargo install flip-link + elf2uf2-rs (--locked)\n'
	@printf '\nBuild:\n'
	@printf '  build                Alias for build-release\n'
	@printf '  build-release        cargo build --release (size-optimised, LTO)\n'
	@printf '  build-debug          cargo build (faster iteration build)\n'
	@printf '\nFlash / Run:\n'
	@printf '  flash                build-release then flash via probe-rs (SWD)\n'
	@printf '  run                  Alias for flash\n'
	@printf '  run-debug            build-debug then flash via probe-rs\n'
	@printf '  uf2                  build-release then convert to UF2 drag-and-drop image\n'
	@printf '\nTest:\n'
	@printf '  test                 Run test-host + test-client (no hardware needed)\n'
	@printf '  test-host            Tier 1+2 host unit/mock tests\n'
	@printf '  test-client          pico-socketeer-client crate tests\n'
	@printf '  test-integration     Tier 4 TCP tests (requires PICO_IP=<ip>)\n'
	@printf '\nLint:\n'
	@printf '  lint                 fmt-check + clippy + clippy-client\n'
	@printf '  fmt                  Auto-format all workspace crates\n'
	@printf '  fmt-check            Check formatting without modifying files\n'
	@printf '  clippy               Clippy firmware (embedded target, -D warnings)\n'
	@printf '  clippy-client        Clippy client crate (host target, -D warnings)\n'
	@printf '\nSize / Budget:\n'
	@printf '  size                 Print firmware section sizes (text/data/bss)\n'
	@printf '  check-size           Verify firmware fits within flash budget (4 MB - 8 KB)\n'
	@printf '\nCI:\n'
	@printf '  ci                   Full local CI: fmt-check → clippy → test → build → check-size\n'
	@printf '\nOther:\n'
	@printf '  debug                flash with LOG_LEVEL=debug (verbose RTT output)\n'
	@printf '  clean                cargo clean\n'
	@printf '\n'

# ── Setup ────────────────────────────────────────────────────────────────────

setup: setup-firmware setup-env setup-tools

# File targets — make skips the download if the blobs already exist
$(CYW43_DIR)/43439A0.bin:
	mkdir -p $(CYW43_DIR)
	curl -fsSL $(CYW43_BASE)/43439A0.bin -o $@

$(CYW43_DIR)/43439A0_clm.bin:
	mkdir -p $(CYW43_DIR)
	curl -fsSL $(CYW43_BASE)/43439A0_clm.bin -o $@

setup-firmware: $(CYW43_DIR)/43439A0.bin $(CYW43_DIR)/43439A0_clm.bin

setup-env:
	@if [ -f .env ]; then \
		echo ".env already exists — skipping (edit it manually if needed)"; \
	else \
		cp .env.example .env; \
		echo "Copied .env.example → .env"; \
		echo "Edit .env with your Wi-Fi credentials, then: source .env"; \
	fi

setup-tools:
	cargo install flip-link --locked
	cargo install elf2uf2-rs --locked

# ── Build ────────────────────────────────────────────────────────────────────

build: build-release

build-release:
	DEFMT_LOG=$(LOG_LEVEL) cargo build --release --target $(EMBEDDED_TARGET)

build-debug:
	DEFMT_LOG=$(LOG_LEVEL) cargo build --target $(EMBEDDED_TARGET)

# ── Flash / Run ──────────────────────────────────────────────────────────────

flash: build-release
	probe-rs run --chip $(CHIP) $(FIRMWARE_ELF)

run: flash

run-debug: build-debug
	probe-rs run --chip $(CHIP) $(FIRMWARE_DBG)

uf2: build-release
	elf2uf2-rs $(FIRMWARE_ELF) $(FIRMWARE_UF2)
	@echo "UF2 written to: $(FIRMWARE_UF2)"
	@echo "Hold BOOTSEL, plug in USB, then drag $(FIRMWARE_UF2) onto the RPI-RP2 drive."

# ── Test ─────────────────────────────────────────────────────────────────────

test: test-host test-client

test-host:
	cargo test --test host --no-default-features --target $(HOST_TARGET)

test-client:
	cargo test -p pico-socketeer-client --target $(HOST_TARGET)

test-integration:
	@if [ -z "$(PICO_IP)" ]; then \
		echo "Error: PICO_IP is not set."; \
		echo "Usage: make test-integration PICO_IP=192.168.x.x"; \
		exit 1; \
	fi
	PICO_IP=$(PICO_IP) cargo test --test integration --target $(HOST_TARGET)

# ── Lint ─────────────────────────────────────────────────────────────────────

lint: fmt-check clippy clippy-client

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all --check

clippy:
	cargo clippy --target $(EMBEDDED_TARGET) -- -D warnings

clippy-client:
	cargo clippy -p pico-socketeer-client --target $(HOST_TARGET) -- -D warnings

# ── Size / Flash Budget ───────────────────────────────────────────────────────

size: build-release
	size $(FIRMWARE_ELF)

check-size: build-release
	@TEXT=$$(size $(FIRMWARE_ELF) | awk 'NR==2 {print $$1}'); \
	DATA=$$(size $(FIRMWARE_ELF) | awk 'NR==2 {print $$2}'); \
	USED=$$(( TEXT + DATA )); \
	echo "Flash budget : $(FLASH_MAX) bytes (4 MB - 8 KB credentials)"; \
	echo "Firmware size: $${USED} bytes  (text=$${TEXT}, data=$${DATA})"; \
	echo "Remaining    : $$(( $(FLASH_MAX) - USED )) bytes"; \
	if [ "$$USED" -gt "$(FLASH_MAX)" ]; then \
		echo "FAIL: firmware exceeds flash capacity by $$(( USED - $(FLASH_MAX) )) bytes"; \
		exit 1; \
	else \
		echo "OK: firmware fits within flash budget."; \
	fi

# ── CI ────────────────────────────────────────────────────────────────────────

ci: fmt-check clippy clippy-client test build-release check-size
	@echo "CI simulation complete — all checks passed."

# ── Debug / Clean ─────────────────────────────────────────────────────────────

debug:
	$(MAKE) flash LOG_LEVEL=debug

clean:
	cargo clean
