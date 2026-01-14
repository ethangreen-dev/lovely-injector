#!/bin/bash
gamename="Balatro"
exename="$gamename"
defaultpath="/Users/$USER/Library/Application Support/Steam/steamapps/common/$gamename"

export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "$defaultpath"
./$exename.app/Contents/MacOS/love "$@"
