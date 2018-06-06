all: check_journal.1 check_journal.1.html

check_journal.1 check_journal.1.html: check_journal.1.ronn
	ronn \
	    --manual 'User Commands' \
	    --organization 'Flying Circus Internet Operations' \
	    $<

clean:
	rm check_journal.1 check_journal.1.html

PHONY: all clean
