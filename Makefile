.PHONY: all release debug clean run run_debug
all: release debug

release:
	@cmake --preset=macos-clang-release
	@cmake --build --preset=release

debug:
	@cmake --preset=macos-clang-debug
	@cmake --build --preset=debug

clean:
	@rm -rf build
	@rm -rf main
	@rm -rf main_debug
	@rm -rf src/include/lexer.hpp

run:
	@./build/release/bin/Prim

run_debug:
	@./build/debug/bin/Prim