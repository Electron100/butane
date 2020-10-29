
all : build

build :
	cargo build
	cargo build --all-features

check : build test doc
	cargo clippy --all-features -- -D warnings


test :
	cargo test

clean :
	cargo clean

doc :
	cd propane && cargo +nightly doc --all-features

docview :
	cd propane && cargo +nightly doc --all-features --open

install :
	cd propane_cli && cargo install --path .
