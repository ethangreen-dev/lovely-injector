#!/bin/bash
gamename="Balatro"

# Get the absolute path to where we are running from
SCRIPT_FOLDER="$(dirname -- "${BASH_SOURCE[0]}")"
APP_FOLDER="$(dirname -- "$SCRIPT_FOLDER")"

# Specify the complete path to the library in the app bundle
export DYLD_INSERT_LIBRARIES="$APP_FOLDER/Resources/liblovely.dylib"

defaultpath="/Users/$USER/Library/Application Support/Steam/steamapps/common/$gamename"

cd "$defaultpath"
./$gamename.app/Contents/MacOS/love "$@"