#!/bin/bash
gamename="Balatro"
defaultpath="/Users/$USER/Library/Application Support/Steam/steamapps/common/$gamename"

workingpath=$(cd "$(dirname "$0")"; pwd)

if ! [ -d "${workingpath}/${gamename}.app" ]; then
    if ! [ -d cd "${defaultpath}/${gamename}.app" ]; then
        echo
        echo "Balatro not found. Did you place this in the right directory?"
        echo
        read -n1 -r -p "Press any key to exit..." key
        exit
    else
        cd "$defaultpath"
    fi
else
    cd "$workingpath"
fi

export DYLD_INSERT_LIBRARIES=liblovely.dylib


./$gamename.app/Contents/MacOS/love "$@"
