#!/bin/bash
gamename="Balatro"

workingpath=$(cd "$(dirname "$0")"; pwd)
assetpath="${workingpath}/assets/"

if ! [ -d "${workingpath}/${gamename}.app" ]; then
    echo
    echo "Balatro not found. Did you place everything in the right directory?"
    echo
    read -n1 -r -p "Press any key to exit..." key
    exit
fi

gamebundle="${workingpath}/${gamename}.app"

bundlepath="/Users/$USER/Applications/${gamename} + Mods.app"
contentpath="${bundlepath}/Contents"

echo
echo "Generating bundle..."
mkdir -p "${contentpath}"
cd "${contentpath}"
mkdir MacOS
mkdir Resources

echo
echo "Copying assets..."
ln -s "${gamebundle}" ./MacOS/
mv "${assetpath}/run_lovely.sh" ./MacOS/run_lovely
mv "${assetpath}/info.plist" ./
mv "${workingpath}/liblovely.dylib" ./MacOS/
mv "${assetpath}/application.icns" ./Resources/

chmod +x ./MacOS/run_lovely
echo "Signing application..."
codesign -s - --deep "${bundlepath}/Contents/MacOS/run_lovely"

echo
echo "Cleaning up..."
rm "${workingpath}/install_lovely.sh"
rm -rf "${workingpath}/assets"

echo
read -n1 -r -p "Installation complete! Press any key to exit..." key
open /Users/$USER/Applications
exit