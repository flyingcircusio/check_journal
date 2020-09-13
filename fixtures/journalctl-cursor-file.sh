#! /bin/bash -e

# Test dummy for cursor file upgrade
#
# On the first run, journalctl will find an old status file and abort with an
# error message. Then check_journal is expected to remove that file and run
# journalctl again. On the second run, journalctl will save a correcly formatted
# journal cursor.

OPTS=$(getopt -o 'c' --long 'no-pager,since:,cursor-file:' -- "$@")

# Note the quotes around "$TEMP": they are essential!
eval set -- "$OPTS"

while true; do
	case "$1" in
		'--cursor-file')
			if [[ ! -e "$2" || "$(< $2)" == "" ]]; then
				echo "new-format" > $2
				exit
			elif [[ "$(< $2)" != "new-format" ]]; then
				echo "Failed to seek to cursor" >&2
				exit 1
			fi
			shift 2
			;;
		'--since')
			shift 2
			;;
		--)	exit
			;;
		*)
			shift
			;;
	esac
done
