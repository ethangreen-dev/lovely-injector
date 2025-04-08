#!/bin/bash

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"

echo "====== Lovely Injector macOS Setup ======"

if [ -f "$SCRIPT_DIR/liblovely.dylib" ]; then
    echo "Removing quarantine with sudo (you may be asked for your password)"
    sudo xattr -d com.apple.quarantine "$SCRIPT_DIR/liblovely.dylib"
    if [ $? -ne 0 ]; then
        echo "✗ Failed to remove quarantine attribute"
    else
        echo "✓ Quarantine attribute removed successfully"
    fi
else
    echo "✗ Could not find liblovely.dylib in the current directory"
    exit 1
fi

echo -e "\nWould you like to set up a shell alias to launch the game from anywhere? (y/n)"
read -r SETUP_ALIAS

if [[ $SETUP_ALIAS =~ ^[Yy]$ ]]; then
    # Determine user's shell
    USER_SHELL=$(basename "$SHELL")
    echo "Detected shell: $USER_SHELL"

    ALIAS_COMMAND="alias balatro=\"$SCRIPT_DIR/run_lovely_macos.sh\""
    FISH_FUNCTION="function balatro\n    \"$SCRIPT_DIR/run_lovely_macos.sh\" \$argv\nend"

    case "$USER_SHELL" in
        "bash")
            CONFIG_FILE="$HOME/.bashrc"
            echo -e "\n# Balatro launcher shortcut\n$ALIAS_COMMAND" >> "$CONFIG_FILE"
            echo "✓ Added alias to $CONFIG_FILE"
            echo "Run source $CONFIG_FILE to apply changes immediately"
            ;;
        "zsh")
            CONFIG_FILE="$HOME/.zshrc"
            echo -e "\n# Balatro launcher shortcut\n$ALIAS_COMMAND" >> "$CONFIG_FILE"
            echo "✓ Added alias to $CONFIG_FILE"
            echo "Run source $CONFIG_FILE to apply changes immediately"
            ;;
        "fish")
            CONFIG_DIR="$HOME/.config/fish"
            CONFIG_FILE="$CONFIG_DIR/config.fish"

            # Create config directory if it doesn't exist
            if [ ! -d "$CONFIG_DIR" ]; then
                mkdir -p "$CONFIG_DIR"
            fi

            echo -e "\n# Balatro launcher shortcut\n$FISH_FUNCTION" >> "$CONFIG_FILE"
            echo "✓ Added function to $CONFIG_FILE"
            echo "Run source $CONFIG_FILE to apply changes immediately"
            ;;
        *)
            echo "⚠ Unsupported shell: $USER_SHELL"
            echo "To create an alias manually, add the following to your shell's config file:"
            echo "alias balatro=\"$SCRIPT_DIR/run_lovely_macos.sh\""
            ;;
    esac

    echo -e "\nSetup complete!"
    echo "You can now launch the game by typing balatro in your terminal"
    echo "(after sourcing your shell config or starting a new terminal session)"
else
    echo -e "\nSetup complete!"
    echo "You can launch the game by running $SCRIPT_DIR/run_lovely_macos.sh"
fi
