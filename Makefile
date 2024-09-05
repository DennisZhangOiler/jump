build:
	cargo build --release

install: build
	mkdir -p ~/.jump
	cargo build --release
	./target/release/jump initialize
	sudo cp ./target/release/jump /usr/local/bin/