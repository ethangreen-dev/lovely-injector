#!/bin/sh
gamename="Balatro"
exename="$gamename"
defaultpath="/home/$USER/.local/share/Steam/steamapps/common/$gamename"

cd "$defaultpath"
LD_PRELOAD=liblovely.so love $exename.exe "$@"
