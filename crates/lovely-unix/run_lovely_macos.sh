#! /bin/bash
shopt -s extglob

# GLOBAL VARIABLES
    # Paths
        workingPath=$(cd "$(dirname "$0")"; pwd)
        liblovelyPath="${workingPath}/liblovely.dylib"
        resourcePath="${workingPath}/Resources"
        userAppsPath="/Users/${USER}/Applications"
        propertyFile="${resourcePath}/Info.plist"
        bundlePath=""
        binaryPath=""
        iconPath=""
    # App Bundle Information
        appInfo="" # ${bundlePath}/Contents/Info.plist
        appName="" # CFBundleName
        appNameSuffix="+ Mods"
        appID="" # CFBundleIdentifier
        appIDSuffix=".modded"
        appIcon="" # CFBundleIconFile
        appSignature="" # CFBundleSignature
        appBinary="" # CFBundleExecutable
        appCopyright="" # NSHumanReadableCopyright
        appGraphicsSwitching="" # NSSupportsAutomaticGraphicsSwitching
        appRegion="" # CFBundleDevelopmentRegion
        appRetina="" # NSHighResolutionCapable
        appTarget="" # LSMinimumSystemVersion
    # Options
        useCustomIcon=false
        useDefaultIcon=true
        overwriteBundle=false
    # Other
        binaryName=""

# Main Menu Loop
menuOption() {
    read -n 1 -s method
    case $method in
        1)
            # User wishes to inject lovely and run once
            injectBundle
            ;;
        2)
            # User wishes to build a bundle to repeatedly inject and run lovely
            buildBundle
            ;;
        3)
            # User wishes to exit
            exit
            ;;
        *)
            # Invalid input; Loop around
            echo "Invalid option. Please try again."
            echo
            menuOption
        esac
}

# Get source application path
getApplication() {
    echo
    echo "Please drag-and-drop the .app bundle to be modded, or enter its location below, then press enter."
    echo
    read -p "Path to Application: " inputPath
    echo
    if [[ -d "${inputPath}" ]]; then
        if [[ $inputPath == *.[Aa][Pp][Pp] ]]; then
            
            # Set path information
            bundlePath="${inputPath}"
            appInfo="${bundlePath}/Contents/Info.plist"
            binaryName=$(defaults read "${appInfo}" CFBundleExecutable)
            appBinary=$(basename "$bundlePath")"/Contents/MacOS/${binaryName}"
        else
            # File is not an app bundle
            echo
            echo "The provided input is not a valid macOS application bundle."
            bundlePath=""
            return
        fi
    else
        # File not found
        echo
        echo "The provided file could not be found. Please check the path and try agian."
        bundlePath=""
        return
    fi
}

# Run & inject without building
injectBundle() {
    getApplication

    # Return in the event no source bundle was set.
    if [[ bundlePath == "" ]]; then
        return
    fi
    
    export DYLD_INSERT_LIBRARIES=liblovely.dylib
    echo
    eval "${bundlePath// /\\ }/Contents/MacOS/${binaryName} "'"@"'
    clearVariables
}

