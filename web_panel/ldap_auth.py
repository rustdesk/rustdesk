"""
LDAP/Active Directory Authentication Module
"""

import sqlite3

# Try to import ldap3, if not available, provide stub
try:
    from ldap3 import Server, Connection, ALL, NTLM, SIMPLE
    LDAP_AVAILABLE = True
except ImportError:
    LDAP_AVAILABLE = False
    print("[LDAP] ldap3 not installed. Run: pip install ldap3")

DB_PATH = 'rustdesk.db'


def get_ldap_config():
    """Get LDAP configuration from database"""
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    c = conn.cursor()
    
    config = {}
    for row in c.execute("SELECT key, value FROM settings WHERE key LIKE 'ldap_%'").fetchall():
        key = row['key'].replace('ldap_', '')
        config[key] = row['value']
    
    conn.close()
    return config


def is_ldap_enabled():
    """Check if LDAP authentication is enabled"""
    config = get_ldap_config()
    return config.get('enabled') == '1' and LDAP_AVAILABLE


def ldap_authenticate(username, password):
    """
    Authenticate user against LDAP/Active Directory
    
    Returns:
        dict: User info if authenticated, None otherwise
        {
            'username': 'jdoe',
            'email': 'jdoe@example.com',
            'display_name': 'John Doe',
            'groups': ['Domain Users', 'IT Department']
        }
    """
    if not LDAP_AVAILABLE:
        print("[LDAP] ldap3 library not available")
        return None
    
    config = get_ldap_config()
    
    if config.get('enabled') != '1':
        print("[LDAP] LDAP is disabled")
        return None
    
    server_url = config.get('server', '')
    base_dn = config.get('base_dn', '')
    bind_dn = config.get('bind_dn', '')
    bind_password = config.get('bind_password', '')
    
    if not server_url or not base_dn:
        print("[LDAP] Server or Base DN not configured")
        return None
    
    try:
        # Connect to LDAP server
        server = Server(server_url, get_info=ALL)
        
        # Try different authentication methods
        user_dn = None
        user_info = None
        
        # Method 1: Direct bind with username@domain (AD style)
        if '@' in username or '\\' in username:
            # Username already contains domain
            user_principal = username
        else:
            # Try to extract domain from server URL or base DN
            domain = extract_domain_from_base_dn(base_dn)
            user_principal = f"{username}@{domain}" if domain else username
        
        # Try NTLM authentication (for Active Directory)
        try:
            conn = Connection(server, user=user_principal, password=password, authentication=NTLM)
            if conn.bind():
                user_info = search_user(conn, base_dn, username)
                conn.unbind()
                if user_info:
                    return user_info
        except Exception as e:
            print(f"[LDAP] NTLM auth failed: {e}")
        
        # Try simple bind with constructed DN
        try:
            # First bind as admin to search for user
            if bind_dn and bind_password:
                admin_conn = Connection(server, user=bind_dn, password=bind_password, authentication=SIMPLE)
                if admin_conn.bind():
                    # Search for user DN
                    user_dn = find_user_dn(admin_conn, base_dn, username)
                    admin_conn.unbind()
            
            if user_dn:
                # Bind as user to verify password
                user_conn = Connection(server, user=user_dn, password=password, authentication=SIMPLE)
                if user_conn.bind():
                    user_info = search_user(user_conn, base_dn, username)
                    user_conn.unbind()
                    if user_info:
                        return user_info
        except Exception as e:
            print(f"[LDAP] Simple bind failed: {e}")
        
        # Try direct bind with sAMAccountName (AD)
        try:
            sam_dn = f"CN={username},{base_dn}"
            conn = Connection(server, user=sam_dn, password=password)
            if conn.bind():
                user_info = {
                    'username': username,
                    'email': f"{username}@{extract_domain_from_base_dn(base_dn)}",
                    'display_name': username,
                    'groups': []
                }
                conn.unbind()
                return user_info
        except Exception as e:
            print(f"[LDAP] Direct CN bind failed: {e}")
        
        print(f"[LDAP] All authentication methods failed for user: {username}")
        return None
        
    except Exception as e:
        print(f"[LDAP] Error: {e}")
        return None


