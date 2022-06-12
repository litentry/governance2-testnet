#!/bin/bash

$HOME/.cargo/bin/cargo build --release
ln -sf $HOME/actions-runner/_work/governance2-testnet/governance2-testnet/release/governance2 /opt/governance2
echo "ARGS=\"--tmp --dev --unsafe-rpc-external --unsafe-ws-external --rpc-methods unsafe --rpc-cors all\"" > /opt/governance2.conf
sudo systemctl restart governance2-testnet
