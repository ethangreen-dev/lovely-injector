#!/bin/bash
# This script also works for App Store games.
bundleid="com.playstack.balatroarcade"
executablename="Balatro"
appname="Balatro"
defaultpath="'/Users/$USER/Library/Containers/com.playstack.balatroarcade/Data/Library/Application Support/game'"
gamepath="insert where the .app is with a / at the start but not at the end"
export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "$defaultpath"
$gamepath/$appname.app/Contents/MacOS/$executablename ; exit;
