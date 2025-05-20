#!/bin/bash

while true; do
    echo "Select a game:"
    echo "1) Balatro"
    echo "2) Beatblock"
    echo "3) Custom"
    read -p "Enter choice [1-3]: " choice

    case "$choice" in
        1)
            gamename="Balatro"
            break
            ;;
        2)
            gamename="Beatblock Demo"
            break
            ;;
        3)
            read -p "Enter custom game name: " gamename
            break
            ;;
        *)
            echo "Invalid choice. Please select 1, 2, or 3."
            ;;
    esac
done

defaultpath="/Users/$USER/Library/Application Support/Steam/steamapps/common/$gamename"

export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "$defaultpath" || { echo "Failed to change directory to $defaultpath"; exit 1; }

"./$gamename.app/Contents/MacOS/love" "$@"
