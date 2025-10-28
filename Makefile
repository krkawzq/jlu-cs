.PHONY: all release debug clean run run_debug lib
all: release debug

lib:
	@echo "Setting up local dependencies..."
	@mkdir -p lib
	@if [ ! -d "lib/fmt" ]; then \
		echo "Cloning fmt..."; \
		git clone --depth 1 --branch 11.0.0 https://github.com/fmtlib/fmt.git lib/fmt; \
	else \
		echo "fmt already exists"; \
	fi
	@if [ ! -d "lib/spdlog" ]; then \
		echo "Cloning spdlog..."; \
		git clone --depth 1 --branch v1.13.0 https://github.com/gabime/spdlog.git lib/spdlog; \
	else \
		echo "spdlog already exists"; \
	fi
	@if [ ! -d "lib/magic_enum" ]; then \
		echo "Cloning magic_enum..."; \
		git clone --depth 1 --branch v0.9.6 https://github.com/Neargye/magic_enum.git lib/magic_enum; \
	else \
		echo "magic_enum already exists"; \
	fi
	@echo "Dependencies ready!"

release:
	@cmake --preset=macos-clang-release
	@cmake --build --preset=release

debug:
	@cmake --preset=macos-clang-debug
	@cmake --build --preset=debug

clean:
	@rm -rf build
	@rm -rf src/include/lexer.hpp
	@rm ./release
	@rm ./debug

run:
	@./build/debug/bin/Prim

run-release:
	@./build/release/bin/Prim

lexer:
	@re2c -W -i -c -o src/include/lexer.hpp src/lexer.re

parser:
	@/opt/homebrew/opt/bison/bin/bison -Wcounterexamples -d -o src/parser.c src/parser.y
	@mv src/parser.h src/include/parser.h

fake:
	@for src in $(wildcard .fake/*.c); do \
		bin=$${src%.c}; \
		echo "Compiling $$src to $$bin"; \
		clang "$$src" -o "$$bin"; \
	done