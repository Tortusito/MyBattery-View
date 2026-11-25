# MyBattery-View

Monitor de batería ultraligero en tiempo real para Windows que muestra el nivel de carga de tus dispositivos Bluetooth e inalámbricos (ratones, teclados, auriculares y mandos HID/2.4 GHz/Xbox) desde la bandeja del sistema y un mini preview flotante.

Construido con **Rust + Tauri v2** (~4–7 MB en disco, bajo consumo de RAM, sin Chromium).

## Descarga

Descarga el instalador o la versión portable para Windows desde [Releases](https://github.com/Tortusito/MyBattery-View/releases):

- **`MyBattery-View-Installer.exe`**: instalador ligero para Windows.
- **`MyBattery-View-Portable.exe`**: ejecutable único sin instalación.

## Características

- **Mini preview en la bandeja del sistema**: porcentaje en tiempo real de todos tus dispositivos conectados al instante.
- **Detección multi-protocolo**: Bluetooth Clásico (PnP), BLE (`0x180F`), Wireless HID 2.4 GHz estándar y mandos Xbox (`XInput`).
- **Personalización completa**: tema claro/oscuro/sistema, 5 paletas de acento (`Wyforce`, `Cyan`, `Emerald`, `Amber`, `Rose`), estilo de iconos (`Línea`, `Tintado`, `Grueso`) e icono editable por dispositivo.
- **Inicio automático con Windows** configurable desde Ajustes.

## Compilación en Windows

Requiere Rust (`rustup` / `cargo`) y `tauri-cli` v2:

```powershell
cargo install tauri-cli --version "^2" --locked
cargo tauri build
```

## Licencia

Distribuido bajo licencia [MIT](LICENSE).

---

Powered by [Wyforce](https://wyforce.com)
