import assert from 'node:assert/strict';
import test from 'node:test';
import { formatConnectionError } from './errors';

test('classifies wrong password into auth error in zh and en', () => {
  const zh = formatConnectionError(
    'zh',
    'The host rejected the password. Check the permanent password and connect again.',
  );
  assert.equal(zh.kind, 'auth');
  assert.equal(zh.title, '密码错误');
  assert.match(zh.message, /密码/);

  const en = formatConnectionError('en', 'wrong password');
  assert.equal(en.kind, 'auth');
  assert.equal(en.title, 'Incorrect Password');
});

test('classifies empty/required password into auth error', () => {
  const res = formatConnectionError(
    'zh',
    'The host requires a password. Enter the permanent password set on the desktop and connect again.',
  );
  assert.equal(res.kind, 'auth');
  assert.equal(res.title, '需要访问密码');
});

test('classifies gateway access token error into auth error', () => {
  const res = formatConnectionError('zh', 'The gateway access token is missing or incorrect.');
  assert.equal(res.kind, 'auth');
  assert.equal(res.title, '网关令牌无效');
});

test('classifies offline and missing peer ID into network error', () => {
  const res = formatConnectionError(
    'zh',
    'The remote desktop is offline, or the ID does not match this host.',
  );
  assert.equal(res.kind, 'network');
  assert.equal(res.title, '设备不在线');
});

test('classifies server key / signature mismatch into network error', () => {
  const res = formatConnectionError('zh', 'Invalid server public key signature');
  assert.equal(res.kind, 'network');
  assert.equal(res.title, '服务器公钥不匹配');
});

test('classifies connection refused / unreachable and DNS failures distinctly', () => {
  const refused = formatConnectionError(
    'zh',
    'Cannot reach the server or host. Check the address, port, and firewall settings.',
  );
  assert.equal(refused.kind, 'network');
  assert.equal(refused.title, '无法连接到目标');

  const dnsEn = formatConnectionError('en', 'getaddrinfo ENOTFOUND hbbs.invalid');
  assert.equal(dnsEn.kind, 'network');
  assert.equal(dnsEn.title, 'DNS Resolution Failed');

  const dnsZh = formatConnectionError(
    'zh',
    'DNS lookup failed for "rs.bad-domain.test". Check the domain spelling, remove any http:// prefix, and verify DNS records.',
  );
  assert.equal(dnsZh.kind, 'network');
  assert.equal(dnsZh.title, '域名解析失败');
  assert.match(dnsZh.message, /rs\.bad-domain\.test/);
});

test('classifies timeout into network error', () => {
  const res = formatConnectionError(
    'zh',
    'The host did not answer in time. It may be offline, or a firewall is blocking the port.',
  );
  assert.equal(res.kind, 'network');
  assert.equal(res.title, '连接超时');
});

test('classifies WebCodecs and insecure context into codec error', () => {
  const insecure = formatConnectionError('zh', 'WebCodecs requires HTTPS or a secure context');
  assert.equal(insecure.kind, 'codec');
  assert.equal(insecure.title, '浏览器解码异常');
  assert.match(insecure.message, /HTTPS/);

  const noCodec = formatConnectionError(
    'zh',
    'This browser cannot decode VP8, VP9, H.264, HEVC, or AV1.',
  );
  assert.equal(noCodec.kind, 'codec');
  assert.match(noCodec.message, /VP8/);
});

test('falls back cleanly for empty or unknown error messages', () => {
  const empty = formatConnectionError('zh', '   ');
  assert.equal(empty.kind, 'generic');
  assert.equal(empty.title, '连接失败');
  assert.ok(empty.message.length > 0);

  const custom = formatConnectionError('en', 'Custom host policy rejected session');
  assert.equal(custom.kind, 'generic');
  assert.equal(custom.message, 'Custom host policy rejected session');
});

test('classifies non-RustDesk HTTP and protobuf wire-type errors cleanly in zh and en', () => {
  const httpZh = formatConnectionError(
    'zh',
    'Not a RustDesk server: received an HTTP response instead of RustDesk protocol data.',
  );
  assert.equal(httpZh.kind, 'network');
  assert.equal(httpZh.title, '不是 RustDesk 服务器');
  assert.match(httpZh.message, /HTTP/);
  assert.doesNotMatch(httpZh.message, /wire type/i);

  const wireZh = formatConnectionError('zh', 'invalid wire type 4 at offset 1');
  assert.equal(wireZh.kind, 'network');
  assert.equal(wireZh.title, '不是 RustDesk 服务器');
  assert.match(wireZh.message, /ID 服务器/);

  const httpEn = formatConnectionError(
    'en',
    'Not a RustDesk server: received an HTTP response instead of RustDesk protocol data.',
  );
  assert.equal(httpEn.kind, 'network');
  assert.equal(httpEn.title, 'Not a RustDesk Server');
  assert.match(httpEn.message, /HTTP/);
});

