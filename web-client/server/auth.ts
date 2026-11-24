import crypto from 'node:crypto';
import dns from 'node:dns/promises';
import net from 'node:net';

export const RENDEZVOUS_PORT = 21116;
export const RELAY_PORT = 21117;
export const DEFAULT_DIRECT_PORT = RENDEZVOUS_PORT + 2; // 21118

/**
 * RustDesk password challenge-response:
 * h1 = SHA256(password || salt)
 * response = SHA256(h1_raw_bytes || challenge)
 */
export function passwordResponse(password: string, salt: string, challenge: string): Buffer {
  const h1 = crypto
    .createHash('sha256')
    .update(Buffer.from(password, 'utf8'))
    .update(Buffer.from(salt, 'utf8'))
    .digest();

  return crypto
    .createHash('sha256')
    .update(h1)
    .update(Buffer.from(challenge, 'utf8'))
    .digest();
}

/**
 * Normalizes user-pasted host / server strings by stripping URL schemes
 * (http://, https://, ws://, wss://, tcp://, rustdesk://), fullwidth colons,
 * trailing paths/queries/fragments, and surrounding whitespace.
 */
export function sanitizeHostInput(raw: string): string {
  let s = (raw ?? '')
    .replace(/[\u200B-\u200D\uFEFF]/g, '')
    .trim()
    .replace(/：/g, ':');

  // Strip common URL protocol prefixes users accidentally paste
  s = s.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, '');

  // Strip path, query, or hash suffix if present
  const cutIdx = s.search(/[/?#]/);
  if (cutIdx !== -1) {
    s = s.slice(0, cutIdx);
  }

  return s.trim();
}

export function hostHasPort(host: string): boolean {
  const trimmed = sanitizeHostInput(host);
  if (trimmed.startsWith('[')) {
    return trimmed.includes(']:');
  }
  const first = trimmed.indexOf(':');
  return first !== -1 && first === trimmed.lastIndexOf(':');
}

export function isIpStr(host: string): boolean {
  const trimmed = sanitizeHostInput(host);
  if (trimmed.startsWith('[')) {
    const end = trimmed.indexOf(']');
    if (end === -1) return false;
    return net.isIP(trimmed.slice(1, end)) !== 0;
  }
  if (hostHasPort(trimmed)) {
    const idx = trimmed.lastIndexOf(':');
    return net.isIP(trimmed.slice(0, idx)) !== 0;
  }
  return net.isIP(trimmed) !== 0;
}

/**
 * Formats host + default port like hbb_common::socket_client::check_port,
 * while tolerating pasted URLs, fullwidth colons, and trailing slashes.
 */
export function checkPort(host: string, defaultPort: number): { host: string; port: number } {
  const trimmed = sanitizeHostInput(host);
  if (trimmed.startsWith('[')) {
    const end = trimmed.indexOf(']');
    if (end !== -1) {
      const addr = trimmed.slice(1, end);
      const rest = trimmed.slice(end + 1);
      if (rest.startsWith(':')) {
        const parsed = Number.parseInt(rest.slice(1), 10);
        if (Number.isFinite(parsed) && parsed > 0 && parsed <= 65535) {
          return { host: addr, port: parsed };
        }
      }
      return { host: addr, port: defaultPort };
    }
  }
  if (net.isIP(trimmed) === 6) {
    return { host: trimmed, port: defaultPort };
  }
  const idx = trimmed.lastIndexOf(':');
  if (idx !== -1 && trimmed.indexOf(':') === idx) {
    const hostPart = trimmed.slice(0, idx).trim();
    const portPart = trimmed.slice(idx + 1).trim();
    const parsed = Number.parseInt(portPart, 10);
    if (hostPart && /^\d+$/.test(portPart) && parsed > 0 && parsed <= 65535) {
      return { host: hostPart, port: parsed };
    }
  }
  return { host: trimmed, port: defaultPort };
}

export function formatHostPort(host: string, port: number): string {
  if (net.isIP(host) === 6) {
    return `[${host}]:${port}`;
  }
  return `${host}:${port}`;
}

/**
 * Computes RustDesk direct login username (IP:port or [IPv6]:port).
 * Resolves hostnames to IPv4 first (falling back to IPv6) to match TCP dial order.
 */
export async function directUsername(host: string, port: number): Promise<string> {
  const trimmed = sanitizeHostInput(host);
  if (isIpStr(trimmed) && hostHasPort(trimmed)) {
    return trimmed;
  }
  const parsed = checkPort(trimmed, port);
  if (net.isIP(parsed.host) !== 0) {
    return formatHostPort(parsed.host, parsed.port);
  }
  try {
    const records = await dns.lookup(parsed.host, { all: true });
    const preferred = records.find((r) => r.family === 4) ?? records[0];
    if (preferred?.address) {
      return formatHostPort(preferred.address, parsed.port);
    }
  } catch {
    // Fall back to host:port when DNS resolution is unavailable
  }
  return formatHostPort(parsed.host, parsed.port);
}

/**
 * Constant-time equality check for optional GATEWAY_TOKEN.
 * Trims both expected and given strings. Empty/undefined expected means open access.
 */
export function tokenMatches(expected: string | undefined | null, given: string): boolean {
  const cleanExpected = (expected ?? '').trim();
  if (!cleanExpected) return true;
  const a = Buffer.from(cleanExpected, 'utf8');
  const b = Buffer.from((given ?? '').trim(), 'utf8');
  if (a.length !== b.length) return false;
  return crypto.timingSafeEqual(a, b);
}

export function isValidRelayUuid(uuid: string): boolean {
  if (!uuid || Buffer.byteLength(uuid, 'utf8') > 128) return false;
  return /^[A-Za-z0-9_-]+$/.test(uuid);
}
