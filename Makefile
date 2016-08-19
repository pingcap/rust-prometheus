all: format build test

build:
	cargo build --features default

test:
	cargo test --features default -- --nocapture 

dev: format
	cargo build --features dev
	cargo test --features dev -- --nocapture 

format:
	cargo fmt -- --write-mode overwrite

clean:
	cargo clean

demo:
	mkdir -p bin
	cargo build --bin demo
	cp target/debug/demo bin/

.PHONY: demo all
