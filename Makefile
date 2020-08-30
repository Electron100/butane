
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