def find_user_dn(conn, base_dn, username):
    """Find user DN by username"""
    search_filter = f"(&(objectClass=person)(|(sAMAccountName={username})(uid={username})(cn={username})))"
    
    conn.search(base_dn, search_filter, attributes=['distinguishedName'])
    
    if conn.entries:
        return str(conn.entries[0].distinguishedName)
    return None


def search_user(conn, base_dn, username):
    """Search for user and return info"""
    search_filter = f"(&(objectClass=person)(|(sAMAccountName={username})(uid={username})(cn={username})))"
    attributes = ['sAMAccountName', 'uid', 'cn', 'mail', 'displayName', 'memberOf', 'givenName', 'sn']
    
    conn.search(base_dn, search_filter, attributes=attributes)
    
    if not conn.entries:
        return None
    
    entry = conn.entries[0]
    
    # Extract username
    user = str(entry.sAMAccountName) if hasattr(entry, 'sAMAccountName') else \
           str(entry.uid) if hasattr(entry, 'uid') else \
           str(entry.cn) if hasattr(entry, 'cn') else username
    
    # Extract email
    email = str(entry.mail) if hasattr(entry, 'mail') and entry.mail else f"{user}@localhost"
    
    # Extract display name
    display_name = str(entry.displayName) if hasattr(entry, 'displayName') and entry.displayName else \
                   str(entry.cn) if hasattr(entry, 'cn') else user
    
    # Extract groups
    groups = []
    if hasattr(entry, 'memberOf'):
        for group_dn in entry.memberOf:
            # Extract CN from group DN
            cn_part = str(group_dn).split(',')[0]
            if cn_part.upper().startswith('CN='):
                groups.append(cn_part[3:])
    
    return {
        'username': user,
        'email': email,
        'display_name': display_name,
        'groups': groups
    }


def extract_domain_from_base_dn(base_dn):
    """Extract domain from base DN (e.g., DC=example,DC=com -> example.com)"""
    if not base_dn:
        return None
    
    parts = []
    for part in base_dn.split(','):
        part = part.strip()
        if part.upper().startswith('DC='):
            parts.append(part[3:])
    
    return '.'.join(parts) if parts else None


def sync_ldap_user_to_db(ldap_user, is_admin=False):
    """
    Create or update user in local database from LDAP info
    
    Returns user ID
    """
    import hashlib
    
    conn = sqlite3.connect(DB_PATH)
    c = conn.cursor()
    
    username = ldap_user['username']
    email = ldap_user.get('email', '')
    
    # Check if user exists
    existing = c.execute("SELECT id FROM users WHERE username = ?", (username,)).fetchone()
    
    if existing:
        # Update existing user
        c.execute("UPDATE users SET email = ? WHERE username = ?", (email, username))
        user_id = existing[0]
    else:
        # Create new user with random password (they'll use LDAP auth)
        random_password = hashlib.sha256(f"ldap_{username}_{email}".encode()).hexdigest()
        c.execute("INSERT INTO users (username, password, email, is_admin, status) VALUES (?, ?, ?, ?, 1)",
                  (username, random_password, email, 1 if is_admin else 0))
        user_id = c.lastrowid
    
    conn.commit()
    conn.close()
    
    return user_id


def test_ldap_connection():
    """Test LDAP connection with current settings"""
    if not LDAP_AVAILABLE:
        return False, "ldap3 library not installed. Run: pip install ldap3"
    
    config = get_ldap_config()
    
    if not config.get('server'):
        return False, "LDAP server not configured"
    
    try:
        server = Server(config['server'], get_info=ALL)
        
        if config.get('bind_dn') and config.get('bind_password'):
            conn = Connection(server, user=config['bind_dn'], password=config['bind_password'])
        else:
            conn = Connection(server)
        
        if conn.bind():
            info = f"Connected to {server.host}"
            if server.info:
                info += f" ({server.info.vendor_name})" if server.info.vendor_name else ""
            conn.unbind()
            return True, info
        else:
            return False, f"Bind failed: {conn.result}"
    
    except Exception as e:
        return False, str(e)


# Test function
if __name__ == '__main__':
    print("LDAP Module Test")
    print(f"LDAP Available: {LDAP_AVAILABLE}")
    
    if LDAP_AVAILABLE:
        success, message = test_ldap_connection()
        print(f"Connection Test: {'OK' if success else 'FAILED'} - {message}")






