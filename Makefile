
all : build

build :
	cargo build

check : build test

test :
	cargo test

clean :
	cargo clean

doc :
	cargo +nightly doc
