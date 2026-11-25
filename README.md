# MyBattery-View

Aplicación ultraligera para Windows (System Tray) que muestra en tiempo real el porcentaje de batería de los dispositivos Bluetooth conectados (Bluetooth Clásico vía PnP + BLE Battery Service).

## Características
- **Icono dinámico** en el área de notificación (bandeja del sistema) según el nivel de batería.
- **Mini preview flotante (Flyout)** al hacer clic izquierdo con lista de dispositivos, barras de nivel y `%`.
- **Tooltip** rápido al pasar el cursor.
- **Iniciar con Windows** configurable desde el menú contextual (clic derecho).
- **Alertas de batería baja** (<20%) nativas de Windows.
- Tema claro/oscuro sincronizado con Windows 11.

## Compilar en Windows

Requiere [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0).

### Versión ligera (~1–2 MB, requiere .NET 8 Desktop Runtime en Windows)
```powershell
dotnet publish MyBatteryView.csproj -c Release -r win-x64 --no-self-contained -o publish/light
```

### Versión portable (sin dependencias en Windows)
```powershell
dotnet publish MyBatteryView.csproj -c Release -r win-x64 --self-contained true -o publish/portable
```
