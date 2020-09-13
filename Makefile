PREFIX = /usr/local

all: bin man

bin: target/release/check_journal

target/release/check_journal: Cargo.toml src/*
	cargo build --release

man: man/check_journal.1 man/check_journal.1.html

man/check_journal.1 man/check_journal.1.html: man/check_journal.1.ronn
	ronn \
	    --manual 'User Commands' \
	    --organization 'Flying Circus Internet Operations' \
	    $<

test:
	cargo test

install: bin man
	strip target/release/check_journal
	install -D -t $(DESTDIR)$(PREFIX)/bin target/release/check_journal
	install -D -t $(DESTDIR)$(PREFIX)/share/man/man1 -m 0644 man/check_journal.1
	install -D -t $(DESTDIR)$(PREFIX)/share/doc/check_journal -m 0644 README.md

clean:
	rm -f man/check_journal.1 man/check_journal.1.html result *.snap
	cargo clean

PHONY: all clean bin man test install
