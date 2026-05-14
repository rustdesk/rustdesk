# Arquitetura do Fork RustDesk

## Objetivo

Cliente RustDesk customizado para suporte remoto seguro.

Características:
- conexão exclusiva ao servidor privado
- configuração de rede hardcoded
- sem configuração manual do usuário
- confirmação obrigatória para toda conexão
- sem acesso unattended
- interface simplificada

## Stack

### Frontend
- Flutter

### Core
- Rust

### Networking
- hbbs (ID server)
- hbbr (relay)

## Componentes relevantes

### Configuração
libs/hbb_common/src/config.rs

Responsável por:
- rendezvous_server
- relay_server
- key
- access_mode
- password policies

### Sessão
rust/src/server.rs

Responsável por:
- controle de sessão
- permissões
- confirmação de acesso

### Flutter
lib/views/settings/

Responsável por:
- tela de configurações
- inputs de rede
- preferências do usuário

## Restrições

- nunca permitir conexão sem confirmação
- nunca permitir alteração manual do servidor
- nunca aceitar senha permanente
- nunca aceitar senha temporária
- impedir conexões diretas por IP
- manter compatibilidade com protocolo RustDesk