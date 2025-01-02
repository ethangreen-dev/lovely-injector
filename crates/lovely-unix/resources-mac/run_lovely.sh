#!/bin/bash
gamename="Balatro"
workingpath=$(cd "$(dirname "$0")"; pwd)

export DYLD_INSERT_LIBRARIES=liblovely.dylib

cd "${workingpath}"
./$gamename.app/Contents/MacOS/love "$@"