# Create bundle launcher
buildBundle() {
    getApplication

    # Return in the event no source bundle was set.
    if [[ bundlePath == "" ]]; then
        return
    fi

    if [[ -f "${propertyFile}.original" ]]; then
        # If a backup property file exists, assume script exited in a failed state previously, and restore backup
        cp -f "${propertyFile}.original" "${propertyFile}"
    else
        # Create a backup property file
        cp -f "${propertyFile}" "${propertyFile}.original"
    fi

    echo
    echo "Setting bundle information..."
    
    # Use 2> /dev/null to silence errors and make checks easier to script

    # Bundle Name
    appName=$(defaults read "${appInfo}" CFBundleName 2> /dev/null)
    if [[ $appName == "" ]]; then
        appName="LÖVE 2d Application"
    fi
    defaults write "${propertyFile}" CFBundleName "${appName}"
    
    # Bundle Identifier
    appID=$(defaults read "${appInfo}" CFBundleIdentifier 2> /dev/null)   
    if [[ $appID == "" ]]; then
        echo "Error: Bundle identifier missing. (CFBundleIdentifier) Are you sure this is a valid application?"
        exitScript
    fi
    defaults write "${propertyFile}" CFBundleIdentifier "${appID}${appIDSuffix}"
    
    # Ask for user icon path with read and set appIcon to it
    askForIcon
    
    # If a custom icon is not in use...
    if [[ $useCustomIcon == false ]]; then
        if [[ -f "${resourcePath}/Icons/${appID}.icns" ]]; then
            # ...Use tailored icon if it is available. This may be extended or removed depending on scope of the project in the future.
            useDefaultIcon=false
            appIcon="${appID}.icns"
            iconPath="${resourcePath}/Icons/${appIcon}"
        elif ! [[ $appID == "" ]]; then
            # ...Use the original app's icon
            appIcon=$(defaults read "${appInfo}" CFBundleIconFile 2> /dev/null)
            iconPath="${bundlePath}/Contents/Resources/${appIcon}"
        else
            # Well... that happened.
            echo
            echo "An unknown error has occured..."
            exitScript
        fi
    fi

    # Use icon if present or remove icon key
    if ! [[ $appIcon == "" ]]; then
        defaults write "${propertyFile}" CFBundleIconFile "${appIcon}"
    else
        defaults write "${propertyFile}" CFBundleIconFile ""
    fi

    # Bundle Signature
    appSignature=$(defaults read "${appInfo}" CFBundleSignature 2> /dev/null)
    if [[ $appSignature == "" ]]; then
        appSignature="????"
    fi
    defaults write "${propertyFile}" CFBundleSignature "${appSignature}"

    # Bundle Copyright
    appCopyright=$(defaults read "${appInfo}" NSHumanReadableCopyright 2> /dev/null)
    if ! [[ $appCopyright == "" ]]; then
        defaults write "${propertyFile}" NSHumanReadableCopyright "© ${appCopyright//\\+([0-9]) /}"
    else
        defaults delete "${propertyFile}" NSHumanReadableCopyright
    fi
    
    # Bundle Graphics Switching
    appGraphicsSwitching=$(defaults read "${appInfo}" NSSupportsAutomaticGraphicsSwitching 2> /dev/null)
    if ! [[ $appGraphicsSwitching == "" ]]; then
        case $appGraphicsSwitching in
            0)
                defaults write "${propertyFile}" NSSupportsAutomaticGraphicsSwitching -boolean FALSE
                ;;
            1)
                defaults write "${propertyFile}" NSSupportsAutomaticGraphicsSwitching -boolean TRUE
                ;;
        esac
    fi
    
    # Bundle Language
    appRegion=$(defaults read "${appInfo}" CFBundleDevelopmentRegion 2> /dev/null)
    if ! [[ $appRegion == "" ]]; then
        defaults write "${propertyFile}" CFBundleDevelopmentRegion "${appRegion}"
    fi
    
    # Bundle Retina Capable
    appRetina=$(defaults read "${appInfo}" NSHighResolutionCapable 2> /dev/null)
    if ! [[ $appRetina == "" ]]; then
        case $appRetina in
            0)
                defaults write "${propertyFile}" NSHighResolutionCapable -boolean FALSE
                ;;
            1)
                defaults write "${propertyFile}" NSHighResolutionCapable -boolean TRUE
                ;;
        esac
    fi
    
    # Target macOS
    appTarget=$(defaults read "${appInfo}" LSMinimumSystemVersion 2> /dev/null)
    if ! [[ $appTarget == "" ]]; then
        defaults write "${propertyFile}" LSMinimumSystemVersion "${appTarget}"
    fi

    # Ensure PLIST is XML format
    plutil -convert xml1 "${propertyFile}"

    echo
    echo "Generating bundle..."
    local moddedPath="${userAppsPath}/${appName} ${appNameSuffix}.app"
    local contentPath="${moddedPath}/Contents"

    # Handle existing launcher bundle
    if [[ -d "${moddedPath}" ]]; then
        askForOverwrite
        if [[ $overwriteBundle == false ]]; then
            return
        else
            rm -rf "${moddedPath}"
        fi
    fi

    # Create bundle structure
    mkdir -p "${contentPath}/MacOS"
    mkdir -p "${contentPath}/Resources"

    echo
    echo "Copying assets..."
    # Copy bundle assets
    ln -s "${bundlePath}" "${contentPath}/MacOS/"
    cp "${propertyFile}" "${contentPath}/"
    cp "${liblovelyPath}" "${contentPath}/MacOS/"
    if ! [[ $iconPath == "" ]]; then
        cp "${iconPath}" "${contentPath}/Resources/"
    fi

    echo
    echo "Building script..."
    # Use echo to write out script contents
    echo '#! /bin/bash' > "${contentPath}/MacOS/run_lovely"
    echo 'workingpath=$(cd "$(dirname "$0")"; pwd)' >> "${contentPath}/MacOS/run_lovely"
    echo 'export DYLD_INSERT_LIBRARIES=liblovely.dylib' >> "${contentPath}/MacOS/run_lovely"
    echo 'cd "${workingpath}"' >> "${contentPath}/MacOS/run_lovely"
    echo "./${appBinary} "'"$@"' >> "${contentPath}/MacOS/run_lovely"

    # Ensure script is owned by user and executable
    chown -R -P $USER "${contentPath}" > /dev/null
    chmod +x "${contentPath}/MacOS/run_lovely"

    echo
    echo "Signing application..."
    # Return .app extension to bundle before signing
    # mv -f "${moddedPath//\.app/}" "${moddedPath}"
    codesign -s - --deep "${moddedPath}/Contents/MacOS/run_lovely"

    # Remove duplicate property file
    cp -f "${propertyFile}.original" "${propertyFile}"
    rm -f "${propertyFile}.original"

    echo
    read -n 1 -p "Installation complete! Press any key to return to menu..." -s key
    open "${userAppsPath}"
    clearVariables
    return
}

