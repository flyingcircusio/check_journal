# check_journal

Nagios/Icinga compatible plugin to search `journalctl` output for matching lines.

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
