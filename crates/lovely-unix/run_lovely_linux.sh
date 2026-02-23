#!/bin/sh
gamename="Balatro"
exename="$gamename"
defaultpath="$HOME/.local/share/Steam/steamapps/common/$gamename"

cd "$defaultpath"
LD_PRELOAD=liblovely.so love $exename.exe "$@"
