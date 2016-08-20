all: format build examples test

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

examples: build
	cp target/debug/examples/* bin/

.PHONY: all
