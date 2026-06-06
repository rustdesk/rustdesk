<p align="center">
  <img src="../res/logo-header.svg" alt="RustDesk - Seu desktop remoto"><br>
  <a href="#compilar">Compilar</a> •
  <a href="#como-compilar-com-o-docker">Docker</a> •
  <a href="#estrutura-de-arquivos">Estrutura</a> •
  <a href="#capturas-de-tela">Capturas de Tela</a><br>
  [<a href="../README.md">Inglês</a>] | [<a href="docs/README-UA.md">Ucraniano</a>] | [<a href="docs/README-CS.md">Tcheco</a>] | [<a href="docs/README-ZH.md">Chinês</a>] | [<a href="docs/README-HU.md">Húngaro</a>] | [<a href="docs/README-ES.md">Espanhol</a>] | [<a href="docs/README-FA.md">Persa</a>] | [<a href="docs/README-FR.md">Francês</a>] | [<a href="docs/README-DE.md">Alemão</a>] | [<a href="docs/README-PL.md">Polonês</a>] | [<a href="docs/README-ID.md">Indonésio</a>] | [<a href="docs/README-FI.md">Finlandês</a>] | [<a href="docs/README-ML.md">Malaiala</a>] | [<a href="docs/README-JP.md">Japonês</a>] | [<a href="docs/README-NL.md">Holandês</a>] | [<a href="docs/README-IT.md">Italiano</a>] | [<a href="docs/README-RU.md">Russo</a>] | [<a href="docs/README-EO.md">Esperanto</a>] | [<a href="docs/README-KR.md">Coreano</a>] | [<a href="docs/README-AR.md">Árabe</a>] | [<a href="docs/README-VN.md">Vietnamita</a>] | [<a href="docs/README-DA.md">Dinamarquês</a>] | [<a href="docs/README-GR.md">Grego</a>] | [<a href="docs/README-TR.md">Turco</a>] | [<a href="docs/README-NO.md">Norueguês</a>] | [<a href="docs/README-RO.md">Romeno</a>]<br>
  <b>Precisamos da sua ajuda para traduzir este README, a <a href="https://github.com/rustdesk/rustdesk/tree/master/src/lang">Interface do RustDesk</a> e a <a href="https://github.com/rustdesk/doc.rustdesk.com">Documentação do RustDesk</a> para o seu idioma nativo</b>
</p>

> [!Caution]
> **Aviso de Isenção de Responsabilidade por Uso Indevido:** <br>
> Os desenvolvedores do RustDesk não toleram ou apoiam qualquer uso antiético ou ilegal deste software. O uso indevido, como acesso não autorizado, controle ou invasão de privacidade, viola estritamente nossas diretrizes. Os autores não são responsáveis por qualquer uso indevido do aplicativo.


