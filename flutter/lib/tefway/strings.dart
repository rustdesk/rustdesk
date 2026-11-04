// Product copy for the Tefway Remote surfaces.
//
// The Tefway product ships in Brazilian Portuguese only, and every visible
// string of the Tefway UI lives here instead of being spread across widgets.
// Strings shared with the stock RustDesk UI keep using `translate()`.

class TefwayStrings {
  TefwayStrings._();

  // Product identity.
  static const productName = 'Tefway Remote';
  static const masterSubtitle = 'Central do técnico · Master';
  static const clientSubtitle = 'Assistência ao cliente';
  static const masterSurfaceTitle = 'Central do técnico';
  static const supportSignature = 'Suporte Tefway · Servir por Amor';
  static const licenseNote = 'Baseado no RustDesk · AGPL-3.0';

  // Master.
  static const newConnection = 'Nova conexão';
  static const remoteIdLabel = 'ID do computador do cliente';
  static const remoteIdHint = 'Digite o ID que o cliente informou';
  static const connect = 'Conectar';
  static const connecting = 'Solicitando conexão…';
  static const activeSessions = 'Sessões ativas';
  static const noSessions = 'Nenhuma sessão aberta';
  static const noSessionsHint =
      'Os atendimentos que você iniciar aparecem aqui, com a máquina de destino e as ações daquela sessão.';
  static const sessionOpen = 'Sessão aberta';
  static const sessionFocused = 'Em primeiro plano';
  static const openSession = 'Abrir sessão';
  static const endSession = 'Encerrar sessão';
  static const sessionSince = 'Aberta às';
  static const sessionsHint =
      'Cada atendimento abre uma janela própria. Encerrar a janela encerra a sessão remota.';
  static const endSessionTitle = 'Encerrar sessão?';
  static String endSessionBody(String id) =>
      'A sessão com o computador $id será encerrada agora. O cliente verá o atendimento terminar.';
  static const cancel = 'Cancelar';
  static const end = 'Encerrar';

  // Connection validation and outcomes.
  static const idRequired = 'Informe o ID do computador.';
  static const idInvalid = 'O ID deve conter apenas números, com pelo menos 6 dígitos.';
  static const idSelf = 'Este é o ID desta máquina. Informe o ID do computador do cliente.';
  static const connectFailed =
      'Não foi possível abrir a sessão. Verifique o ID e a conexão com o servidor Tefway.';
  static const serverUnavailable =
      'Servidor Tefway indisponível. Verifique a conexão deste computador com a internet.';

  // Client.
  static const clientIdLabel = 'Seu ID de atendimento';
  static const copyId = 'Copiar ID';
  static const copiedId = 'ID copiado';
  static const clientGuidance =
      'O suporte remoto só poderá começar depois da sua autorização.';
  static const clientAwaiting = 'Aguardando atendimento';
  static const clientAwaitingDetail = 'Ninguém está conectado ao seu computador.';
  static const clientStarting = 'Inicializando…';
  static const clientConnecting = 'Conectando ao servidor';
  static const clientConnectingDetail =
      'Estabelecendo contato com o servidor Tefway.';
  static const clientServerDown = 'Servidor indisponível';
  static const clientServerDownDetail =
      'Não foi possível falar com o servidor Tefway. Este computador continua sem acesso remoto.';
  static const clientSessionActive = 'Atendimento ativo';
  static String clientSessionDetail(int count) => count == 1
      ? 'Um acesso remoto está em andamento neste computador.'
      : '$count acessos remotos estão em andamento neste computador.';
  static const clientSessionsHint =
      'O encerramento do acesso é feito na janela de atendimento remoto que está aberta neste computador.';
  static const clientNoOutgoing =
      'Este aplicativo não inicia conexões para outros computadores.';

  // Notices.
  static const noticePrivacy = 'Seu computador só é acessado com sua autorização.';
  static const uacTitle = 'Permissões administrativas';
  static const uacBody =
      'Algumas janelas com privilégios administrativos podem exigir permissões adicionais.';
  static const uacDetails =
      'O UAC do Windows protege janelas elevadas. Enquanto elas estiverem em primeiro plano, o '
      'controle remoto pode não alcançá-las. O Tefway Remote não instala serviços nem eleva o '
      'processo automaticamente para contornar essa proteção.';
  static const waylandTitle = 'Sessão Wayland';
  static const waylandBody =
      'O ambiente Wayland limita a captura de tela e o controle de entrada em alguns aplicativos.';
  static const waylandDetails =
      'A captura depende do portal do sistema e da autorização de compartilhamento de tela. '
      'Se a tela aparecer preta ou sem resposta, verifique o diálogo do portal do Wayland.';
  static const loginWaylandTitle = 'Tela de login em Wayland';
  static const loginWaylandBody =
      'Não é possível capturar a tela de login quando ela roda em Wayland.';
  static const selinuxTitle = 'SELinux ativo';
  static const selinuxBody =
      'O SELinux pode bloquear a captura de tela ou o controle de entrada.';
  static const selinuxDetails =
      'Com o SELinux em modo enforcing, a política do sistema pode negar o acesso aos '
      'dispositivos de entrada e ao framebuffer. O ajuste é responsabilidade do administrador '
      'do computador; o Tefway Remote não altera a política do sistema.';
  static const learnMore = 'Saiba mais';
  static const hideDetails = 'Ocultar';

  // Errors.
  static const errorTitle = 'Falha de rede';
  static const retry = 'Tentar novamente';

  // Footer.
  static const versionLabel = 'Versão';
  static const serverOk = 'Servidor conectado';
  static const serverConnecting = 'Conectando ao servidor';
  static const serverDown = 'Servidor indisponível';
}
