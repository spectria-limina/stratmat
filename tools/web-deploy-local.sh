#!/bin/sh

PROFILE=debug

echo 'Generating stratmat_bg.wasm...'
wasm-bindgen --no-typescript --target web --out-dir public/static/app --out-name stratmat target/wasm32-unknown-unknown/$PROFILE/stratmat.wasm
ln -fs $(realpath assets) public/static/app/assets