# MyBattery-View

Monitor de batería en tiempo real para Windows que muestra el nivel de carga de tus dispositivos Bluetooth e inalámbricos (ratones, teclados, auriculares y mandos HID/2.4 GHz) desde la bandeja del sistema y un mini preview flotante.

## Descarga

Descarga el instalador para Windows desde la sección [Releases](https://github.com/Tortusito/MyBattery-View/releases).

## Características

- **Mini preview en la bandeja del sistema**: porcentaje en tiempo real de todos tus dispositivos conectados al instante.
- **Compatibilidad amplia**: Bluetooth, BLE y periféricos inalámbricos HID / 2.4 GHz (Logitech, Razer, Corsair, SteelSeries, mandos, auriculares, etc.).
- **Alertas de batería baja** e historial de descarga.
- **Inicio automático con Windows** configurable desde la aplicación.
- **Estética Wyforce** con soporte automático de tema oscuro/claro.

## Desarrollo y compilación

Requiere Node.js 22+ en Windows:

```bash
npm install
npm start
```

Para generar el instalador en Windows:

```bash
npm run dist
npm run installer
```

## Licencia

Distribuido bajo licencia [MIT](LICENSE).

---

Powered by [Wyforce](https://wyforce.com)
