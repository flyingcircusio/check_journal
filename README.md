# check_journal

Nagios/Icinga compatible plugin to search `journalctl` output for matching lines.

## Usage

check_journal takes a YAML document with regular expressions for matches and exceptions. Example:

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

check_journal reports a CRITICAL result if any one of `criticalpatterns` and no
one of `criticalexceptions` matches. If there is not critical match, the same
procedure is repeated for WARNING.

`journalctl` is invoked with a `--since` parameter (time span is configurable)
so that log lines are not reported multiple times for recurrent runs of
check_journal. See the man page for more options.

## Building

Standard Rust build procedures apply. Basicall, invoke

> cargo build --release

to obtain a binary. A Makefile is included for convenience which also builds the
manpage.

Build requirements:

* *Rust* >= 1.41
* *ronn* for compiling the man page

## Author

The primary author is Christian Kauhaus <kc@flyingcircus.io>.

## License

This program is distributed under the terms of the [BSD 3-Clause Revised
License](https://opensource.org/licenses/BSD-3-Clause).
