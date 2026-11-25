# MyBattery-View

Monitor ultraligero en segundo plano para Windows (System Tray) que muestra en tiempo real el porcentaje de batería de tus dispositivos Bluetooth conectados.

## Descarga directa

Descarga el ejecutable listo para usar desde la sección [Releases](https://github.com/Tortusito/MyBattery-View/releases):

- **`MyBatteryView-light.exe`** (~1–2 MB): versión ultraligera (requiere [.NET 8 Desktop Runtime](https://dotnet.microsoft.com/en-us/download/dotnet/8.0)).
- **`MyBatteryView-portable.exe`**: ejecutable único sin dependencias externas (doble clic y listo).

## Características

- **Consumo mínimo**: diseñado en C# / .NET 8 nativo con WinForms + GDI+, sin WebViews ni procesos pesados.
- **Icono dinámico en la bandeja del sistema**: cambia color y nivel en tiempo real según el dispositivo con batería más baja.
- **Mini preview flotante (Flyout)**: al hacer clic izquierdo sobre el icono aparece una tarjeta compacta sobre la barra de tareas con cada dispositivo, su categoría y su porcentaje exacto.
- **Tooltip instantáneo**: al pasar el ratón por encima del icono muestra el resumen de porcentajes.
- **Lectura dual Bluetooth**:
  - **Bluetooth Clásico** vía API nativa PnP de Windows (`DEVPKEY_Bluetooth_Battery`): auriculares, mandos, teclados y ratones.
  - **BLE (Bluetooth Low Energy)** vía WinRT `Battery Service` (`0x180F` / `0x2A19`).
- **Iniciar con Windows**: activable/desactivable con un clic desde el menú contextual (registro de usuario `HKCU`, sin permisos de administrador).
- **Alertas de batería baja**: notificación nativa cuando un dispositivo baja del 20%.
- **Tema claro/oscuro automático**: sincronizado con la configuración de Windows 10/11.

## Uso

1. Ejecuta `MyBatteryView.exe`.
2. Verás el icono de batería en el área de notificación (desplegable de programas en segundo plano, abajo a la derecha).
3. **Clic izquierdo**: abre o cierra el mini preview con la lista de dispositivos.
4. **Clic derecho**: menú rápido (`Actualizar ahora`, `Iniciar con Windows`, `Avisar batería baja (<20%)`, `Salir`).

## Compilar desde código fuente

Requiere [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0) en Windows:

```powershell
# Versión ligera (~1–2 MB)
dotnet publish MyBatteryView.csproj -c Release -r win-x64 --no-self-contained -o publish/light

# Versión portable (sin dependencias)
dotnet publish MyBatteryView.csproj -c Release -r win-x64 --self-contained true -o publish/portable
```

## Licencia

Distribuido bajo la licencia [MIT](LICENSE).

---

Powered by [Wyforce](https://wyforce.com)
