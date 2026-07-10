# Makefile para el workspace Midway (Rust + iced)
# Requiere: rustup/cargo instalado. No requiere Node.

CARGO ?= cargo
BIN    := midway-desktop

# WSLg can expose a broken GLES/EGL path that makes Mesa probe Zink before
# wgpu falls back. Prefer the working Vulkan path in WSL only, while keeping
# native defaults elsewhere and honoring `make run WGPU_BACKEND=...`.
IS_WSL := $(shell grep -qi microsoft /proc/sys/kernel/osrelease 2>/dev/null && echo 1)
ifeq ($(IS_WSL),1)
WGPU_BACKEND ?= vulkan
RUN_GRAPHICS_ENV := WGPU_BACKEND=$(WGPU_BACKEND)
endif

.PHONY: help run build release check test fmt fmt-check clippy verify package clean

help: ## Muestra esta ayuda
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

run: ## Levanta la app en modo desarrollo
	$(RUN_GRAPHICS_ENV) $(CARGO) run -p $(BIN)

build: ## Compila el workspace (debug)
	$(CARGO) build --workspace

release: ## Compila el binario de release
	$(CARGO) build --release --bin $(BIN)

check: ## Chequeo rápido de compilación sin generar binarios
	$(CARGO) check --workspace

test: ## Corre toda la suite de tests del workspace
	$(CARGO) test --workspace

fmt: ## Formatea el código (rustfmt)
	$(CARGO) fmt --all

fmt-check: ## Verifica formato sin modificar archivos
	$(CARGO) fmt --all -- --check

clippy: ## Lint con clippy (warnings como errores)
	$(CARGO) clippy --workspace --all-targets -- -D warnings

verify: check test ## Quality gate local equivalente a CI (check + test)

package: release ## Genera instaladores con cargo-packager (requiere cargo-packager instalado)
	$(CARGO) packager --release

clean: ## Limpia artefactos de build
	$(CARGO) clean
