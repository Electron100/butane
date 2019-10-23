
all : build

build :
	cargo build
	cargo build --all-features

check : build test
	cargo clippy --all-features


test :
	cargo test

clean :
	cargo clean

doc :
	cargo +nightly doc --all-features
