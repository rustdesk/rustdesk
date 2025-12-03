#!/usr/bin/env python3
"""
RustDesk Web Management Panel
Tailwind CSS + DataTables + Chart.js

Запуск: python server.py
URL: http://localhost:21114
"""

from flask import Flask, request, jsonify, render_template_string, redirect, url_for, session, make_response, send_from_directory
from functools import wraps
import jwt
import json
import time
import hashlib
import os
import sqlite3
from datetime import datetime, timedelta
from flask_cors import CORS

# LDAP Module
try:
    from ldap_auth import ldap_authenticate, is_ldap_enabled, sync_ldap_user_to_db, test_ldap_connection, LDAP_AVAILABLE
except ImportError:
    LDAP_AVAILABLE = False
    def ldap_authenticate(u, p): return None
    def is_ldap_enabled(): return False
    def sync_ldap_user_to_db(u, a=False): return None
    def test_ldap_connection(): return False, "LDAP module not found"

app = Flask(__name__)
app.secret_key = os.environ.get('SECRET_KEY', 'rustdesk-web-panel-secret-key-2024')

# Enable CORS for API endpoints
CORS(app, resources={r"/api/*": {"origins": "*", "methods": ["GET", "POST", "OPTIONS"], "allow_headers": ["Content-Type", "Authorization"]}})

# Configuration
HOST = os.environ.get('API_HOST', '0.0.0.0')  # Listen on all interfaces
PORT = int(os.environ.get('API_PORT', 21114))
JWT_SECRET = 'rustdesk-api-jwt-secret'
DB_PATH = 'rustdesk.db'

# SSL Configuration
SSL_ENABLED = os.environ.get('SSL_ENABLED', 'true').lower() == 'true'
SSL_CERT = os.path.join(os.path.dirname(__file__), '10.21.31.11+2.pem')
SSL_KEY = os.path.join(os.path.dirname(__file__), '10.21.31.11+2-key.pem')

# Static files directory
STATIC_DIR = os.path.join(os.path.dirname(__file__), 'static')

# ==================== DATABASE ====================

def init_db():
    conn = sqlite3.connect(DB_PATH)
    c = conn.cursor()
    
    c.execute('''CREATE TABLE IF NOT EXISTS users (
        id INTEGER PRIMARY KEY,
        username TEXT UNIQUE,
        password TEXT,
        email TEXT,
        is_admin INTEGER DEFAULT 0,
        status INTEGER DEFAULT 1,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )''')
    
    c.execute('''CREATE TABLE IF NOT EXISTS devices (
        id TEXT PRIMARY KEY,
        uuid TEXT,
        hostname TEXT,
        os TEXT,
        username TEXT,
        version TEXT,
        cpu TEXT,
        memory TEXT,
        ip TEXT,
        group_name TEXT DEFAULT 'Default',
        user_id INTEGER,
        online INTEGER DEFAULT 0,
        last_seen TIMESTAMP,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id)
    )''')
    
    c.execute('''CREATE TABLE IF NOT EXISTS address_books (
        id INTEGER PRIMARY KEY,
        user_id INTEGER,
        data TEXT,
        updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        FOREIGN KEY (user_id) REFERENCES users(id)
    )''')
    
    c.execute('''CREATE TABLE IF NOT EXISTS audit_logs (
        id INTEGER PRIMARY KEY,
        type TEXT,
        device_id TEXT,
        peer_id TEXT,
        action TEXT,
        data TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )''')
    
    c.execute('''CREATE TABLE IF NOT EXISTS connections (
        id INTEGER PRIMARY KEY,
        device_id TEXT,
        peer_id TEXT,
        conn_type TEXT,
        started_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
        ended_at TIMESTAMP,
        duration INTEGER
    )''')
    
    c.execute('''CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT
    )''')
    
    # Default admin
    try:
        c.execute("INSERT INTO users (username, password, email, is_admin) VALUES (?, ?, ?, ?)",
                  ('admin', hash_password('admin123'), 'admin@localhost', 1))
    except sqlite3.IntegrityError:
        pass
    
    conn.commit()
    conn.close()

def get_db():
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn

def hash_password(password):
    return hashlib.sha256(password.encode()).hexdigest()

# ==================== AUTH ====================

def create_token(user_id, username, is_admin):
    return jwt.encode({
        'user_id': user_id,
        'username': username,
        'is_admin': is_admin,
        'exp': time.time() + 86400 * 30
    }, JWT_SECRET, algorithm="HS256")

