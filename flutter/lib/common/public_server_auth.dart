const kPublicServerApiServer = 'https://admin.rustdesk.com';
const kPublicServerAccessToken = 'public-access-token';
const kPublicServerOidcPrefix = 'public-server/';

bool isPublicServerPeer(String id) {
  final parts = id.replaceAll(' ', '').split('@');
  return parts.length > 1 && parts[1].split('?').first == 'public';
}
