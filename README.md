# RustDesk Voice

Fork personal de RustDesk con micrófono remoto integrado: envía el audio del micrófono de una tablet Android a un ordenador Linux durante una sesión RustDesk. Linux lo publica como una entrada virtual de PipeWire, disponible para aplicaciones como grabadoras, navegadores y Codex.

## Diferencias respecto a RustDesk original

- Añade controles de micrófono en las sesiones móviles: iniciar, silenciar, reanudar y detener.
- Android captura el micrófono en un servicio foreground con notificación, de modo que puede seguir funcionando al cambiar de aplicación o bloquear la pantalla.
- El audio se codifica con Opus y viaja en mensajes propios dentro de la conexión autenticada de RustDesk. No usa una llamada de voz y no se reproduce por los altavoces del PC.
- El receptor Linux crea `RustDeskRemoteMicrophone Virtual Microphone` mediante PipeWire.
- La preferencia opcional de inicio automático se guarda por equipo remoto en Android.
- Se excluye la captura simultánea de micrófono, llamada de voz y captura de audio de pantalla para evitar conflictos.

Ambos extremos deben ejecutar una compilación de este fork que incluya la función. Para compilar el receptor Linux se activa la feature `remote-microphone`; está desactivada por defecto.

## Dependencias y servicios

- **Linux:** PipeWire en ejecución; para compilar hacen falta además los encabezados de desarrollo de PipeWire y Clang/libclang. La versión instalada usa el servicio habitual de RustDesk y crea la fuente virtual bajo la sesión de usuario. Esta fuente requiere que el usuario tenga una sesión PipeWire activa.
- **Android:** Android 11 o posterior para la captura foreground del micrófono. Hay que conceder el permiso de grabación cuando se solicite.
- **OpenDroidMic:** no se instala como programa ni servicio aparte. Se adaptó únicamente la creación de la fuente PipeWire, conservando su atribución y licencia MIT en `src/remote_microphone_LICENSE`. Su transporte UDP y descubrimiento no se usan.
- **Tailscale:** no se instala, descarga ni configura. El audio se transporta por la conexión de RustDesk. No se requiere una cuenta ni un daemon de Tailscale.

## Instaladores personales

En [`installers/`](installers/) están el APK ARM64 de RustDesk Voice y el paquete Arch/Omarchy x86_64, junto con instrucciones, huellas SHA-256 y datos de compilación. El APK usa un certificado local de desarrollo y está pensado para uso personal; la aplicación Android puede coexistir con la oficial. El paquete Arch se llama `rustdesk-voice` y entra en conflicto con el paquete `rustdesk`, por lo que pacman solicitará la sustitución al instalarlo.

```sh
# Android: copia el APK a la tablet y ábrelo para instalarlo.

# Arch/Omarchy x86_64:
sudo pacman -U ./installers/rustdesk-voice-1.5.3-1-x86_64.pkg.tar.zst
```

Después conecta desde RustDesk Voice en Android al RustDesk Voice de Linux, activa el botón del micrófono y selecciona `RustDeskRemoteMicrophone Virtual Microphone` como entrada en la aplicación del ordenador. El inicio automático es opcional y se configura por cada equipo desde el menú del micrófono.

## Compilar

Linux, desde `rustdesk/` y con el Flutter/toolchain del proyecto:

```sh
cargo build --release --lib --features flutter,linux-pkg-config,remote-microphone
```

Android utiliza la captura y el canal de sesión ya existentes, así que no necesita la feature de receptor `remote-microphone`. Consulta [`rustdesk/docs/remote-microphone.md`](rustdesk/docs/remote-microphone.md) para el flujo detallado y los requisitos de compilación del fork.

## Repositorio y planificación

`rustdesk/` es el repositorio publicable de este fork. `opendroidmic/` y `tailscale/` fueron clones de investigación usados al explorar la solución; no forman parte del producto ni son necesarios para compilar o ejecutar esta función. Los artefactos compartidos de especificación están en `specs/` en el workspace de desarrollo y no son necesarios para usar el fork.
