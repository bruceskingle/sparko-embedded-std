#!/bin/zsh
set -e          # exit on error
set -u          # exit on undefined variable
set -o pipefail # exit if any command in a pipe fails

echo Build all...

for board in boards/*
do
	echo Board $board
	(cd $board ; cargo build --release)
done
