# 📋 klipBored

**klipBored** es un gestor de portapapeles moderno, ligero y minimalista diseñado específicamente para entornos Linux (GNOME/GTK4). Permite mantener un historial de tus textos e imágenes copiadas, accesible instantáneamente mediante un atajo de teclado personalizable.

![Icono de klipBored](assets/klipbored.svg)

## ✨ Características

- 🕒 **Historial Inteligente**: Guarda tus últimos clips (texto e imágenes).
- 🖼️ **Soporte de Imágenes**: Previsualiza y recupera imágenes directamente desde el historial.
- ⚡ **Acceso Instantáneo**: Configura un atajo de teclado (ej. `Super + V`) para abrir y cerrar el panel.
- ⚙️ **Ajustes Integrados**: Cambia el atajo o activa el auto-inicio directamente desde la app.
- 🌑 **Diseño Premium**: Interfaz oscura moderna basada en Libadwaita y GTK4.
- 🖱️ **Auto-ocultado**: El panel se oculta automáticamente al perder el foco para no interrumpir tu flujo de trabajo.

## 🚀 Instalación rápida

Si ya tienes instalado **Rust** y las librerías de desarrollo de **GTK4 / Libadwaita**, simplemente ejecuta:

```bash
chmod +x install.sh
./install.sh
```

### Requisitos del sistema
En Ubuntu/Debian, asegúrate de tener las dependencias necesarias:
```bash
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential
```
Y Rust (vía rustup):
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## 🛠️ Uso

1. **Primer inicio**: Abre la aplicación desde el menú de aplicaciones de tu sistema.
2. **Asistente**: Sigue el asistente para elegir tu atajo de teclado favorito.
3. **Uso diario**:
   - Pulsa tu **atajo** para abrir el historial.
   - Haz clic en el botón de **copiar** de cualquier elemento para volver a tenerlo en el portapapeles (la ventana se cerrará sola).
   - Usa los **Ajustes** (icono ⚙️) para cambiar el comportamiento del programa.
   - Pulsa `Esc` o haz clic fuera para cerrar el panel.

## 🧹 Desinstalación

Si deseas eliminar klipBored y limpiar toda su configuración:
```bash
chmod +x uninstall.sh
./uninstall.sh
```

---
*Desarrollado con ❤️ usando Rust, Relm4 y GTK4.*
