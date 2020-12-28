
all : build

build :
	cargo build
	# build some intermediate configuration to test different feature combinations
	cd butane && cargo build --features pg
	cd butane && cargo build --features pg,datetime
	cd butane && cargo build --features sqlite
	cargo build --all-features


check : build test doc
	cargo clippy --all-features -- -D warnings


test :
	cargo test --all-features

clean :
	cargo clean

doc :
	cd butane && cargo +nightly doc --all-features

docview :
	cd butane && cargo +nightly doc --all-features --open

install :
	cd butane_cli && cargo install --path .
