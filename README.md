<p align="center">
  <img src="res/logo-header.svg" alt="CosmoDesk" width="360"><br>
  <b>CosmoDesk</b> — acesso remoto de desktop, com suporte a máquinas da sua rede Tailscale.
</p>

<p align="center">
  <a href="#sobre">Sobre</a> •
  <a href="#recursos-do-fork">Recursos do fork</a> •
  <a href="#compilando">Compilando</a> •
  <a href="#estrutura-do-projeto">Estrutura</a> •
  <a href="#licença-e-créditos">Licença</a>
</p>

> [!CAUTION]
> **Aviso de uso indevido:** os desenvolvedores não apoiam nem toleram o uso antiético ou ilegal deste software. Acesso não autorizado, controle remoto sem consentimento e invasão de privacidade vão contra as diretrizes do projeto. Os autores não se responsabilizam por qualquer uso indevido.

## Sobre

O CosmoDesk é um fork do [RustDesk](https://github.com/rustdesk/rustdesk) (versão base 1.5.1), um software de acesso remoto escrito em Rust com interface em Flutter. Ele funciona sem configuração e permite usar o servidor rendezvous/relay público, [hospedar o seu próprio](https://rustdesk.com/server) ou conectar direto por IP.

> [!NOTE]
> O renomeio completo para CosmoDesk ainda está em andamento. Nomes de pacote, identificadores e alguns textos da interface ainda dizem RustDesk.

## Recursos do fork

### Integração com Tailscale

As máquinas **online** da sua tailnet aparecem na aba de peers descobertos, junto com as da rede local. Cada máquina é identificada pelo endereço `100.x` do Tailscale, então a conexão é direta por IP, sem depender de servidor rendezvous.

- Usa a CLI do Tailscale: `tailscale status --json`.
- Se o Tailscale não estiver instalado, parado ou deslogado, a lista fica vazia e nada quebra.
- No macOS procura o CLI no `PATH`, em `/usr/local/bin/tailscale` e dentro do `Tailscale.app`.
- Android e iOS não têm CLI, então nesses sistemas a lista Tailscale não é carregada. Eles ainda podem ser alvo de conexão a partir de um desktop.
- A saída `--json` é documentada pelo Tailscale como sujeita a mudança, por isso o parser lê poucos campos e todos são opcionais. Código em [`src/tailscale.rs`](src/tailscale.rs).

Para conectar, a outra máquina precisa estar com o CosmoDesk (ou RustDesk) em execução.

## Compilando

O guia oficial de build do projeto base está em <https://rustdesk.com/docs/en/dev/build/>. Abaixo está o caminho validado para o desktop Flutter no Linux.

### Requisitos

- Rust (`rust-version` do `Cargo.toml`: 1.75 ou mais novo) via [rustup](https://rustup.rs)
- Flutter 3.24.5
- [vcpkg](https://github.com/microsoft/vcpkg) com `libvpx`, `libyuv`, `opus` e `aom`
- Ferramentas de build C/C++: `clang`, `cmake`, `ninja`, `nasm`, `pkg-config`, GTK 3

Pacotes de sistema por distribuição:

<details>
<summary>Ubuntu / Debian</summary>

```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev \
        libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make \
        libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev
```
</details>

<details>
<summary>Fedora / CentOS</summary>

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel
```

No Fedora a `libvpx` precisa ser recompilada com `-fPIC`:

```sh
cd vcpkg/buildtrees/libvpx/src
cd *
./configure
sed -i 's/CFLAGS+=-I/CFLAGS+=-fPIC -I/g' Makefile
sed -i 's/CXXFLAGS+=-I/CXXFLAGS+=-fPIC -I/g' Makefile
make
cp libvpx.a $HOME/vcpkg/installed/x64-linux/lib/
```
</details>

<details>
<summary>Arch / Manjaro</summary>

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```
</details>

<details>
<summary>openSUSE Tumbleweed</summary>

```sh
sudo zypper install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libXfixes-devel cmake alsa-lib-devel gstreamer-devel gstreamer-plugins-base-devel xdotool-devel
```
</details>

### 1. Clonar com o submódulo

```sh
git clone --recurse-submodules git@github.com:devhttps/CosmosDesk.git
cd CosmosDesk
```

Se já clonou sem `--recurse-submodules`, rode `git submodule update --init`. Sem o `libs/hbb_common` o projeto não compila.

### 2. Instalar o vcpkg e as bibliotecas nativas

```sh
git clone https://github.com/microsoft/vcpkg ~/vcpkg
~/vcpkg/bootstrap-vcpkg.sh -disableMetrics
~/vcpkg/vcpkg install libvpx libyuv opus aom
export VCPKG_ROOT=$HOME/vcpkg
```

### 3. Compilar a biblioteca Rust

```sh
export VCPKG_ROOT=$HOME/vcpkg
cargo build --features flutter --lib
```

> [!TIP]
> Com GCC recente, o `webm-sys` pode falhar com `'uint64_t' does not name a type`. Defina `CXXFLAGS="-include cstdint"` antes do `cargo`.

### 4. Gerar a ponte Rust ↔ Dart

Os arquivos `src/bridge_generated.rs` e `flutter/lib/generated_bridge.dart` não ficam no repositório (estão no `.gitignore`). Gere-os localmente:

```sh
cargo install cargo-expand --version 1.0.95 --locked
cargo install flutter_rust_bridge_codegen --version 1.80.1 --features uuid --locked
(cd flutter && flutter pub get)
flutter_rust_bridge_codegen --rust-input ./src/flutter_ffi.rs \
  --dart-output ./flutter/lib/generated_bridge.dart \
  --c-output ./flutter/macos/Runner/bridge_generated.h
```

### 5. Compilar e rodar o app

```sh
cd flutter
flutter build linux --debug
./build/linux/x64/debug/bundle/rustdesk
```

### Testes

```sh
cargo test --lib tailscale
```

### Compilando com Docker

```sh
git submodule update --init --recursive
docker build -t "cosmodesk-builder" .
docker run --rm -it -v $PWD:/home/user/rustdesk \
  -v cosmodesk-git-cache:/home/user/.cargo/git \
  -v cosmodesk-registry-cache:/home/user/.cargo/registry \
  -e PUID="$(id -u)" -e PGID="$(id -g)" cosmodesk-builder
```

Argumentos extras do `cargo` vão no final do comando (por exemplo `--release`). O executável fica em `target/debug/` ou `target/release/`. Rode os comandos da raiz do repositório. Subcomandos como `cargo install` e `cargo run` não funcionam por esse método, pois executariam dentro do container.

### Interface Sciter (obsoleta)

A interface antiga em Sciter ainda existe em `src/ui/`, mas está obsoleta. Para usá-la, baixe a biblioteca [`libsciter-gtk.so`](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) para `target/debug/` e rode `cargo run`.

## Estrutura do projeto

| Caminho | Função |
|---|---|
| [`src/`](src) | Aplicativo em Rust |
| [`src/server/`](src/server) | Serviços de áudio, clipboard, input, vídeo e conexões de rede |
| [`src/client.rs`](src/client.rs) | Início da conexão com um peer |
| [`src/rendezvous_mediator.rs`](src/rendezvous_mediator.rs) | Comunicação com o [rustdesk-server](https://github.com/rustdesk/rustdesk-server) (conexão direta ou por relay) |
| [`src/lan.rs`](src/lan.rs) | Descoberta de peers na rede local |
| [`src/tailscale.rs`](src/tailscale.rs) | Descoberta de máquinas da tailnet |
| [`src/platform/`](src/platform) | Código específico de cada sistema |
| [`src/ui/`](src/ui) | Interface Sciter (obsoleta) |
| [`flutter/`](flutter) | Interface atual, desktop e mobile |
| [`libs/hbb_common/`](libs/hbb_common) | Submódulo: codecs, config, wrappers TCP/UDP, compartilhado com o servidor |
| [`libs/base/`](libs/base) | Código só do cliente: protobuf, transferência de arquivos, teclado e plataforma |
| [`libs/scrap/`](libs/scrap) | Captura de tela |
| [`libs/enigo/`](libs/enigo) | Controle de teclado e mouse por plataforma |
| [`libs/clipboard/`](libs/clipboard) | Copiar e colar arquivos (Windows, Linux, macOS) |

Código novo exclusivo do cliente deve ir em `libs/base`, não no submódulo `hbb_common`. Convenções de código e de logging estão em [`AGENTS.md`](AGENTS.md).

## Contribuindo

Veja [`docs/CONTRIBUTING.md`](docs/CONTRIBUTING.md), [`docs/CODE_OF_CONDUCT.md`](docs/CODE_OF_CONDUCT.md) e [`docs/SECURITY.md`](docs/SECURITY.md).

## Licença e créditos

O CosmoDesk é distribuído sob a [GNU AGPL v3](LICENCE), a mesma licença do projeto original. Este trabalho é baseado no [RustDesk](https://github.com/rustdesk/rustdesk), de seus autores e colaboradores, que mantêm os direitos sobre o código original. Documentação e traduções do projeto base estão em [`docs/`](docs).
