#!/bin/sh

echo Build all...

for board in examples/*
do
	echo Board $board
	(cd $board ; cargo build --release)
done