# Get ICNS File Loop
askForIcon() {
    echo
    read -p "Use custom icon file? Please say [Y] for Yes, or [N] for No." -n 1 -s key
    case $key in
        [Yy])
            # Do Nothing and continue
            resume
            ;;
        [Nn])
            # Early return
            echo
            return 0
            ;;
        *)
            # Loop back and ask again
            echo
            echo "Invalid input received."
            askForIcon
            ;;
    esac

    echo
    echo "Please drag-and-drop the icon file to be used, or enter its location below, then press enter."
    echo
    read -p "Path to ICNS: " iconPath
    if [[ -f "${iconPath}" ]] && [[ $iconPath == *.[Ii][Cc][Nn][Ss] ]]; then
        appIcon="$(basename $iconPath)"
        useCustomIcon=true
        return
    else
        echo
        echo "Invalid input received. Please make sure to input the path to a valid macOS Icon Format file (.icns)..."
        echo
        echo "Continuing with standard icon..."
        return
    fi
}

# Bundle Overwrite Message Loop
askForOverwrite() {
    echo
    read -p "Modded bundle already found. Overwrite? (Y/N)" -n 1 -s overwrite
    case $overwrite in
        [Yy])
            overwriteBundle=true
            echo
            return
            ;;
        [Nn])
            echo
            echo "Returning to menu..."
            return
            ;;
        *)
            echo "Invalid option. Please press [Y] for Yes, or [N] for No."
            askForOverwrite
            ;;
    esac
}

# Script exit command
exitScript() {
    echo
    read -p "Press any key to exit..." -n 1 -s
    exit
}

resume() {
    # Do nothing command where needed; Used mostly as placeholder or for debugging.
    echo > /dev/null
}

# Set current path to the working directory of this script
cd "${workingPath}"

# Erases dynamic variables for next pass
clearVariables() {
    appInfo=""
    appName=""
    appID=""
    appIcon=""
    appSignature=""
    appBinary=""
    appCopyright=""
    appGraphicsSwitching=""
    appRegion=""
    appRetina=""
    appTarget=""

    bundlePath=""
    binaryPath=""
    iconPath=""

    useCustomIcon=false
    useDefaultIcon=true
    overwriteBundle=false

    binaryName=""
}

# Main loop
while [ true ]; do
    echo
    echo "╔═════════════════════════════════════════════════════╗"
    echo "║ ♡ Lovely Injector by Ethan Green ♡                  ║"
    echo "╠═════════════════════════════════════════════════════╣"
    echo "║                                                     ║"
    echo "║ MAIN MENU : Please select an option...              ║"
    echo "║                                                     ║"
    echo "╠═════════════════════════════════════════════════════╣"
    echo "║                                                     ║"
    echo "║ 1) Inject Lovely into application only & run once   ║"
    echo "║                                                     ║"
    echo "║ 2) Build modded application bundle for repeated use ║"
    echo "║                                                     ║"
    echo "║ 3) Exit                                             ║"
    echo "║                                                     ║"
    echo "╚═════════════════════════════════════════════════════╝"
    echo

    # Menu Input Loop
    menuOption
done