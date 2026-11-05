<p align="center">
  <img src="res/logo-header.svg" alt="RocketDesk - Seu Acesso Remoto"><br>
</p>

# 🚀 Bem-vindo ao RocketDesk (Customização do RustDesk)

Esta é uma versão customizada e simplificada do RustDesk, idealizada para as nossas operações com a marca **RocketDesk**.
Todas as alterações foram projetadas para simplificar o uso pelos nossos clientes e automatizar nossa distribuição na Microsoft Store via pacote MSIX.

## 🛠 O que mudou nesta versão?

1. **Customização de Marca (Branding):**
   - Todos os logotipos, ícones (incluindo o ícone de bandeja) e nomes do aplicativo (no `Cargo.toml`, `pubspec.yaml` e outros manifestos) foram alterados para **RocketDesk**.
   - As imagens originais foram substituídas pelas nossas (localizadas na pasta `res/`).

2. **Interface do Usuário Simplificada (UX):**
   - Alteramos as páginas em Flutter (`desktop_home_page.dart` e `connection_page.dart`) para separar e evidenciar a experiência em duas ações simples:
     - **Receber Suporte:** Foco em exibir o ID e a Senha para o cliente final.
     - **Prestar Suporte:** Painel para o técnico inserir o ID de destino.
   - Textos e disposições em tela foram adaptados para priorizar o nosso idioma e a clareza dessas duas funções vitais.

3. **Configuração de Servidor Fixo:**
   - Para que nossos clientes não precisem configurar nada, nosso servidor de "rendezvous" (conexão) e a nossa chave pública foram **embutidos nativamente** no código no arquivo de configuração do backend em Rust (`libs/hbb_common/src/config.rs`).

4. **Automação de Build e Distribuição MSIX:**
   - Adicionamos a dependência `msix` no `pubspec.yaml` do Flutter.
   - Incluímos o script e os fluxos (`.github/workflows/build-windows-only.yml` e `ci.yml`) no GitHub Actions para gerar automaticamente não só o `.exe` padrão, mas também o pacote `.msix` exigido pela Microsoft Store.
   - O CI gera o executável no momento do *push* e fornece o artefato `.msix` para download direto na aba "Actions".

---

## 🔒 Onde e Como Configurar o Servidor (Variáveis)

Nós preparamos o ambiente para injetar seus dados confidenciais (chaves e servidores) de forma segura através de variáveis de ambiente. Você **não deve** gravar suas chaves finais nos arquivos do repositório público!

Para configurar o servidor que será compilado junto ao app:
1. No seu servidor CI (ex: GitHub Actions), você deve configurar os "Secrets" chamados `RENDEZVOUS_SERVER`, `API_SERVER` e `RS_PUB_KEY` com os valores da sua infraestrutura.
2. Caso vá compilar *localmente*, basta criar um arquivo `.env` na raiz deste projeto com o seguinte formato:
   ```env
   # .env
   RENDEZVOUS_SERVERS=seu-servidor-rocketdesk.com
   RS_PUB_KEY=sua-chave-publica-do-servidor
   ```
O código Rust irá puxar esses dados durante a compilação do executável ou MSIX.

---

> Abaixo você confere as instruções originais do repositório matriz (RustDesk), inteiramente traduzidas para Português (Brasil).

---

<p align="center">
  <img src="res/logo-header.svg" alt="RocketDesk - Seu Acesso Remoto"><br>
  <a href="#instrucoes-de-compilacao">Build</a> •
  <a href="#como-compilar-com-docker">Docker</a> •
  <a href="#estrutura-de-arquivos">Estrutura</a><br>
</p>

> [!Caution]
> **Aviso de Uso Incorreto:** <br>
> Os desenvolvedores do software original não apoiam usos antiéticos ou ilegais deste software. O uso indevido, como acesso não autorizado, controle ou invasão de privacidade, é estritamente contra as nossas diretrizes. Os autores não são responsáveis por qualquer uso indevido da aplicação.

Mais uma solução de acesso remoto, escrita em Rust. Funciona perfeitamente sem a necessidade de configuração. Você tem controle total dos seus dados, sem preocupações com segurança. Você pode usar nosso servidor de *rendezvous/relay* (agora embutido na aplicação), configurar o seu próprio, ou escrever um do zero.

## Dependências

A versão desktop utiliza Flutter. Verifique nosso [CI](https://github.com/rustdesk/rustdesk/blob/master/.github/workflows/flutter-build.yml) para o fluxo de construção da versão Flutter.
É necessário instalar vcpkg, Clang, llvm, e o SDK do Flutter para a compilação.

## Instruções de Compilação (Localmente)

- Prepare o seu ambiente de desenvolvimento Rust e o ambiente de build C++
- Instale [vcpkg](https://github.com/microsoft/vcpkg), e defina a variável `VCPKG_ROOT` corretamente.
  - Windows: `vcpkg install libvpx:x64-windows-static libyuv:x64-windows-static opus:x64-windows-static aom:x64-windows-static`
  - Linux/macOS: `vcpkg install libvpx libyuv opus aom`
- rode `cargo run`

## Como compilar com Docker

Comece clonando o repositório e compilando o container Docker:

```sh
git clone https://github.com/rustdesk/rustdesk
cd rustdesk
git submodule update --init --recursive
docker build -t "rustdesk-builder" .
```

Então, cada vez que precisar compilar a aplicação, rode o comando:
```sh
docker run --rm -it -v $PWD:/home/user/rustdesk -v rustdesk-git-cache:/home/user/.cargo/git -v rustdesk-registry-cache:/home/user/.cargo/registry -e PUID="$(id -u)" -e PGID="$(id -g)" rustdesk-builder
```

## Estrutura de Arquivos

- **libs/hbb_common**: codec de vídeo, configuração, wrapper tcp/udp, protobuf, funções de fs e de utilidade.
- **libs/scrap**: captura de tela.
- **libs/enigo**: controle de teclado/mouse por plataforma.
- **libs/clipboard**: implementação de copiar e colar.
- **src/server**: serviços de áudio/área de transferência/entrada/vídeo.
- **src/client.rs**: início da conexão par a par.
- **src/rendezvous_mediator.rs**: comunicação com o servidor.
- **flutter**: Código frontend (Interface do cliente final e do técnico) em Flutter.
