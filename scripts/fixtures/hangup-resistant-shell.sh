#!/bin/sh
# Test-only SHELL override. Ignored signals survive exec into the interactive shell.
trap '' HUP
printf 'Shutdown fixture: shell %s ignores SIGHUP\r\n' "$$"
ENV=/dev/null
PS1='$ '
export ENV PS1
exec /bin/sh -i
