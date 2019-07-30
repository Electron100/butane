
all : build

build :
	cargo build

clean :
	cargo clean

doc :
	cargo +nightly doc
