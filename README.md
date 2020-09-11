# check_journal

[![check-journal](https://snapcraft.io//check-journal/badge.svg)](https://snapcraft.io/check-journal)


Nagios/Icinga compatible plugin to search `journalctl` output for matching lines.


## Usage

check_journal takes a YAML document with regular expressions for matches and
exceptions. Example:

```
criticalpatterns:
  - '[Aa]bort|ABORT'
  - '[Ee]rror|ERROR'

criticalexceptions:
  - 'timestamp:".*",level:"(error|warn)"'
  - '0 errors'

warningpatterns:
  - '[Ff]ail|FAIL'
  - '[Ww]arn|WARN'

warningexceptions:
  - '0 failures'
  - 'graylogctl'
  - 'node\[.*\]: Exception'
```

check_journal reports a CRITICAL result if any one of `criticalpatterns` and
none of `criticalexceptions` matches. If there is not critical match, the same
procedure is repeated for WARNING.

It is stongly recommended to pass a state file with the `-f` option. The state
file helps check_journal to resume exactly where it stopped on the last run so
that no log line is reported twice.


## Installation

Standard Rust build procedures apply. Basically, invoke
```
cargo build --release
```
to obtain a binary.

A Makefile is included which also builds the manpage. To compile and install
under `/usr/local`, invoke
```
make install PREFIX=/usr/local
```

Build requirements:

* *Rust* >= 1.40
* *ronn* for compiling the man page

## Packaging

The plugin can be released as a snap package by running
```bash
snapcraft clean
snapcraft
```

#### Installing the snap
Once released, this will download the snap from the snapstore and install
on the machine.
```bash
snap install check-journal
```

#### Running the snap
```bash
check-journal
# -- or -- #
snap run check-journal
```


## Journal permissions

The plugin, which is usually running under the *nagios* user, must be able to
access the journal. The recommended way to achieve this is:

1. Grant members of the *adm* group access to the journal:
      `setfacl -Rnm g:adm:rx,d:g:adm:rx /var/log/journal` -- see
      systemd-journald.service(8) for details. Some distributions already have
      that ACL set by default.

2. Add the *nagios* user to the *adm* group.


## Author

The primary author is [Christian Kauhaus](mailto:kc@flyingcircus.io).


## License

This program is distributed under the terms of the [BSD 3-Clause Revised
License](https://opensource.org/licenses/BSD-3-Clause).
