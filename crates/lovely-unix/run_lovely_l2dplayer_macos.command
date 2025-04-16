#!/bin/bash
gamename="mari0"
defaultpath="/Users/$USER/Library/Application Support/LOVE/$gamename"
export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "$defaultpath"
/Applications/love.app/Contents/MacOS/love ; exit;
