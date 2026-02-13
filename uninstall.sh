#!/bin/bash
echo "Iniciando desinstalación segura de klipBored..."

pkill -9 klipBored

SCHEMA_MAIN="org.gnome.settings-daemon.plugins.media-keys"
SCHEMA_CUSTOM="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding"

# Get the list of custom keybindings
LIST=$(gsettings get "$SCHEMA_MAIN" "custom-keybindings")

# Loop through the list to find the one named 'klipBored'
NEW_LIST="["
FIRST=true
FOUND_PATH=""

# Clean the list string and split by comma
CLEAN_LIST=${LIST#[}
CLEAN_LIST=${CLEAN_LIST%]}
IFS=',' read -ra ADDR <<< "$CLEAN_LIST"

for i in "${ADDR[@]}"; do
    PATH_RAW=$(echo "$i" | xargs) # trim whitespace
    PATH_CLEAN=${PATH_RAW#\'}
    PATH_CLEAN=${PATH_CLEAN%\'}
    
    if [ -n "$PATH_CLEAN" ]; then
        NAME=$(gsettings get "$SCHEMA_CUSTOM:$PATH_CLEAN" name 2>/dev/null)
        if [[ $NAME == *"'klipBored'"* ]]; then
            echo "   -> Detectado atajo de klipBored en $PATH_CLEAN. Marcando para eliminar..."
            FOUND_PATH="$PATH_CLEAN"
        else
            if [ "$FIRST" = true ]; then
                NEW_LIST+="'$PATH_CLEAN'"
                FIRST=false
            else
                NEW_LIST+=", '$PATH_CLEAN'"
            fi
        fi
    fi
done
NEW_LIST+="]"

if [ -n "$FOUND_PATH" ]; then
    echo "   -> Actualizando lista de atajos y limpiando configuración..."
    gsettings set "$SCHEMA_MAIN" "custom-keybindings" "$NEW_LIST"
    # Also clear the specific entry
    gsettings set "$SCHEMA_CUSTOM:$FOUND_PATH" name ""
    gsettings set "$SCHEMA_CUSTOM:$FOUND_PATH" command ""
    gsettings set "$SCHEMA_CUSTOM:$FOUND_PATH" binding ""
    
    # Restaurar atajo por defecto de Ubuntu (Win+V para calendario)
    echo "   -> Restaurando atajo por defecto de Ubuntu (Win+V)..."
    gsettings set org.gnome.shell.keybindings message-list-toggle "['<Super>v']"
else
    echo "   -> No se detectó configuración de klipBored en tus atajos. No tocamos nada."
fi

# Limpieza de iconos de usuario (si existen)
rm -f ~/.local/share/icons/hicolor/scalable/apps/io.github.klipbored.app.svg
rm -f ~/.local/share/icons/hicolor/scalable/apps/klipbored.svg
rm -f ~/.local/share/icons/hicolor/128x128/apps/io.github.klipbored.app.png
rm -f ~/.local/share/icons/hicolor/48x48/apps/io.github.klipbored.app.png

# Limpieza de archivos de escritorio del usuario
rm -f ~/.local/share/applications/io.github.klipbored.app.desktop
rm -f ~/.config/autostart/io.github.klipbored.app.desktop

# Borrado de historial y configuración personal
rm -rf ~/.config/klipBored

# Actualización de base de datos de iconos para refrescar el dock
if command -v gtk4-update-icon-cache &> /dev/null; then
    touch ~/.local/share/icons/hicolor
    gtk4-update-icon-cache -f -t ~/.local/share/icons/hicolor &> /dev/null
fi

if dpkg -l | grep -q klipbored; then
    sudo apt remove -y klipbored
fi

echo "✅ Desinstalación completa. Rastro eliminado al 100%."