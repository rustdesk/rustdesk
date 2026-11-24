import { t, type Locale } from './i18n';

export type ErrorKind = 'auth' | 'network' | 'codec' | 'generic';

export interface FormattedError {
  kind: ErrorKind;
  title: string;
  message: string;
}

/**
 * Classifies and localizes gateway, socket, and WebCodecs errors into concise,
 * human-readable alerts with an error category for contextual recovery actions.
 */
export function formatConnectionError(locale: Locale, rawMessage: string): FormattedError {
  const raw = rawMessage.trim();
  const lower = raw.toLowerCase();

  if (!raw) {
    return {
      kind: 'generic',
      title: t(locale, 'errTitleGeneric'),
      message: t(locale, 'errMsgUnexpected'),
    };
  }

  // 1. Authentication & password errors
  if (lower.includes('gateway access token') || lower.includes('令牌') || /\btoken\b/.test(lower)) {
    return {
      kind: 'auth',
      title: t(locale, 'errTitleGatewayToken'),
      message: t(locale, 'errMsgGatewayToken'),
    };
  }
  if (lower.includes('wrong password') || lower.includes('rejected the password')) {
    return {
      kind: 'auth',
      title: t(locale, 'errTitleWrongPassword'),
      message: t(locale, 'errMsgWrongPassword'),
    };
  }
  if (
    lower.includes('empty password') ||
    lower.includes('requires a password') ||
    lower.includes('no password access')
  ) {
    return {
      kind: 'auth',
      title: t(locale, 'errTitlePasswordRequired'),
      message: t(locale, 'errMsgPasswordRequired'),
    };
  }

  // 2. WebCodecs / decoder errors
  if (lower.includes('secure context') || (lower.includes('webcodecs') && lower.includes('https'))) {
    return {
      kind: 'codec',
      title: t(locale, 'errTitleCodec'),
      message: t(locale, 'errMsgInsecureContext'),
    };
  }
  if (
    lower.includes('webcodecs') ||
    lower.includes('videodecoder') ||
    lower.includes('cannot decode') ||
    lower.includes('不支持 vp8')
  ) {
    return {
      kind: 'codec',
      title: t(locale, 'errTitleCodec'),
      message: lower.includes('cannot decode') || lower.includes('不支持 vp8')
        ? t(locale, 'detailNoCodecs')
        : t(locale, 'errMsgWebCodecsUnsupported'),
    };
  }

  // 3. Network, DNS, server key, peer offline, timeout, and gateway errors
  if (
    lower.includes('dns lookup') ||
    lower.includes('enotfound') ||
    lower.includes('eai_again') ||
    lower.includes('eai_nodata') ||
    lower.includes('eai_noname') ||
    lower.includes('eservfail') ||
    lower.includes('域名解析')
  ) {
    const match = raw.match(/DNS lookup (?:failed|timed out) for "([^"]+)"/i);
    const domain = match?.[1];
    return {
      kind: 'network',
      title: t(locale, 'errTitleDns'),
      message: domain
        ? locale === 'zh'
          ? `无法解析域名「${domain}」，请检查域名拼写、去掉 http:// 前缀并确认 DNS 记录正常。`
          : `Could not resolve domain "${domain}". Check the spelling, remove any http:// prefix, and verify DNS records.`
        : t(locale, 'errMsgDns'),
    };
  }
  if (
    lower.includes('not a rustdesk') ||
    lower.includes('http response') ||
    lower.includes('http/https') ||
    lower.includes('tls/https') ||
    lower.includes('ssh service') ||
    lower.includes('invalid wire type') ||
    lower.includes('index out of range') ||
    lower.includes('frame overflow') ||
    lower.includes('invalid protocol')
  ) {
    const isHttp = lower.includes('http') || lower.includes('tls');
    return {
      kind: 'network',
      title: t(locale, 'errTitleNotRustDesk'),
      message: t(locale, isHttp ? 'errMsgNotRustDeskHttp' : 'errMsgNotRustDeskProto'),
    };
  }
  if (
    lower.includes('signature') ||
    lower.includes('public key') ||
    lower.includes('server key') ||
    lower.includes('key mismatch') ||
    lower.includes('decrypt')
  ) {
    return {
      kind: 'network',
      title: t(locale, 'errTitleServerKey'),
      message: t(locale, 'errMsgServerKey'),
    };
  }
  if (
    lower.includes('timed out') ||
    lower.includes('timeout') ||
    lower.includes('etimedout') ||
    lower.includes('did not answer in time')
  ) {
    return {
      kind: 'network',
      title: t(locale, 'errTitleTimeout'),
      message: t(locale, 'errMsgTimeout'),
    };
  }
  if (
    lower.includes('offline') ||
    lower.includes('id does not exist') ||
    lower.includes('peer not found') ||
    lower.includes('not match this host')
  ) {
    return {
      kind: 'network',
      title: t(locale, 'errTitleOffline'),
      message: t(locale, 'errMsgOffline'),
    };
  }
  if (lower.includes('gateway') || lower.includes('网关')) {
    return {
      kind: 'network',
      title: t(locale, 'errTitleGateway'),
      message: t(locale, 'errMsgGateway'),
    };
  }
  if (
    lower.includes('cannot reach') ||
    lower.includes('connection refused') ||
    lower.includes('econnrefused') ||
    lower.includes('failed to connect') ||
    lower.includes('os error 111') ||
    lower.includes('network is unreachable') ||
    lower.includes('enetunreach') ||
    lower.includes('ehostunreach')
  ) {
    return {
      kind: 'network',
      title: t(locale, 'errTitleUnreachable'),
      message: t(locale, 'errMsgUnreachable'),
    };
  }

  // 4. Fallback for other host/protocol messages
  return {
    kind: 'generic',
    title: t(locale, 'errTitleGeneric'),
    message: raw,
  };
}
