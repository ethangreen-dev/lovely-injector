#!/bin/bash
gamename="Balatro"
defaultpath="/Users/$USER/Library/Application Support/Steam/steamapps/common/$gamename"

export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "$defaultpath"
./$gamename.app/Contents/MacOS/love "$@"
