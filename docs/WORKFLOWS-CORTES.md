# Workflows do fork: o que foi cortado e por quê

Escopo do fork `rech-rustdesk`: distribuir apenas **Windows, Linux e macOS**. Contexto de sync e tags em [AUTOMACAO-SYNC-RELEASE.md](AUTOMACAO-SYNC-RELEASE.md).

## 1. Workflows mantidos

| Arquivo | Função |
|---|---|
| `sync-upstream.yml` | Sync noturno do upstream para `upstream-tracking` e PR para `master`. |
| `tag-release.yml` | Cria a tag `X.Y.Z-rech.N` quando o PR de sync é mesclado. |
| `flutter-tag.yml` | Dispara build e Release a partir da tag. |
| `flutter-build.yml` | Build em si (chamado por `flutter-tag.yml` e `flutter-ci.yml`). |
| `bridge.yml` | Gera a ponte Rust/Dart, usada pelo `flutter-build.yml`. |
| `third-party-RustDeskTempTopMostWindow.yml` | DLL auxiliar do Windows, usada pelo `flutter-build.yml`. |
| `flutter-ci.yml` | Validação de PRs e pushes no `master`. |
| `clear-cache.yml` | Limpeza manual do cache das Actions; não gasta CI. |

## 2. Workflows removidos

| Arquivo | Motivo |
|---|---|
| `fdroid.yml` | Gera arquivo de versão para a loja F-Droid (Android). Fora do escopo. |
| `flutter-nightly.yml` | Build completo toda noite na tag `nightly`; custo alto e as releases saem pelas tags. |
| `playground.yml` | Só manual; build de teste antigo do upstream (`VERSION` fixo). |
| `update-webpki-roots.yml` | Atualização semanal de certificados raiz via PR. O upstream já faz isso e chega pelo sync; no fork geraria PR duplicado mexendo no `Cargo.lock`. |
| `ci.yml` | Quase toda a matriz comentada e redundante com o `flutter-ci.yml`. |

Apagar é seguro: o sync resolve conflito modify/delete mantendo a exclusão (seção 4).

## 3. Jobs desativados no `flutter-build.yml`

Desativados com `if: false` em vez de apagar os blocos, para manter o diff mínimo e conflitos de sync raros.

| Job | Motivo |
|---|---|
| `build-rustdesk-android`, `build-rustdesk-android-universal` | Android fora do escopo. |
| `build-rustdesk-ios` | iOS fora do escopo. |
| `build-rustdesk-web` | Web fora do escopo (já estava `if: False` no upstream). |
| `build-for-windows-sciter`, `build-rustdesk-linux-sciter` | UI Sciter é legada; Windows 32 bits não é necessário. |
| `build-rustdesk-linux-drm` | Pacote para Linux sem display server (DRM/KMS); os alvos são desktops comuns. |

Ajustes necessários para os jobs restantes não quebrarem:

- `publish_unsigned`: removidos o `needs` do Sciter, o download do artefato `windows-x86` e o `windows-x86` do `tar`. Job dependente de job desativado é pulado, ou falha por artefato ausente.
- `publish-signed-update-metadata`: removido o `needs` do Sciter. Esse job só roda em `rustdesk/rustdesk`, então nunca executa no fork.
- `build-flatpak`: removidos o `needs` do Linux Sciter e a entrada `-sciter` da matriz.

Mantidos: Windows Flutter (x86_64, aarch64), macOS (x86_64, aarch64), Linux (deb, rpm, Arch), AppImage e flatpak.

Pendências conhecidas: variáveis de ambiente de Android/Sciter no topo do arquivo e a referência a `x86-sciter.exe` no job de metadata assinada seguem no arquivo.

## 4. Mudanças no `sync-upstream.yml`

Problema: o merge noturno conflitava com os workflows editados no fork. A `upstream-tracking` não tinha as edições do `master`, então o conflito só aparecia no PR, onde o script não atua.

Decisões:

1. **Base do merge:** o job faz checkout do `master`, recria `upstream-tracking` com `git checkout -B` e mescla o upstream por cima. O conflito passa a aparecer no merge do job.
2. **Conflito em `.github/workflows/`:** vale a versão do fork (`--ours`); se o fork apagou o arquivo, a exclusão é mantida (`git rm`). Conflito fora dessa pasta aborta o job.
3. **Push com `--force`:** aceitável porque a branch é só de quarentena, e só roda se o merge passou. Em conflito fora dos workflows, o job sai antes do push.
4. **Rastreio do que foi descartado:** `--ours` descarta as mudanças do upstream naquele arquivo. Os commits descartados (`git log HEAD..MERGE_HEAD -- arquivo`) vão para o corpo do PR.
5. **Workflows novos do upstream:** entram sem conflito e já ficam ativos. Os arquivos adicionados também são listados no corpo do PR.
6. **PR já aberto:** a descrição é atualizada a cada execução, para não ficar desatualizada.

Risco aceito: no `flutter-build.yml`, um conflito faz o arquivo inteiro ficar com a versão do fork. Se o upstream trocar algo importante junto (ex.: `FLUTTER_VERSION`, `VCPKG_COMMIT_ID`), isso precisa ser aplicado à mão. A lista no corpo do PR existe para isso.
