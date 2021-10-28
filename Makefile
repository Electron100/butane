
all : build

build :
	cargo check
	# build some intermediate configuration to test different feature combinations
	cd butane && cargo check --features pg
	cd butane && cargo check --features pg,datetime
	cd butane && cargo check --features sqlite
	cargo build --all-features

lint :
	cargo clippy --all-features -- -D warnings


check : build test doc lint


test :
	cargo test --all-features
	# mirror the CI run which doesn't do pg right now
	cd butane && cargo build --features "default,sqlite" --tests

clean :
	cargo clean

doc :
	cd butane && cargo +nightly doc --all-features

docview :
	cd butane && cargo +nightly doc --all-features --open

install :
	cd butane_cli && cargo install --path .
