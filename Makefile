
all : build

build :
	cargo build

check : build test
	cargo clippy


test :
	cargo test

clean :
	cargo clean

doc :
	cargo +nightly doc
