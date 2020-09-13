PREFIX = /usr/local

all: bin doc

bin: target/release/check_journal

target/release/check_journal: Cargo.toml src/*
	cargo build --release

doc: check_journal.1 check_journal.1.html

check_journal.1 check_journal.1.html: check_journal.1.ronn
ifeq ($(shell gem list \^ronn\$$ -i),false)
	gem install ronn
endif
	ronn \
	    --manual 'User Commands' \
	    --organization 'Flying Circus Internet Operations' \
	    $<

test:
	cargo test

install: bin doc
	strip target/release/check_journal
	install -D -t $(DESTDIR)$(PREFIX)/bin target/release/check_journal
	install -D -t $(DESTDIR)$(PREFIX)/share/man/man1 -m 0644 check_journal.1

clean:
	rm check_journal.1 check_journal.1.html
	cargo clean

PHONY: all clean bin doc test install
