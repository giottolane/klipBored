#!/bin/bash

# klipBored - Script de Instalación Automática
echo "🚀 Iniciando instalación de klipBored..."

# 1. Comprobar dependencias básicas
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Rust/Cargo no está instalado. Instálalo con: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# 2. Compilar en modo release
echo "📦 Compilando aplicación (esto puede tardar un poco la primera vez)..."
cargo build --release
if [ $? -ne 0 ]; then
    echo "❌ Error en la compilación."
    exit 1
fi

# 3. Crear directorios necesarios
echo "📁 Creando rutas de sistema..."
mkdir -p ~/.local/bin
mkdir -p ~/.local/share/applications
mkdir -p ~/.local/share/icons/hicolor/scalable/apps
mkdir -p ~/.config/klipBored

# 4. Instalar archivos
echo "💾 Copiando archivos a las rutas de usuario..."
cp target/release/klipBored ~/.local/bin/klipBored
cp io.github.klipbored.app.desktop ~/.local/share/applications/
cp assets/klipbored.svg ~/.local/share/icons/hicolor/scalable/apps/io.github.klipbored.app.svg

# 5. Actualizar bases de datos del sistema
echo "🔄 Refrescando bases de datos de iconos y aplicaciones..."
touch ~/.local/share/icons/hicolor
if command -v gtk4-update-icon-cache &> /dev/null; then
    gtk4-update-icon-cache -f -t ~/.local/share/icons/hicolor &> /dev/null
fi
update-desktop-database ~/.local/share/applications &> /dev/null

echo ""
echo "✅ ¡Instalación completada con éxito!"
echo "-------------------------------------------------------"
echo "Puedes abrir 'klipBored' desde tu lanzador de aplicaciones."
echo "La primera vez se abrirá un asistente para configurar tu atajo."
echo "-------------------------------------------------------"
