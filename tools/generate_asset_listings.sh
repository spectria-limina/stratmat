#!/bin/bash

cd $(dirname $0)/../assets

function list_dir {
    find $1 -type f -name "*.$2" | sort >"$1/.listing"
}

list_dir arenas arena.ron
