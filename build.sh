#!/bin/zsh
# Builds every board's demo, or those named, e.g. ./build.sh seeed-esp32c6-xiao, and copies each to the shared firmware
# folder as bruce-embedded-std's dev version, see tools/bruce-embedded-firmware, where bruce-embedded-flash's window
# and apps which flash boards, e.g. Marco Sparko, find it.
set -e          # exit on error
set -u          # exit on undefined variable
set -o pipefail # exit if any command in a pipe fails

# The shared firmware folder, as bruce-embedded-firmware's folder() finds it.
if [[ -n "${BRUCE_EMBEDDED_FIRMWARE:-}" ]]; then
	folder=$BRUCE_EMBEDDED_FIRMWARE
elif [[ "$(uname)" = Darwin ]]; then
	folder="$HOME/Library/Application Support/bruce-embedded-std/firmware"
else
	folder="${XDG_DATA_HOME:-$HOME/.local/share}/bruce-embedded-std/firmware"
fi
dev="$folder/bruce-embedded-std/dev"
mkdir -p "$dev"

echo Build all...

for board in boards/*
do
	if (( $# > 0 )) && (( ! ${@[(Ie)${board:t}]} )); then
		continue
	fi
	echo Board $board
	(cd $board ; cargo build --release)
	# The binary, named as its package, in its target's folder, from the board's own cargo config.
	name=$(sed -n 's/^name = "\(.*\)"/\1/p' $board/Cargo.toml | head -1)
	target=$(sed -n 's/^target = "\(.*\)"/\1/p' $board/.cargo/config.toml | head -1)
	cp "$board/target/$target/release/$name" "$dev/$name.elf"
done
ls -l "$dev"
