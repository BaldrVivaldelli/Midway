# Distribución seria para Midway

Este paquete deja preparada una cadena de release más madura en **GitHub Releases** con:

- instaladores por plataforma
- firma / notarización donde corresponde
- `latest.json` para auto-update estable
- `latest-beta.json` para canal beta
- `SHA256SUMS.txt`
- release notes categorizadas
- artifact attestations de GitHub Actions
- updater integrado en runtime para chequear / descargar / reinstalar desde la UI

## Qué incluye

- `.github/workflows/release.yml`
- `.github/release.yml`
- `src-tauri/tauri.release.conf.json`
- `src-tauri/tauri.beta.conf.json`
- `src-tauri/tauri.windows.conf.json`
- `src-tauri/tauri.macos.conf.json`
- `src-tauri/tauri.linux.conf.json`
- `src-tauri/src/lib.rs` con `tauri-plugin-updater` y `tauri-plugin-process`
- `src-tauri/capabilities/default.json` con `updater:default` y `process:default`
- `scripts/release/render-tauri-config.mjs`
- `scripts/release/generate-checksums.mjs`
- `scripts/release/generate-updater-json.mjs`
- `src/components/UpdateCenterCard.tsx`
- `src/lib/updater.ts`

## Estrategia recomendada

### Canal stable

Tags como:

```text
v0.2.0
v0.2.1
v1.0.0
```

Publican:

- release draft con assets
- `latest.json`
- `SHA256SUMS.txt`

### Canal beta

Tags como:

```text
v0.2.0-beta.1
v0.2.0-rc.1
```

Publican:

- release draft marcado como prerelease
- `latest-beta.json`
- `SHA256SUMS.txt`

## Variables y secrets de GitHub

### Secrets obligatorios para updater firmado

- `TAURI_SIGNING_PRIVATE_KEY`
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (si corresponde)
- `TAURI_UPDATER_PUBLIC_KEY`

### Variables de release que conviene definir en GitHub

- `MIDWAY_APP_IDENTIFIER`
- `MIDWAY_PRODUCT_NAME`
- `MIDWAY_BETA_PRODUCT_NAME`
- `MIDWAY_PUBLISHER`
- `MIDWAY_HOMEPAGE`

El workflow inyecta esas variables en `scripts/release/render-tauri-config.mjs`, que genera en runtime:

- `src-tauri/tauri.release.generated.json`
- `src-tauri/tauri.beta.generated.json`

Así evitás hardcodear `OWNER/REPO`, pubkeys y el bundle identifier final en el repo fuente.

### macOS signing / notarization

- `APPLE_CERTIFICATE`
- `APPLE_CERTIFICATE_PASSWORD`
- `KEYCHAIN_PASSWORD`
- y **uno** de estos dos esquemas:
  - Apple ID: `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID`
  - App Store Connect API: `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_P8`

### Windows

El workflow deja listo el carril para firmar Windows, pero necesitás completar la estrategia real que vayas a usar:

- **EV / OV local** con `signtool`
- o **Azure Code Signing** / **Azure Key Vault**

Para eso completá `src-tauri/tauri.windows.conf.json` con tu configuración definitiva de firma.

## Paso previo único: generar la clave del updater

En tu máquina, con Tauri CLI disponible:

```bash
npm run tauri signer generate -- -w ~/.tauri/midway.key
```

Guardá:

- la **private key** en `TAURI_SIGNING_PRIVATE_KEY`
- la passphrase en `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` si aplica
- la **public key** en `TAURI_UPDATER_PUBLIC_KEY`

## Qué hace el workflow de release

1. corre quality gate (`build`, tests y budget)
2. corre `cargo check`
3. genera overlays de config para stable / beta
4. build matrix en Linux / Windows / macOS Intel / macOS Apple Silicon
5. sube instaladores a un **draft release**
6. genera **artifact attestations**
7. descarga bundles locales
8. genera `latest.json` o `latest-beta.json`
9. genera `SHA256SUMS.txt`
10. adjunta metadatos al mismo release draft

## Cómo se usa el updater en la app

Midway ya incluye un panel **App updates** dentro de **Workspace**:

- muestra versión actual y canal
- permite check manual
- descarga e instala updates
- ofrece reiniciar cuando la instalación termina

Notas prácticas:

- en desarrollo (`vite` / `tauri dev`) el auto-check queda desactivado para no ensuciar la UX
- en la build empaquetada el updater usa el canal resuelto por `VITE_MIDWAY_UPDATE_CHANNEL`
- si el updater no está soportado o la configuración no existe, la UI muestra el error en vez de romper la app

## Validación recomendada antes de publicar

### Stable

- instalar una build estable anterior
- publicar una release draft estable nueva
- validar update `stable -> stable`
- confirmar firma / SmartScreen / Gatekeeper

### Beta

- instalar una beta anterior
- publicar un draft beta nuevo
- validar update `beta -> beta`
- confirmar que no se mezclen manifests entre `latest.json` y `latest-beta.json`

## Checklist antes de publicar el draft

- que los assets estén todos
- que `latest.json` o `latest-beta.json` tenga URLs correctas
- que `SHA256SUMS.txt` se haya adjuntado
- que la release note automática tenga categorías razonables
- que macOS quede firmado / notarizado
- que Windows quede firmado
- que el panel de update dentro de la app encuentre la nueva versión

## Primer release recomendado

Yo haría este orden:

1. `v0.2.0-beta.1`
2. instalar en máquinas limpias
3. validar update beta -> beta
4. corregir problemas de firma / notarización
5. `v0.2.0`
6. validar update stable desde una build instalada previamente
