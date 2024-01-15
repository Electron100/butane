CARGO := cargo +stable
CARGO_NIGHTLY := $(subst stable,nightly,$(CARGO))

all : build

build :
	$(CARGO) check
	# build some intermediate configuration to test different feature combinations
	cd butane && $(CARGO) check --features pg
	cd butane && $(CARGO) check --features pg,datetime
	cd butane && $(CARGO) check --features sqlite
	cargo build --all-features

lint :
	$(CARGO) clippy --all-features -- -D warnings


check : build test doclint lint spellcheck


test :
	$(CARGO) test --all-features

clean :
	$(CARGO) clean


spellcheck :
	typos

doclint :
	RUSTDOCFLAGS="-D warnings" $(CARGO_NIGHTLY) doc --no-deps --all-features

doc :
	cd butane && $(CARGO_NIGHTLY) doc --no-deps --all-features

docview :
	cd butane && $(CARGO_NIGHTLY) doc --all-features --open

install :
	cd butane_cli && $(CARGO) install --path .
