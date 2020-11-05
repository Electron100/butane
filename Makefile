
all : build

build :
	cargo build
	# build some intermediate configuration to test different feature combinations
	cd propane && cargo build --features pg
	cd propane && cargo build --features pg,datetime
	cd propane && cargo build --features sqlite
	cargo build --all-features


check : build test doc
	cargo clippy --all-features -- -D warnings


test :
	cargo test --all-features

clean :
	cargo clean

doc :
	cd propane && cargo +nightly doc --all-features

docview :
	cd propane && cargo +nightly doc --all-features --open

install :
	cd propane_cli && cargo install --path .
