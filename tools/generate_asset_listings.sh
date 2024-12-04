#!/bin/sh

cd $(dirname $0)/../assets

function list_dir {
    find $1 -type f -name "*.$2" | sort >"$(dirname $1)/$(basename $1).listing"
}

list_dir arenas arena.ron
