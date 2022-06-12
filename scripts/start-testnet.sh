#!/bin/bash

pid=$(ps aux | grep governance2 | grep -v grep | awk ${print '$2'})
echo "Running governance2 pid is: $pid"

[ ! -z $pid ] && kill -9 $pid

echo "Start a new testnet with --tmp option ..."
./target/release/governance2 --tmp --dev --unsafe-rpc-external --unsafe-ws-external --rpc-methods unsafe --rpc-cors all