def token_required(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        token = request.headers.get('Authorization', '').replace('Bearer ', '')
        if not token:
            return jsonify({"error": "Token required"}), 401
        try:
            data = jwt.decode(token, JWT_SECRET, algorithms=["HS256"])
            request.current_user = data
        except:
            return jsonify({"error": "Invalid token"}), 401
        return f(*args, **kwargs)
    return decorated

def web_login_required(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        if 'user_id' not in session:
            return redirect(url_for('web_login'))
        return f(*args, **kwargs)
    return decorated

# ==================== STATIC FILES ====================

@app.route('/static/<path:filename>')
def serve_static(filename):
    return send_from_directory(STATIC_DIR, filename)

# ==================== TEMPLATES ====================

BASE_HTML = '''
<!DOCTYPE html>
<html lang="ru" class="light">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ title }} - RustDesk Panel</title>
    <link href="/static/output.css" rel="stylesheet">
    <link href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.1/font/bootstrap-icons.css" rel="stylesheet">
</head>
<body class="min-h-screen bg-gray-100 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
    <!-- Sidebar -->
    <nav class="sidebar" id="sidebar">
        <div class="sidebar-header">
            <a href="/" class="sidebar-brand">
                <i class="bi bi-display"></i>
                RustDesk Panel
            </a>
        </div>
        <ul class="sidebar-nav">
            <li class="nav-item">
                <a class="nav-link {{ 'active' if active_page == 'dashboard' else '' }}" href="{{ url_for('web_dashboard') }}">
                    <i class="bi bi-speedometer2"></i>
                    Dashboard
                </a>
            </li>
            <li class="nav-item">
                <a class="nav-link {{ 'active' if active_page == 'devices' else '' }}" href="{{ url_for('web_devices') }}">
                    <i class="bi bi-pc-display"></i>
                    Devices
                </a>
            </li>
            <li class="nav-item">
                <a class="nav-link {{ 'active' if active_page == 'users' else '' }}" href="{{ url_for('web_users') }}">
                    <i class="bi bi-people"></i>
                    Users
                </a>
            </li>
            <li class="nav-item">
                <a class="nav-link {{ 'active' if active_page == 'logs' else '' }}" href="{{ url_for('web_logs') }}">
                    <i class="bi bi-journal-text"></i>
                    Logs
                </a>
            </li>
            <li class="nav-item">
                <a class="nav-link {{ 'active' if active_page == 'settings' else '' }}" href="{{ url_for('web_settings') }}">
                    <i class="bi bi-gear"></i>
                    Settings
                </a>
            </li>
        </ul>
        <div class="mt-auto p-4 border-t border-gray-200 dark:border-gray-700">
            <small class="text-gray-500 dark:text-gray-400">RustDesk Panel v2.0</small>
        </div>
    </nav>

    <!-- Main Content -->
    <main class="main-content">
        <div class="top-navbar">
            <button class="lg:hidden p-2 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg" onclick="toggleSidebar()">
                <i class="bi bi-list text-2xl"></i>
            </button>
            <div class="flex items-center gap-3">
                <span class="text-gray-500 dark:text-gray-400">{{ current_time }}</span>
            </div>
            <div class="flex items-center gap-3">
                <button class="p-2 text-gray-600 dark:text-gray-300 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-lg" onclick="toggleTheme()" title="Toggle Theme">
                    <i class="bi bi-moon-stars text-xl" id="themeIcon"></i>
                </button>
                <div class="relative">
                    <button class="user-dropdown flex items-center gap-2" onclick="toggleDropdown('userDropdown')">
                        <i class="bi bi-person-circle"></i>
                        {{ session.username }}
                        <i class="bi bi-chevron-down text-xs"></i>
                    </button>
                    <div id="userDropdown" class="dropdown-menu">
                        <a class="dropdown-item" href="{{ url_for('web_logout') }}">
                            <i class="bi bi-box-arrow-right mr-2"></i>Logout
                        </a>
                    </div>
                </div>
            </div>
        </div>

        <div class="content-area">
            {% block content %}{% endblock %}
        </div>
    </main>

    <script src="https://code.jquery.com/jquery-3.7.1.min.js"></script>
    <script src="https://cdn.datatables.net/1.13.7/js/jquery.dataTables.min.js"></script>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <script>
        // Theme toggle
        function toggleTheme() {
            const html = document.documentElement;
            const isDark = html.classList.contains('dark');
            
            if (isDark) {
                html.classList.remove('dark');
                localStorage.setItem('theme', 'light');
            } else {
                html.classList.add('dark');
                localStorage.setItem('theme', 'dark');
            }
            
            updateThemeIcon();
            
            // Reload for charts if they exist
            if (document.getElementById('connectionsChart') || document.getElementById('osChart')) {
                location.reload();
            }
        }

        function updateThemeIcon() {
            const icon = document.getElementById('themeIcon');
            const isDark = document.documentElement.classList.contains('dark');
            icon.className = isDark ? 'bi bi-sun text-xl' : 'bi bi-moon-stars text-xl';
        }

        // Init theme from localStorage
        const savedTheme = localStorage.getItem('theme') || 'light';
        if (savedTheme === 'dark') {
            document.documentElement.classList.add('dark');
        } else {
            document.documentElement.classList.remove('dark');
        }
        updateThemeIcon();

        // Sidebar toggle (mobile)
        function toggleSidebar() {
            document.getElementById('sidebar').classList.toggle('show');
        }

        // Dropdown toggle
        function toggleDropdown(id) {
            const dropdown = document.getElementById(id);
            dropdown.classList.toggle('show');
        }

        // Close dropdowns when clicking outside
        document.addEventListener('click', function(e) {
            if (!e.target.closest('.relative')) {
                document.querySelectorAll('.dropdown-menu.show').forEach(d => d.classList.remove('show'));
            }
        });

        // Modal functions
        function openModal(id) {
            document.getElementById(id).classList.add('show');
            document.body.style.overflow = 'hidden';
        }

        function closeModal(id) {
            document.getElementById(id).classList.remove('show');
            document.body.style.overflow = '';
        }

        // Close modal on backdrop click
        document.addEventListener('click', function(e) {
            if (e.target.classList.contains('modal-backdrop')) {
                e.target.classList.remove('show');
                document.body.style.overflow = '';
            }
        });

        // Close modal on Escape key
        document.addEventListener('keydown', function(e) {
            if (e.key === 'Escape') {
                document.querySelectorAll('.modal-backdrop.show').forEach(m => {
                    m.classList.remove('show');
                    document.body.style.overflow = '';
                });
            }
        });
    </script>
    {% block scripts %}{% endblock %}
</body>
</html>
'''

LOGIN_HTML = '''
<!DOCTYPE html>
<html lang="ru" class="light">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Login - RustDesk Panel</title>
    <link href="/static/output.css" rel="stylesheet">
    <link href="https://cdn.jsdelivr.net/npm/bootstrap-icons@1.11.1/font/bootstrap-icons.css" rel="stylesheet">
</head>
<body class="min-h-screen bg-gray-100 dark:bg-gray-900 flex items-center justify-center">
    <div class="login-card">
        <div class="text-center mb-6">
            <i class="bi bi-display login-logo"></i>
            <h3 class="mt-4 text-2xl font-bold text-gray-900 dark:text-white">RustDesk Panel</h3>
            <p class="text-gray-500 dark:text-gray-400 mt-2">Sign in to your account</p>
        </div>
        {% if error %}
        <div class="alert alert-danger">{{ error }}</div>
        {% endif %}
        <form method="POST">
            <div class="mb-4">
                <label class="form-label">Username</label>
                <div class="input-group">
                    <span class="input-group-text"><i class="bi bi-person"></i></span>
                    <input type="text" class="form-control rounded-l-none" name="username" required autofocus>
                </div>
            </div>
            <div class="mb-6">
                <label class="form-label">Password</label>
                <div class="input-group">
                    <span class="input-group-text"><i class="bi bi-lock"></i></span>
                    <input type="password" class="form-control rounded-l-none" name="password" required>
                </div>
            </div>
            <button type="submit" class="btn btn-primary w-full py-3">
                <i class="bi bi-box-arrow-in-right mr-2"></i>Sign In
            </button>
        </form>
    </div>
    <script>
        // Init theme
        const savedTheme = localStorage.getItem('theme') || 'light';
        if (savedTheme === 'dark') {
            document.documentElement.classList.add('dark');
        }
    </script>
</body>
</html>
'''

DASHBOARD_HTML = '''
{% extends "base" %}
{% block content %}
<h4 class="text-xl font-semibold text-gray-900 dark:text-white mb-6">Dashboard</h4>

<!-- Stats -->
<div class="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-4 gap-4 mb-6">
    <div class="stat-card">
        <div class="flex items-center">
            <div class="stat-icon bg-blue-100 dark:bg-blue-900/30 text-blue-600 dark:text-blue-400 mr-4">
                <i class="bi bi-pc-display"></i>
            </div>
            <div>
                <div class="stat-value">{{ stats.total }}</div>
                <div class="text-gray-500 dark:text-gray-400">Total Devices</div>
            </div>
        </div>
    </div>
    <div class="stat-card">
        <div class="flex items-center">
            <div class="stat-icon bg-green-100 dark:bg-green-900/30 text-green-600 dark:text-green-400 mr-4">
                <i class="bi bi-wifi"></i>
            </div>
            <div>
                <div class="stat-value">{{ stats.online }}</div>
                <div class="text-gray-500 dark:text-gray-400">Online Now</div>
            </div>
        </div>
    </div>
    <div class="stat-card">
        <div class="flex items-center">
            <div class="stat-icon bg-cyan-100 dark:bg-cyan-900/30 text-cyan-600 dark:text-cyan-400 mr-4">
                <i class="bi bi-arrow-left-right"></i>
            </div>
            <div>
                <div class="stat-value">{{ stats.connections_today }}</div>
                <div class="text-gray-500 dark:text-gray-400">Connections Today</div>
            </div>
        </div>
    </div>
    <div class="stat-card">
        <div class="flex items-center">
            <div class="stat-icon bg-amber-100 dark:bg-amber-900/30 text-amber-600 dark:text-amber-400 mr-4">
                <i class="bi bi-people"></i>
            </div>
            <div>
                <div class="stat-value">{{ stats.users }}</div>
                <div class="text-gray-500 dark:text-gray-400">Users</div>
            </div>
        </div>
    </div>
</div>

<!-- Charts -->
<div class="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-6">
    <div class="lg:col-span-2 card-custom">
        <div class="card-header flex justify-between items-center">
            <h6 class="font-semibold text-gray-900 dark:text-white">Connections (Last 7 Days)</h6>
        </div>
        <div class="card-body">
            <div class="chart-container">
                <canvas id="connectionsChart"></canvas>
            </div>
        </div>
    </div>
    <div class="card-custom">
        <div class="card-header">
            <h6 class="font-semibold text-gray-900 dark:text-white">OS Distribution</h6>
        </div>
        <div class="card-body">
            <div class="chart-container">
                <canvas id="osChart"></canvas>
            </div>
        </div>
    </div>
</div>

<!-- Recent Devices -->
<div class="card-custom">
    <div class="card-header flex justify-between items-center">
        <h6 class="font-semibold text-gray-900 dark:text-white">Recent Devices</h6>
        <a href="{{ url_for('web_devices') }}" class="btn btn-outline btn-sm">View All</a>
    </div>
    <div class="overflow-x-auto">
        <table class="w-full">
            <thead>
                <tr class="bg-gray-50 dark:bg-gray-800/50">
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">ID</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">Hostname</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">User</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">OS</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">IP</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">Status</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">Last Seen</th>
                    <th class="text-left px-4 py-3 font-semibold text-gray-700 dark:text-gray-300">Action</th>
                </tr>
            </thead>
            <tbody>
                {% for d in devices[:10] %}
                <tr class="border-b border-gray-100 dark:border-gray-800 hover:bg-gray-50 dark:hover:bg-gray-800/50">
                    <td class="px-4 py-3"><span class="device-id">{{ d.id }}</span></td>
                    <td class="px-4 py-3 text-gray-700 dark:text-gray-300">{{ d.hostname or '-' }}</td>
                    <td class="px-4 py-3 text-gray-700 dark:text-gray-300">{{ d.username or '-' }}</td>
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{{ d.os_short }}</td>
                    <td class="px-4 py-3 text-gray-700 dark:text-gray-300">{{ d.ip or '-' }}</td>
                    <td class="px-4 py-3">
                        <span class="{{ 'badge-online' if d.online else 'badge-offline' }}">
                            {{ 'Online' if d.online else 'Offline' }}
                        </span>
                    </td>
                    <td class="px-4 py-3 text-sm text-gray-500 dark:text-gray-400">{{ d.last_seen_str }}</td>
                    <td class="px-4 py-3">
                        <button class="btn btn-primary btn-connect" onclick="connectTo('{{ d.id }}')">
                            <i class="bi bi-link-45deg"></i> Connect
                        </button>
                    </td>
                </tr>
                {% endfor %}
            </tbody>
        </table>
    </div>
</div>
{% endblock %}

{% block scripts %}
<script>
function connectTo(id) {
    window.location.href = 'rustdesk://connection/new/' + id;
}

// Get theme colors
const isDark = document.documentElement.classList.contains('dark');
const gridColor = isDark ? '#374151' : '#e5e7eb';
const textColor = isDark ? '#9ca3af' : '#6b7280';

// Connections Chart
const connCtx = document.getElementById('connectionsChart').getContext('2d');
new Chart(connCtx, {
    type: 'line',
    data: {
        labels: {{ chart_labels | safe }},
        datasets: [{
            label: 'Connections',
            data: {{ chart_data | safe }},
            borderColor: '#0d6efd',
            backgroundColor: 'rgba(13, 110, 253, 0.1)',
            fill: true,
            tension: 0.4
        }]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { display: false } },
        scales: {
            y: { beginAtZero: true, grid: { color: gridColor }, ticks: { color: textColor } },
            x: { grid: { color: gridColor }, ticks: { color: textColor } }
        }
    }
});

// OS Chart
const osCtx = document.getElementById('osChart').getContext('2d');
new Chart(osCtx, {
    type: 'doughnut',
    data: {
        labels: {{ os_labels | safe }},
        datasets: [{
            data: {{ os_data | safe }},
            backgroundColor: ['#0d6efd', '#10b981', '#f59e0b', '#ef4444', '#6b7280']
        }]
    },
    options: {
        responsive: true,
        maintainAspectRatio: false,
        plugins: { legend: { position: 'bottom', labels: { color: textColor } } }
    }
});
</script>
{% endblock %}
'''

DEVICES_HTML = '''
{% extends "base" %}
{% block content %}
<div class="flex justify-between items-center mb-6">
    <h4 class="text-xl font-semibold text-gray-900 dark:text-white">Devices</h4>
    <button class="btn btn-primary" onclick="location.reload()">
        <i class="bi bi-arrow-clockwise mr-2"></i>Refresh
    </button>
</div>

<div class="card-custom">
    <div class="card-body">
        <table id="devicesTable" class="dataTable w-full">
            <thead>
                <tr>
                    <th>ID</th>
                    <th>Hostname</th>
                    <th>Username</th>
                    <th>OS</th>
                    <th>IP Address</th>
                    <th>Version</th>
                    <th>Status</th>
                    <th>Last Seen</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {% for d in devices %}
                <tr>
                    <td><span class="device-id">{{ d.id }}</span></td>
                    <td>{{ d.hostname or '-' }}</td>
                    <td>{{ d.username or '-' }}</td>
                    <td class="text-sm">{{ d.os_short }}</td>
                    <td>{{ d.ip or '-' }}</td>
                    <td>{{ d.version or '-' }}</td>
                    <td>
                        <span class="{{ 'badge-online' if d.online else 'badge-offline' }}">
                            {{ 'Online' if d.online else 'Offline' }}
                        </span>
                    </td>
                    <td class="text-sm">{{ d.last_seen_str }}</td>
                    <td>
                        <button class="btn btn-primary btn-sm" onclick="connectTo('{{ d.id }}')" title="Connect">
                            <i class="bi bi-link-45deg"></i>
                        </button>
                        <button class="btn btn-outline btn-sm" onclick="showDetails('{{ d.id }}')" title="Details">
                            <i class="bi bi-info-circle"></i>
                        </button>
                    </td>
                </tr>
                {% endfor %}
            </tbody>
        </table>
    </div>
</div>

<!-- Device Details Modal -->
<div class="modal-backdrop" id="detailsModal">
    <div class="modal">
        <div class="modal-header">
            <h5 class="modal-title">Device Details</h5>
            <button class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200" onclick="closeModal('detailsModal')">
                <i class="bi bi-x-lg"></i>
            </button>
        </div>
        <div class="modal-body" id="detailsBody">
        </div>
    </div>
</div>
{% endblock %}

{% block scripts %}
<script>
$(document).ready(function() {
    $('#devicesTable').DataTable({
        order: [[7, 'desc']],
        pageLength: 25,
        language: {
            search: "Search:",
            lengthMenu: "Show _MENU_ devices"
        }
    });
});

function connectTo(id) {
    window.location.href = 'rustdesk://connection/new/' + id;
}

const devices = {{ devices_json | safe }};

function showDetails(id) {
    const d = devices.find(x => x.id === id);
    if (!d) return;
    document.getElementById('detailsBody').innerHTML = `
        <table class="w-full text-sm">
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400 w-24">ID</th><td class="py-2"><code class="device-id">${d.id}</code></td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">Hostname</th><td class="py-2">${d.hostname || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">Username</th><td class="py-2">${d.username || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">OS</th><td class="py-2">${d.os || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">IP</th><td class="py-2">${d.ip || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">CPU</th><td class="py-2">${d.cpu || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">Memory</th><td class="py-2">${d.memory || '-'}</td></tr>
            <tr class="border-b border-gray-200 dark:border-gray-700"><th class="py-2 text-left text-gray-600 dark:text-gray-400">Version</th><td class="py-2">${d.version || '-'}</td></tr>
            <tr><th class="py-2 text-left text-gray-600 dark:text-gray-400">Last Seen</th><td class="py-2">${d.last_seen_str}</td></tr>
        </table>
        <button class="btn btn-primary w-full mt-4" onclick="connectTo('${d.id}')">
            <i class="bi bi-link-45deg mr-2"></i>Connect
        </button>
    `;
    openModal('detailsModal');
}
</script>
{% endblock %}
'''

USERS_HTML = '''
{% extends "base" %}
{% block content %}
<div class="flex justify-between items-center mb-6">
    <h4 class="text-xl font-semibold text-gray-900 dark:text-white">Users</h4>
    <button class="btn btn-primary" onclick="openModal('addUserModal')">
        <i class="bi bi-plus-lg mr-2"></i>Add User
    </button>
</div>

<div class="card-custom">
    <div class="card-body">
        <table id="usersTable" class="dataTable w-full">
            <thead>
                <tr>
                    <th>ID</th>
                    <th>Username</th>
                    <th>Email</th>
                    <th>Role</th>
                    <th>Status</th>
                    <th>Created</th>
                    <th>Actions</th>
                </tr>
            </thead>
            <tbody>
                {% for u in users %}
                <tr>
                    <td>{{ u.id }}</td>
                    <td><i class="bi bi-person-circle mr-2 text-gray-400"></i>{{ u.username }}</td>
                    <td>{{ u.email or '-' }}</td>
                    <td>
                        <span class="text-xs font-medium px-2.5 py-1 rounded {{ 'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400' if u.is_admin else 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }}">
                            {{ 'Admin' if u.is_admin else 'User' }}
                        </span>
                    </td>
                    <td>
                        <span class="text-xs font-medium px-2.5 py-1 rounded {{ 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' if u.status == 1 else 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }}">
                            {{ 'Active' if u.status == 1 else 'Disabled' }}
                        </span>
                    </td>
                    <td class="text-sm">{{ u.created_at }}</td>
                    <td>
                        <button class="btn btn-sm text-red-600 hover:bg-red-50 dark:hover:bg-red-900/20" onclick="deleteUser({{ u.id }})" {{ 'disabled' if u.username == 'admin' else '' }}>
                            <i class="bi bi-trash"></i>
                        </button>
                    </td>
                </tr>
                {% endfor %}
            </tbody>
        </table>
    </div>
</div>

<!-- Add User Modal -->
<div class="modal-backdrop" id="addUserModal">
    <div class="modal">
        <div class="modal-header">
            <h5 class="modal-title">Add User</h5>
            <button class="text-gray-400 hover:text-gray-600 dark:hover:text-gray-200" onclick="closeModal('addUserModal')">
                <i class="bi bi-x-lg"></i>
            </button>
        </div>
        <form action="{{ url_for('web_add_user') }}" method="POST">
            <div class="modal-body">
                <div class="mb-4">
                    <label class="form-label">Username</label>
                    <input type="text" class="form-control" name="username" required>
                </div>
                <div class="mb-4">
                    <label class="form-label">Email</label>
                    <input type="email" class="form-control" name="email">
                </div>
                <div class="mb-4">
                    <label class="form-label">Password</label>
                    <input type="password" class="form-control" name="password" required>
                </div>
                <div class="form-check">
                    <input type="checkbox" class="form-check-input" name="is_admin" id="isAdmin">
                    <label class="form-check-label" for="isAdmin">Administrator</label>
                </div>
            </div>
            <div class="modal-footer">
                <button type="button" class="btn btn-secondary" onclick="closeModal('addUserModal')">Cancel</button>
                <button type="submit" class="btn btn-primary">Add User</button>
            </div>
        </form>
    </div>
</div>
{% endblock %}

{% block scripts %}
<script>
$(document).ready(function() {
    $('#usersTable').DataTable();
});

function deleteUser(id) {
    if (confirm('Delete this user?')) {
        fetch('/api/admin/users/' + id, { method: 'DELETE' })
            .then(() => location.reload());
    }
}
</script>
{% endblock %}
'''

LOGS_HTML = '''
{% extends "base" %}
{% block content %}
<div class="flex justify-between items-center mb-6">
    <h4 class="text-xl font-semibold text-gray-900 dark:text-white">Audit Logs</h4>
    <div class="flex gap-1">
        <button class="btn {{ 'btn-primary' if log_type == 'all' else 'btn-outline' }} btn-sm" onclick="location.href='?type=all'">All</button>
        <button class="btn {{ 'btn-primary' if log_type == 'conn' else 'btn-outline' }} btn-sm" onclick="location.href='?type=conn'">Connections</button>
        <button class="btn {{ 'btn-primary' if log_type == 'file' else 'btn-outline' }} btn-sm" onclick="location.href='?type=file'">Files</button>
        <button class="btn {{ 'btn-primary' if log_type == 'alarm' else 'btn-outline' }} btn-sm" onclick="location.href='?type=alarm'">Alarms</button>
    </div>
</div>

<div class="card-custom">
    <div class="card-body">
        <table id="logsTable" class="dataTable w-full">
            <thead>
                <tr>
                    <th>Time</th>
                    <th>Type</th>
                    <th>Device ID</th>
                    <th>Peer ID</th>
                    <th>Action</th>
                </tr>
            </thead>
            <tbody>
                {% for log in logs %}
                <tr>
                    <td class="text-sm">{{ log.created_at }}</td>
                    <td>
                        <span class="text-xs font-medium px-2.5 py-1 rounded 
                            {{ 'bg-cyan-100 text-cyan-700 dark:bg-cyan-900/30 dark:text-cyan-400' if log.type == 'conn' else 
                               'bg-amber-100 text-amber-700 dark:bg-amber-900/30 dark:text-amber-400' if log.type == 'file' else 
                               'bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400' if log.type == 'alarm' else 
                               'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }}">
                            {{ log.type }}
                        </span>
                    </td>
                    <td><code class="text-sm text-gray-600 dark:text-gray-400">{{ log.device_id or '-' }}</code></td>
                    <td><code class="text-sm text-gray-600 dark:text-gray-400">{{ log.peer_id or '-' }}</code></td>
                    <td>{{ log.action or '-' }}</td>
                </tr>
                {% endfor %}
            </tbody>
        </table>
    </div>
</div>
{% endblock %}

{% block scripts %}
<script>
$(document).ready(function() {
    $('#logsTable').DataTable({
        order: [[0, 'desc']],
        pageLength: 50
    });
});
</script>
{% endblock %}
'''

SETTINGS_HTML = '''
{% extends "base" %}
{% block content %}
<h4 class="text-xl font-semibold text-gray-900 dark:text-white mb-6">Settings</h4>

<div class="grid grid-cols-1 lg:grid-cols-2 gap-6">
    <div class="space-y-6">
        <div class="card-custom">
            <div class="card-header">
                <h6 class="font-semibold text-gray-900 dark:text-white"><i class="bi bi-server mr-2"></i>Server Configuration</h6>
            </div>
            <div class="card-body">
                <div class="mb-4">
                    <label class="form-label">ID Server</label>
                    <input type="text" class="form-control bg-gray-50 dark:bg-gray-700" value="10.21.31.11" disabled>
                </div>
                <div class="mb-4">
                    <label class="form-label">Relay Server</label>
                    <input type="text" class="form-control bg-gray-50 dark:bg-gray-700" value="10.21.31.11" disabled>
                </div>
                <div>
                    <label class="form-label">API Server</label>
                    <input type="text" class="form-control bg-gray-50 dark:bg-gray-700" value="http://{{ request.host }}" disabled>
                </div>
            </div>
        </div>
        
        <div class="card-custom">
            <div class="card-header">
                <h6 class="font-semibold text-gray-900 dark:text-white"><i class="bi bi-info-circle mr-2"></i>System Info</h6>
            </div>
            <div class="card-body">
                <table class="w-full text-sm">
                    <tr class="border-b border-gray-200 dark:border-gray-700">
                        <td class="py-2 text-gray-600 dark:text-gray-400">LDAP Library</td>
                        <td class="py-2 text-right">
                            <span class="text-xs font-medium px-2.5 py-1 rounded {{ 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' if ldap_available else 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }}">
                                {{ 'Installed' if ldap_available else 'Not installed' }}
                            </span>
                        </td>
                    </tr>
                    <tr>
                        <td class="py-2 text-gray-600 dark:text-gray-400">LDAP Status</td>
                        <td class="py-2 text-right">
                            <span class="text-xs font-medium px-2.5 py-1 rounded {{ 'bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400' if ldap_config.get('enabled') else 'bg-gray-100 text-gray-700 dark:bg-gray-700 dark:text-gray-300' }}">
                                {{ 'Enabled' if ldap_config.get('enabled') else 'Disabled' }}
                            </span>
                        </td>
                    </tr>
                </table>
            </div>
        </div>
    </div>
    
    <div class="card-custom">
        <div class="card-header flex justify-between items-center">
            <h6 class="font-semibold text-gray-900 dark:text-white"><i class="bi bi-diagram-3 mr-2"></i>LDAP / Active Directory</h6>
            <button type="button" class="btn btn-outline btn-sm" onclick="testLdap()">
                <i class="bi bi-plug mr-1"></i>Test Connection
            </button>
        </div>
        <div class="card-body">
            <div id="ldapTestResult" class="alert hidden mb-4"></div>
            
            <form action="{{ url_for('web_save_ldap') }}" method="POST">
                <div class="mb-4">
                    <label class="form-label">LDAP Server URL</label>
                    <input type="text" class="form-control" name="ldap_server" placeholder="ldap://dc.example.com:389" value="{{ ldap_config.get('server', '') }}">
                    <small class="text-gray-500 dark:text-gray-400 text-xs mt-1 block">Example: ldap://192.168.1.10 or ldaps://dc.company.local</small>
                </div>
                <div class="mb-4">
                    <label class="form-label">Base DN</label>
                    <input type="text" class="form-control" name="ldap_base_dn" placeholder="DC=company,DC=local" value="{{ ldap_config.get('base_dn', '') }}">
                    <small class="text-gray-500 dark:text-gray-400 text-xs mt-1 block">The base distinguished name for user searches</small>
                </div>
                <div class="mb-4">
                    <label class="form-label">Bind DN (Admin Account)</label>
                    <input type="text" class="form-control" name="ldap_bind_dn" placeholder="CN=Administrator,CN=Users,DC=company,DC=local" value="{{ ldap_config.get('bind_dn', '') }}">
                    <small class="text-gray-500 dark:text-gray-400 text-xs mt-1 block">Leave empty for anonymous bind</small>
                </div>
                <div class="mb-4">
                    <label class="form-label">Bind Password</label>
                    <input type="password" class="form-control" name="ldap_bind_password" placeholder="••••••••">
                </div>
                <div class="form-check form-switch mb-4">
                    <input type="checkbox" class="form-check-input" name="ldap_enabled" id="ldapEnabled" {{ 'checked' if ldap_config.get('enabled') else '' }}>
                    <label class="form-check-label" for="ldapEnabled">Enable LDAP Authentication</label>
                </div>
                <button type="submit" class="btn btn-primary">
                    <i class="bi bi-save mr-1"></i>Save Settings
                </button>
            </form>
        </div>
    </div>
</div>
{% endblock %}

{% block scripts %}
<script>
function testLdap() {
    const resultDiv = document.getElementById('ldapTestResult');
    resultDiv.className = 'alert alert-info';
    resultDiv.innerHTML = '<i class="bi bi-hourglass-split mr-2"></i>Testing connection...';
    resultDiv.classList.remove('hidden');
    
    fetch('/api/ldap/test', { method: 'POST' })
        .then(r => r.json())
        .then(data => {
            if (data.success) {
                resultDiv.className = 'alert alert-success';
                resultDiv.innerHTML = '<i class="bi bi-check-circle mr-2"></i>' + data.message;
            } else {
                resultDiv.className = 'alert alert-danger';
                resultDiv.innerHTML = '<i class="bi bi-x-circle mr-2"></i>' + data.message;
                if (!data.ldap_available) {
                    resultDiv.innerHTML += '<br><small>Install ldap3: <code>pip install ldap3</code></small>';
                }
            }
        })
        .catch(err => {
            resultDiv.className = 'alert alert-danger';
            resultDiv.innerHTML = '<i class="bi bi-x-circle mr-2"></i>Connection test failed: ' + err;
        });
}
</script>
{% endblock %}
'''

# ==================== WEB ROUTES ====================

def render_page(template, **kwargs):
    kwargs['session'] = session
    kwargs['url_for'] = url_for
    kwargs['request'] = request
    kwargs['current_time'] = datetime.now().strftime('%Y-%m-%d %H:%M')
    full_template = BASE_HTML.replace('{% block content %}{% endblock %}', template.split('{% block content %}')[1].split('{% endblock %}')[0])
    full_template = full_template.replace('{% block scripts %}{% endblock %}', template.split('{% block scripts %}')[1].split('{% endblock %}')[0] if '{% block scripts %}' in template else '')
    return render_template_string(full_template, **kwargs)

@app.route('/')
def web_index():
    if 'user_id' in session:
        return redirect(url_for('web_dashboard'))
    return redirect(url_for('web_login'))

@app.route('/login', methods=['GET', 'POST'])
def web_login():
    error = None
    if request.method == 'POST':
        username = request.form.get('username')
        password = request.form.get('password')
        
        user = None
        conn = get_db()
        
        # Try local authentication first
        local_user = conn.execute("SELECT * FROM users WHERE username = ?", (username,)).fetchone()
        
        if local_user and local_user['password'] == hash_password(password) and local_user['status'] == 1:
            user = local_user
        
        # If local auth failed and LDAP is enabled, try LDAP
        if not user and is_ldap_enabled():
            ldap_user = ldap_authenticate(username, password)
            if ldap_user:
                # Sync LDAP user to local database
                is_admin = 'Domain Admins' in ldap_user.get('groups', []) or \
                          'Administrators' in ldap_user.get('groups', [])
                user_id = sync_ldap_user_to_db(ldap_user, is_admin)
                
                if user_id:
                    user = conn.execute("SELECT * FROM users WHERE id = ?", (user_id,)).fetchone()
                    print(f"[LDAP] User '{username}' authenticated via LDAP")
        
        conn.close()
        
        if user:
            session['user_id'] = user['id']
            session['username'] = user['username']
            session['is_admin'] = user['is_admin']
            return redirect(url_for('web_dashboard'))
        
        error = 'Invalid username or password'
    
    return render_template_string(LOGIN_HTML, error=error)

@app.route('/logout')
def web_logout():
    session.clear()
    return redirect(url_for('web_login'))

def get_devices_list():
    conn = get_db()
    devices = conn.execute("SELECT * FROM devices ORDER BY last_seen DESC").fetchall()
    conn.close()
    
    devices_list = []
    for d in devices:
        device = dict(d)
        # Short OS name
        os_full = device.get('os', '') or ''
        if 'Windows 11' in os_full:
            device['os_short'] = 'Windows 11'
        elif 'Windows 10' in os_full:
            device['os_short'] = 'Windows 10'
        elif 'Linux' in os_full:
            device['os_short'] = 'Linux'
        elif 'Mac' in os_full or 'Darwin' in os_full:
            device['os_short'] = 'macOS'
        else:
            device['os_short'] = os_full[:20] if os_full else '-'
        
        # Format last seen
        if device['last_seen']:
            try:
                dt = datetime.fromisoformat(device['last_seen'])
                device['last_seen_str'] = dt.strftime('%d.%m.%Y %H:%M')
            except:
                device['last_seen_str'] = str(device['last_seen'])
        else:
            device['last_seen_str'] = 'Never'
        devices_list.append(device)
    return devices_list

@app.route('/dashboard')
@web_login_required
def web_dashboard():
    conn = get_db()
    
    # Stats
    total = conn.execute("SELECT COUNT(*) FROM devices").fetchone()[0]
    online = conn.execute("SELECT COUNT(*) FROM devices WHERE online = 1").fetchone()[0]
    connections_today = conn.execute("SELECT COUNT(*) FROM connections WHERE date(started_at) = date('now')").fetchone()[0]
    users_count = conn.execute("SELECT COUNT(*) FROM users").fetchone()[0]
    
    # Chart data - last 7 days
    chart_labels = []
    chart_data = []
    for i in range(6, -1, -1):
        date = (datetime.now() - timedelta(days=i)).strftime('%Y-%m-%d')
        label = (datetime.now() - timedelta(days=i)).strftime('%d.%m')
        count = conn.execute("SELECT COUNT(*) FROM connections WHERE date(started_at) = ?", (date,)).fetchone()[0]
        chart_labels.append(label)
        chart_data.append(count)
    
    # OS distribution
    os_stats = {}
    devices = conn.execute("SELECT os FROM devices WHERE os IS NOT NULL AND os != ''").fetchall()
    for d in devices:
        os_name = d['os'] or ''
        if 'Windows 11' in os_name:
            key = 'Windows 11'
        elif 'Windows 10' in os_name:
            key = 'Windows 10'
        elif 'Linux' in os_name:
            key = 'Linux'
        elif 'Mac' in os_name or 'Darwin' in os_name:
            key = 'macOS'
        else:
            key = 'Other'
        os_stats[key] = os_stats.get(key, 0) + 1
    
    conn.close()
    
    devices_list = get_devices_list()
    
    return render_page(DASHBOARD_HTML,
        title='Dashboard',
        active_page='dashboard',
        stats={
            'total': total,
            'online': online,
            'connections_today': connections_today,
            'users': users_count
        },
        devices=devices_list,
        chart_labels=json.dumps(chart_labels),
        chart_data=json.dumps(chart_data),
        os_labels=json.dumps(list(os_stats.keys()) or ['No data']),
        os_data=json.dumps(list(os_stats.values()) or [1])
    )

@app.route('/devices')
@web_login_required
def web_devices():
    devices_list = get_devices_list()
    return render_page(DEVICES_HTML,
        title='Devices',
        active_page='devices',
        devices=devices_list,
        devices_json=json.dumps(devices_list)
    )

@app.route('/users')
@web_login_required
def web_users():
    conn = get_db()
    users = conn.execute("SELECT * FROM users ORDER BY id").fetchall()
    conn.close()
    
    return render_page(USERS_HTML,
        title='Users',
        active_page='users',
        users=users
    )

@app.route('/users/add', methods=['POST'])
@web_login_required
def web_add_user():
    username = request.form.get('username')
    email = request.form.get('email', '')
    password = request.form.get('password')
    is_admin = 1 if request.form.get('is_admin') else 0
    
    conn = get_db()
    try:
        conn.execute("INSERT INTO users (username, password, email, is_admin) VALUES (?, ?, ?, ?)",
                     (username, hash_password(password), email, is_admin))
        conn.commit()
    except sqlite3.IntegrityError:
        pass
    conn.close()
    
    return redirect(url_for('web_users'))

@app.route('/logs')
@web_login_required
def web_logs():
    log_type = request.args.get('type', 'all')
    
    conn = get_db()
    if log_type == 'all':
        logs = conn.execute("SELECT * FROM audit_logs ORDER BY created_at DESC LIMIT 500").fetchall()
    else:
        logs = conn.execute("SELECT * FROM audit_logs WHERE type = ? ORDER BY created_at DESC LIMIT 500", (log_type,)).fetchall()
    conn.close()
    
    return render_page(LOGS_HTML,
        title='Logs',
        active_page='logs',
        logs=logs,
        log_type=log_type
    )

@app.route('/settings')
@web_login_required
def web_settings():
    conn = get_db()
    settings = {}
    for row in conn.execute("SELECT key, value FROM settings").fetchall():
        settings[row['key']] = row['value']
    conn.close()
    
    ldap_config = {
        'server': settings.get('ldap_server', ''),
        'base_dn': settings.get('ldap_base_dn', ''),
        'bind_dn': settings.get('ldap_bind_dn', ''),
        'enabled': settings.get('ldap_enabled', '') == '1'
    }
    
    return render_page(SETTINGS_HTML,
        title='Settings',
        active_page='settings',
        ldap_config=ldap_config,
        ldap_available=LDAP_AVAILABLE
    )

@app.route('/settings/ldap', methods=['POST'])
@web_login_required
def web_save_ldap():
    conn = get_db()
    settings = {
        'ldap_server': request.form.get('ldap_server', ''),
        'ldap_base_dn': request.form.get('ldap_base_dn', ''),
        'ldap_bind_dn': request.form.get('ldap_bind_dn', ''),
        'ldap_enabled': '1' if request.form.get('ldap_enabled') else '0'
    }
    
    password = request.form.get('ldap_bind_password', '')
    if password:
        settings['ldap_bind_password'] = password
    
    for key, value in settings.items():
        conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES (?, ?)", (key, value))
    
    conn.commit()
    conn.close()
    
    return redirect(url_for('web_settings'))

@app.route('/api/ldap/test', methods=['POST'])
@web_login_required
def api_ldap_test():
    """Test LDAP connection"""
    success, message = test_ldap_connection()
    return jsonify({
        "success": success,
        "message": message,
        "ldap_available": LDAP_AVAILABLE
    })

# ==================== API ROUTES ====================

@app.after_request
def add_cors(response):
    response.headers['Access-Control-Allow-Origin'] = '*'
    response.headers['Access-Control-Allow-Methods'] = 'GET, POST, DELETE, OPTIONS'
    response.headers['Access-Control-Allow-Headers'] = 'Content-Type, Authorization'
    return response

@app.route('/api/login-options', methods=['GET', 'OPTIONS'])
def api_login_options():
    return jsonify({"oidc": [], "2fa": False})

@app.route('/api/login', methods=['POST', 'OPTIONS'])
def api_login():
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    username = data.get('username', '')
    password = data.get('password', '')
    device_id = data.get('id', '')
    
    conn = get_db()
    user = conn.execute("SELECT * FROM users WHERE username = ?", (username,)).fetchone()
    
    if not user or user['password'] != hash_password(password):
        conn.close()
        return jsonify({"error": "Invalid credentials"})
    
    if user['status'] != 1:
        conn.close()
        return jsonify({"error": "User disabled"})
    
    if device_id:
        conn.execute("UPDATE devices SET user_id = ? WHERE id = ?", (user['id'], device_id))
        conn.commit()
    
    conn.close()
    
    token = create_token(user['id'], user['username'], user['is_admin'])
    
    return jsonify({
        "access_token": token,
        "type": "access_token",
        "user": {
            "name": user['username'],
            "email": user['email'],
            "status": user['status'],
            "is_admin": bool(user['is_admin']),
            "info": {}
        }
    })

@app.route('/api/logout', methods=['POST', 'OPTIONS'])
@token_required
def api_logout():
    return jsonify({"success": True})

@app.route('/api/currentUser', methods=['POST', 'OPTIONS'])
@token_required
def api_current_user():
    conn = get_db()
    user = conn.execute("SELECT * FROM users WHERE id = ?", (request.current_user['user_id'],)).fetchone()
    conn.close()
    
    if not user:
        return jsonify({"error": "User not found"})
    
    return jsonify({
        "name": user['username'],
        "email": user['email'],
        "status": user['status'],
        "is_admin": bool(user['is_admin'])
    })

@app.route('/api/ab/get', methods=['GET', 'POST', 'OPTIONS'])
@token_required
def api_get_ab():
    conn = get_db()
    ab = conn.execute("SELECT * FROM address_books WHERE user_id = ?", (request.current_user['user_id'],)).fetchone()
    conn.close()
    
    data = ab['data'] if ab else '{"tags":[],"peers":[]}'
    return jsonify({"updated_at": int(time.time()), "data": data})

@app.route('/api/ab', methods=['GET', 'POST', 'OPTIONS'])
@token_required
def api_ab():
    """Address Book - GET to retrieve, POST to update"""
    conn = get_db()
    
    if request.method == 'GET':
        # Get address book
        ab = conn.execute("SELECT * FROM address_books WHERE user_id = ?", 
                          (request.current_user['user_id'],)).fetchone()
        conn.close()
        data = ab['data'] if ab else '{"tags":[],"peers":[]}'
        return jsonify({"updated_at": int(time.time()), "data": data})
    
    else:
        # POST - Update address book
        data = request.json or {}
        ab_data = data.get('data', '')
        
        conn.execute("INSERT OR REPLACE INTO address_books (user_id, data, updated_at) VALUES (?, ?, datetime('now'))",
                     (request.current_user['user_id'], ab_data))
        conn.commit()
        conn.close()
        
        return jsonify({"success": True})

@app.route('/api/heartbeat', methods=['POST', 'OPTIONS'])
def api_heartbeat():
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    device_id = data.get('id', '')
    uuid = data.get('uuid', '')
    
    if device_id:
        conn = get_db()
        conn.execute('''INSERT INTO devices (id, uuid, online, last_seen) VALUES (?, ?, 1, datetime('now'))
                        ON CONFLICT(id) DO UPDATE SET uuid = excluded.uuid, online = 1, last_seen = datetime('now')''',
                     (device_id, uuid))
        conn.execute("UPDATE devices SET online = 0 WHERE datetime(last_seen) < datetime('now', '-60 seconds')")
        conn.commit()
        conn.close()
    
    return jsonify({"modified_at": int(time.time())})

@app.route('/api/sysinfo', methods=['POST', 'OPTIONS'])
def api_sysinfo():
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    device_id = data.get('id', '')
    
    if not device_id:
        return 'ID_NOT_FOUND', 200
    
    client_ip = data.get('ip', '') or request.remote_addr
    
    conn = get_db()
    conn.execute('''INSERT INTO devices (id, uuid, hostname, os, username, version, cpu, memory, ip, online, last_seen)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, datetime('now'))
                    ON CONFLICT(id) DO UPDATE SET
                    uuid = excluded.uuid, hostname = excluded.hostname, os = excluded.os,
                    username = excluded.username, version = excluded.version, cpu = excluded.cpu,
                    memory = excluded.memory, ip = excluded.ip, online = 1, last_seen = datetime('now')''',
                 (device_id, data.get('uuid', ''), data.get('hostname', ''), data.get('os', ''),
                  data.get('username', ''), data.get('version', ''), data.get('cpu', ''),
                  data.get('memory', ''), client_ip))
    conn.commit()
    conn.close()
    
    print(f"[SYSINFO] {device_id} | {data.get('hostname', '')} | {data.get('username', '')} | {client_ip}")
    return 'SYSINFO_UPDATED', 200

@app.route('/api/sysinfo_ver', methods=['POST'])
def api_sysinfo_ver():
    return '1', 200

@app.route('/api/audit/<typ>', methods=['POST', 'OPTIONS'])
def api_audit(typ):
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    
    conn = get_db()
    conn.execute("INSERT INTO audit_logs (type, device_id, peer_id, action, data) VALUES (?, ?, ?, ?, ?)",
                 (typ, data.get('id', ''), data.get('peer_id', ''), data.get('action', ''), json.dumps(data)))
    
    if typ == 'conn':
        conn.execute("INSERT INTO connections (device_id, peer_id, conn_type) VALUES (?, ?, ?)",
                     (data.get('id', ''), data.get('peer_id', ''), data.get('type', '')))
    
    conn.commit()
    conn.close()
    
    print(f"[AUDIT:{typ}] {data}")
    return jsonify({"success": True})

@app.route('/api/admin/devices', methods=['GET'])
@token_required
def api_admin_devices():
    if not request.current_user.get('is_admin'):
        return jsonify({"error": "Access denied"}), 403
    
    conn = get_db()
    devices = conn.execute("SELECT * FROM devices ORDER BY last_seen DESC").fetchall()
    conn.close()
    
    return jsonify({"devices": [dict(d) for d in devices]})

@app.route('/api/admin/users/<int:user_id>', methods=['DELETE'])
@token_required
def api_delete_user(user_id):
    if not request.current_user.get('is_admin'):
        return jsonify({"error": "Access denied"}), 403
    
    conn = get_db()
    conn.execute("DELETE FROM users WHERE id = ? AND username != 'admin'", (user_id,))
    conn.commit()
    conn.close()
    
    return jsonify({"success": True})

@app.route('/api/stats/connections', methods=['GET'])
def api_stats_connections():
    conn = get_db()
    data = []
    for i in range(6, -1, -1):
        date = (datetime.now() - timedelta(days=i)).strftime('%Y-%m-%d')
        count = conn.execute("SELECT COUNT(*) FROM connections WHERE date(started_at) = ?", (date,)).fetchone()[0]
        data.append({"date": date, "count": count})
    conn.close()
    return jsonify(data)

# ==================== MAIN ====================

if __name__ == '__main__':
    init_db()
    
    # Check SSL certificates
    ssl_context = None
    protocol = "http"
    if SSL_ENABLED and os.path.exists(SSL_CERT) and os.path.exists(SSL_KEY):
        ssl_context = (SSL_CERT, SSL_KEY)
        protocol = "https"
        ssl_status = "ENABLED"
    else:
        ssl_status = "DISABLED (certificates not found)"
    
    print(f"""
╔═══════════════════════════════════════════════════════════════════╗
║          RustDesk Web Management Panel v2.0 (Tailwind)            ║
╠═══════════════════════════════════════════════════════════════════╣
║  Web Panel:  {protocol}://{HOST}:{PORT}                                ║
║  API:        {protocol}://{HOST}:{PORT}/api/                           ║
║  Login:      admin / admin123                                     ║
║  SSL:        {ssl_status}                                  ║
╠═══════════════════════════════════════════════════════════════════╣
║  NOTE: Run 'npm run build' first to compile Tailwind CSS!         ║
╚═══════════════════════════════════════════════════════════════════╝
    """)
    app.run(host=HOST, port=PORT, debug=True, threaded=True, ssl_context=ssl_context)
