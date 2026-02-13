#!/bin/bash
echo "Iniciando desinstalación segura de klipBored..."

pkill -9 klipBored

SCHEMA="org.gnome.settings-daemon.plugins.media-keys.custom-keybinding"
PATH_CUSTOM="/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/custom0/"
NAME=$(gsettings get "$SCHEMA:$PATH_CUSTOM" name)

if [[ $NAME == *"'klipBored'"* ]]; then
    echo "   -> Detectado atajo de klipBored en custom0. Eliminando..."
    gsettings set "$SCHEMA:$PATH_CUSTOM" binding ""
    gsettings set "$SCHEMA:$PATH_CUSTOM" command ""
    gsettings set "$SCHEMA:$PATH_CUSTOM" name ""
    
else
    echo "   -> No se detectó configuración estándar de klipBored o ya fue modificada. No tocamos tus atajos."
fi

rm -rf ~/.config/klipBored
rm -f ~/.config/autostart/klipbored.desktop

if dpkg -l | grep -q klipbored; then
    sudo apt remove -y klipbored
fi

echo "Listo."