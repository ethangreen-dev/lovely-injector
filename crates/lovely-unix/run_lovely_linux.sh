#!/bin/sh
gamename="Balatro"
defaultpath="/home/$USER/.local/share/Steam/steamapps/common/$gamename"

cd "$defaultpath"
LD_PRELOAD=liblovely.so love $gamename.exe "$@"
