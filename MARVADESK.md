# MarvaDesk – Personalización

Proyecto basado en RustDesk, personalizado como **MarvaDesk** (soporte remoto).

---

## Qué archivos genera cada comando

`build.py` **solo construye para el sistema donde lo ejecutas** (Windows → Windows, macOS → macOS, Linux → Linux). Android se construye por separado con Flutter.

| Comando | Plataforma (donde ejecutas) | Archivos / carpetas que genera |
|--------|-----------------------------|---------------------------------|
| `python3 build.py --flutter --quicksupport` | **Windows** | `flutter/build/windows/x64/runner/Release/` → `marvadesk.exe` + DLLs. Si no usas `--skip-portable-pack`: además `marvadesk-{version}-install.exe` (portable) en la raíz del repo. |
| `python3 build.py --flutter --quicksupport` | **macOS** | `flutter/build/macos/Build/Products/Release/RustDesk.app` (bundle de la app). |
| `python3 build.py --flutter --quicksupport` | **Linux** (Debian/Ubuntu) | `flutter/build/linux/x64/release/bundle/` y, al final, `rustdesk-{version}.deb` en la raíz (puedes renombrarlo a `marvadesk-{version}.deb`). |
| `python3 build.py --flutter` | Igual que arriba | Los **mismos** tipos de salida, pero es la **versión Agente** (con ajustes y conexiones salientes). |
| `cd flutter && flutter build apk --release` | Cualquiera (con Android SDK) | `flutter/build/app/outputs/flutter-apk/app-release.apk`. Con `--split-per-abi`: `app-arm64-v8a-release.apk`, `app-armeabi-v7a-release.apk`, etc. |

### Cliente vs Agente

- **Cliente QuickSupport:** `python3 build.py --flutter --quicksupport` → ejecutable/bundle/APK de **cliente** (solo ID y contraseña).
- **Agente:** `python3 build.py --flutter` (sin `--quicksupport`) → **agente** (misma marca, con ajustes y conexiones salientes).

---

## Clientes y agentes por sistema (Mac, Windows, Linux, Android)

| Sistema | Cliente QuickSupport | Agente |
|--------|----------------------|--------|
| **Windows** | En Windows: `python3 build.py --flutter --quicksupport` | En Windows: `python3 build.py --flutter` |
| **macOS** | En Mac: `python3 build.py --flutter --quicksupport` | En Mac: `python3 build.py --flutter` |
| **Linux** | En Linux: `python3 build.py --flutter --quicksupport` | En Linux: `python3 build.py --flutter` |
| **Android** | `cd flutter && flutter build apk --release` (un APK; para dos APKs cliente/agente haría falta configurar flavors) | Mismo comando |

Para tener **todas** las variantes necesitas ejecutar `build.py` en cada sistema (Windows, Mac, Linux) una vez con `--quicksupport` (cliente) y otra sin (agente). Android se hace con Flutter desde cualquier máquina.

---

## Builds

### Cliente QuickSupport (solo recibir soporte)

- Solo muestra **ID y contraseña** para aceptar conexiones.
- Sin menú de ajustes ni conexiones salientes (estilo TeamViewer QuickSupport).

```bash
python3 build.py --flutter --quicksupport
```

El binario será `marvadesk` (o `marvadesk.exe` en Windows). En Android:

```bash
cd flutter && flutter build apk --release
```

### Agente de escritorio (soporte + conexiones salientes)

- Misma marca (MarvaDesk, colores, servidor fijo).
- **Sí** permite iniciar conexiones salientes y ajustes.

```bash
python3 build.py --flutter
```

Sin el flag `--quicksupport` se construye la versión agente.

---

## Firma con certificado de desarrollador (para el futuro)

Cuando tengas certificados, puedes integrar la firma así:

### Windows (Authenticode)

- Necesitas: certificado de firma de código (`.pfx`) de una CA (DigiCert, Sectigo, etc.).
- El script ya contempla firma si existe la variable de entorno `P` (contraseña del certificado) y el archivo `cert.pfx` en la raíz del proyecto. En `build.py` busca `signtool sign` y `cert.pfx`.
- Sin certificado: no definas `P`; el script imprimirá "Not signed".

### macOS (Developer ID + notarización)

- Necesitas: cuenta **Apple Developer** y certificado **Developer ID Application**.
- En Xcode: Cuenta → Certificates → crear "Developer ID Application"; exportar a `.p12` si firmas por línea de comandos.
- Variable de entorno `P` = nombre del certificado (ej. `Developer ID Application: Tu Nombre (TEAMID)`). El script en macOS ya tiene ejemplos con `codesign` y `rcodesign notary-submit` para notarización (API Key en App Store Connect).

### Android

- Necesitas: keystore (`.jks` o `.keystore`) para release.
- Crear: `keytool -genkey -v -keystore marvadesk-release.keystore -alias marvadesk -keyalg RSA -keysize 2048 -validity 10000`
- En `flutter/key.properties`: `storePassword`, `keyPassword`, `keyAlias`, `storeFile=ruta/al/keystore`. El `build.gradle` de `flutter/android/app` ya lee `key.properties` para `release`; el APK quedará firmado.

### Linux

- No hay firma de código típica para .deb. Opcional: firma **GPG** del repositorio para verificación de paquetes.

---

## Servidor y clave fijos

- **Servidor ID por defecto:** `libs/hbb_common/src/config.rs` → `RENDEZVOUS_SERVERS` (p. ej. `id.marvadesk.com`).
- **Clave pública:** mismo archivo → `RS_PUB_KEY`.
- El usuario **no puede cambiar** servidor ni clave en la UI (opciones ocultas y fijas por código).

Sustituye `id.marvadesk.com` y `RS_PUB_KEY` por tu servidor ID y tu clave pública.

## Logos e iconos

Sustituye los assets en `res/`:

- `icon.png`, `icon.ico` – escritorio
- `mac-icon.png` – macOS
- `logo.svg`, `rustdesk-banner.svg` (o equivalente) – si los usas

Luego regenera iconos de Flutter:

```bash
cd flutter && flutter pub run flutter_launcher_icons
```

## Enlaces y documentación

Enlaces por defecto apuntan a `https://marvadesk.com/` (docs, privacidad, descargas).  
Cámbialos en:

- `libs/hbb_common/src/config.rs` → `LINK_DOCS_*`, `HELPER_URL`
- Flutter: búsqueda de `marvadesk.com` en `flutter/lib/`

## Linux

- `.desktop`: `res/marvadesk.desktop`
- Servicio: `res/marvadesk.service`

Usa estos archivos para empaquetado (deb/rpm) en lugar de `rustdesk.desktop` / `rustdesk.service`.

## Colores

Tema en `flutter/lib/common.dart` → clase `MyTheme` (accent, button, idColor en tonos teal/verde azulado). Ajusta ahí para cambiar la imagen de marca.
