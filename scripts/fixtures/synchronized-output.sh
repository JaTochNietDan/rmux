#!/bin/sh
# Run inside rmux: timeout text should appear while the producer is sleeping.
set -eu
trap 'printf "\033[?2026l"' EXIT HUP INT TERM
printf '\033[2J\033[HBaseline before hold\r\n'
sleep 1
printf '\033[?2026hTimeout recovered without more output\r\n'
sleep 10
printf '\033[?2026lProducer released hold\r\n'