Converse conosco: [Discord](https://discord.gg/nDceKgxnkV) | [Twitter](https://twitter.com/rustdesk) | [Reddit](https://www.reddit.com/r/rustdesk) | [YouTube](https://www.youtube.com/@rustdesk)

[![RustDesk Server Pro](https://img.shields.io/badge/RustDesk%20Server%20Pro-Advanced%20Features-blue)](https://rustdesk.com/pricing.html)

Mais uma solução de desktop remoto, escrita em Rust. Funciona imediatamente, sem necessidade de configuração. Você tem controle total dos seus dados, sem preocupações com segurança. Você pode usar nosso servidor de conexão/retransmissão (rendezvous/relay), [configurar o seu próprio](https://rustdesk.com/server) ou [escrever seu próprio servidor de conexão/retransmissão](https://github.com/rustdesk/rustdesk-server-demo).

![image](https://user-images.githubusercontent.com/71636191/171661982-430285f0-2e12-4b1d-9957-4a58e375304d.png)

O RustDesk acolhe a contribuição de todos. Veja [CONTRIBUTING.md](docs/CONTRIBUTING.md) para ajuda em como começar.

[**Perguntas Frequentes (FAQ)**](https://github.com/rustdesk/rustdesk/wiki/FAQ)

[**DOWNLOAD DOS BINÁRIOS**](https://github.com/rustdesk/rustdesk/releases)

[**VERSÕES NIGHTLY (EM DESENVOLVIMENTO)**](https://github.com/rustdesk/rustdesk/releases/tag/nightly)

[<img src="https://f-droid.org/badge/get-it-on.png"
    alt="Baixe no F-Droid"
    height="80">](https://f-droid.org/en/packages/com.carriez.flutter_hbb)
[<img src="https://flathub.org/api/badge?svg&locale=en"
    alt="Baixe no Flathub"
    height="80">](https://flathub.org/apps/com.rustdesk.RustDesk)

## Dependências

As versões de desktop usam Flutter ou Sciter (descontinuado) para a interface gráfica (GUI). Este tutorial é apenas para o Sciter, por ser mais fácil e amigável para começar. Verifique nosso [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) para instruções de compilação da versão em Flutter.

Por favor, faça o download da biblioteca dinâmica do Sciter por conta própria.

[Windows](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.win/x64/sciter.dll) |
[Linux](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so) |
[macOS](https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.osx/libsciter.dylib)

## Passos básicos para compilar

- Prepare seu ambiente de desenvolvimento Rust e o ambiente de compilação C++

- Instale o [vcpkg](https://github.com/microsoft/vcpkg) e configure a variável de ambiente `VCPKG_ROOT` corretamente

  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
  - Linux/macOS: `vcpkg install libvpx libyuv opus aom`

- Execute `cargo run`

## [Compilar](https://rustdesk.com/docs/en/dev/build/)

## Como Compilar no Linux

### Ubuntu 18 (Debian 10)

```sh
sudo apt install -y zip g++ gcc git curl wget nasm yasm libgtk-3-dev clang libxcb-randr0-dev libxdo-dev         libxfixes-dev libxcb-shape0-dev libxcb-xfixes0-dev libasound2-dev libpulse-dev cmake make         libclang-dev ninja-build libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libpam0g-dev
```

### openSUSE Tumbleweed

```sh
sudo zypper install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libXfixes-devel cmake alsa-lib-devel gstreamer-devel gstreamer-plugins-base-devel xdotool-devel pam-devel
```

### Fedora 28 (CentOS 8)

```sh
sudo yum -y install gcc-c++ git curl wget nasm yasm gcc gtk3-devel clang libxcb-devel libxdo-devel libXfixes-devel pulseaudio-libs-devel cmake alsa-lib-devel gstreamer1-devel gstreamer1-plugins-base-devel pam-devel
```

### Arch (Manjaro)

```sh
sudo pacman -Syu --needed unzip git cmake gcc curl wget yasm nasm zip make pkg-config clang gtk3 xdotool libxcb libxfixes alsa-lib pipewire
```

### Instalar o vcpkg

```sh
git clone https://github.com/microsoft/vcpkg
cd vcpkg
git checkout 2023.04.15
cd ..
vcpkg/bootstrap-vcpkg.sh
export VCPKG_ROOT=$HOME/vcpkg
vcpkg/vcpkg install libvpx libyuv opus aom
```

### Corrigir o libvpx (Para Fedora)

```sh
cd vcpkg/buildtrees/libvpx/src
cd *
./configure
sed -i 's/CFLAGS+=-I/CFLAGS+=-fPIC -I/g' Makefile
sed -i 's/CXXFLAGS+=-I/CXXFLAGS+=-fPIC -I/g' Makefile
make
cp libvpx.a $HOME/vcpkg/installed/x64-linux/lib/
cd
```

### Compilar

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
git clone --recurse-submodules https://github.com/rustdesk/rustdesk
cd rustdesk
mkdir -p target/debug
wget https://raw.githubusercontent.com/c-smile/sciter-sdk/master/bin.lnx/x64/libsciter-gtk.so
mv libsciter-gtk.so target/debug
VCPKG_ROOT=$HOME/vcpkg cargo run
```

## Como compilar com o Docker

Comece clonando o repositório e construindo o contêiner Docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Depois, cada vez que precisar compilar o aplicativo, execute o seguinte comando:

```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

Note que a primeira compilação pode demorar mais até que as dependências sejam armazenadas em cache; as compilações subsequentes serão mais rápidas. Além disso, se você precisar especificar argumentos diferentes para o comando de compilação, poderá fazê-lo ao final do comando na posição `<ARGUMENTOS-OPCIONAIS>`. Por exemplo, se você quiser compilar uma versão de lançamento (release) otimizada, executaria o comando acima seguido de `--release`. O executável resultante estará disponível na pasta `target` do seu sistema e pode ser executado com:

```sh
target/debug/rustdesk
```

Ou, se estiver executando o executável de lançamento:

```sh
target/release/rustdesk
```

Certifique-se de executar esses comandos a partir da raiz do repositório do RustDesk, do contrário o aplicativo pode não encontrar os recursos necessários. Note também que outros subcomandos do cargo, como `install` ou `run`, não são suportados atualmente por este método, pois instalariam ou executariam o programa dentro do contêiner em vez de no sistema hospedeiro.

## Estrutura de Arquivos

- **[libs/hbb_common](https://github.com/rustdesk/rustdesk/tree/master/libs/hbb_common)**: codec de vídeo, configuração, encapsulador (wrapper) tcp/udp, protobuf, funções de sistema de arquivos para transferência de arquivos e algumas outras funções utilitárias.
- **[libs/scrap](https://github.com/rustdesk/rustdesk/tree/master/libs/scrap)**: captura de tela.
- **[libs/enigo](https://github.com/rustdesk/rustdesk/tree/master/libs/enigo)**: controle de teclado/mouse específico de cada plataforma.
- **[libs/clipboard](https://github.com/rustdesk/rustdesk/tree/master/libs/clipboard)**: implementação de copiar e colar arquivos para Windows, Linux e macOS.
- **[src/ui](https://github.com/rustdesk/rustdesk/tree/master/src/ui)**: interface Sciter antiga (descontinuada).
- **[src/server](https://github.com/rustdesk/rustdesk/tree/master/src/server)**: serviços de áudio/área de transferência/entrada/vídeo e conexões de rede.
- **[src/client.rs](https://github.com/rustdesk/rustdesk/tree/master/src/client.rs)**: inicia uma conexão direta (peer connection).
- **[src/rendezvous_mediator.rs](https://github.com/rustdesk/rustdesk/tree/master/src/rendezvous_mediator.rs)**: Comunica-se com o [rustdesk-server](https://github.com/rustdesk/rustdesk-server), aguarda por conexão remota direta (perfuração de túnel TCP / hole punching) ou retransmitida.
- **[src/platform](https://github.com/rustdesk/rustdesk/tree/master/src/platform)**: código específico de cada plataforma.
- **[flutter](https://github.com/rustdesk/rustdesk/tree/master/flutter)**: código Flutter para desktop e dispositivos móveis.
- **[flutter/web/js](https://github.com/rustdesk/rustdesk/tree/master/flutter/web/v1/js)**: JavaScript para o cliente web do Flutter.

## Capturas de Tela

![Gerenciador de Conexões](https://github.com/rustdesk/rustdesk/assets/28412477/db82d4e7-c4bc-4823-8e6f-6af7eadf7651)

![Conectado a um PC Windows](https://github.com/rustdesk/rustdesk/assets/28412477/9baa91e9-3362-4d06-aa1a-7518edcbd7ea)

![Transferência de Arquivos](https://github.com/rustdesk/rustdesk/assets/28412477/39511ad3-aa9a-4f8c-8947-1cce286a46ad)

![Tunelamento TCP](https://github.com/rustdesk/rustdesk/assets/28412477/78e8708f-e87e-4570-8373-1360033ea6c5)
