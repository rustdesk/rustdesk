#!/usr/bin/env python3
"""
RustDesk API Server
Минимальная реализация для работы с RustDesk клиентом

Запуск: python api_server.py
Требования: pip install flask pyjwt
"""

from flask import Flask, request, jsonify, make_response
from functools import wraps
import jwt
import json
import time
import hashlib
import os

app = Flask(__name__)

# Конфигурация
SECRET_KEY = os.environ.get('SECRET_KEY', 'rustdesk-api-secret-key-change-me')
HOST = os.environ.get('API_HOST', '0.0.0.0')
PORT = int(os.environ.get('API_PORT', 21114))

# База данных в памяти (замените на реальную БД)
users_db = {
    "admin": {
        "password": hashlib.sha256("admin123".encode()).hexdigest(),
        "email": "admin@example.com",
        "is_admin": True,
        "status": 1
    },
    "user": {
        "password": hashlib.sha256("user123".encode()).hexdigest(),
        "email": "user@example.com",
        "is_admin": False,
        "status": 1
    }
}

address_books_db = {}
devices_db = {}
audit_log = []


def hash_password(password):
    return hashlib.sha256(password.encode()).hexdigest()


def create_token(username):
    return jwt.encode({
        'username': username,
        'exp': time.time() + 86400 * 30  # 30 дней
    }, SECRET_KEY, algorithm="HS256")


def token_required(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        auth_header = request.headers.get('Authorization', '')
        token = auth_header.replace('Bearer ', '') if auth_header else ''
        
        if not token:
            return jsonify({"error": "Token required"}), 401
        
        try:
            data = jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
            if data.get('exp', 0) < time.time():
                return jsonify({"error": "Token expired"}), 401
            request.current_user = data
        except jwt.InvalidTokenError:
            return jsonify({"error": "Invalid token"}), 401
        
        return f(*args, **kwargs)
    return decorated


def add_cors_headers(response):
    response.headers['Access-Control-Allow-Origin'] = '*'
    response.headers['Access-Control-Allow-Methods'] = 'GET, POST, OPTIONS'
    response.headers['Access-Control-Allow-Headers'] = 'Content-Type, Authorization'
    return response


@app.after_request
def after_request(response):
    return add_cors_headers(response)


@app.route('/api/login-options', methods=['GET', 'OPTIONS'])
def login_options():
    """Опции авторизации"""
    if request.method == 'OPTIONS':
        return '', 200
    return jsonify({
        "oidc": [],  # Список OIDC провайдеров: ["google", "github", "azure"]
        "2fa": False
    })


@app.route('/api/login', methods=['POST', 'OPTIONS'])
def login():
    """Авторизация пользователя"""
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    username = data.get('username', '')
    password = data.get('password', '')
    device_id = data.get('id', '')
    uuid = data.get('uuid', '')
    device_info = data.get('deviceInfo', {})
    
    print(f"[LOGIN] User: {username}, Device ID: {device_id}")
    
    if username not in users_db:
        return jsonify({"error": "User not found"})
    
    user = users_db[username]
    if user['password'] != hash_password(password):
        return jsonify({"error": "Invalid password"})
    
    if user['status'] != 1:
        return jsonify({"error": "User disabled"})
    
    token = create_token(username)
    
    # Сохраняем информацию об устройстве
    if device_id:
        devices_db[device_id] = {
            "uuid": uuid,
            "user": username,
            "device_info": device_info,
            "last_login": time.time()
        }
    
    return jsonify({
        "access_token": token,
        "type": "access_token",
        "tfa_type": "",
        "secret": "",
        "user": {
            "name": username,
            "email": user['email'],
            "status": user['status'],
            "is_admin": user['is_admin'],
            "info": {
                "email_verification": False,
                "email_alarm_notification": False
            }
        }
    })


@app.route('/api/logout', methods=['POST', 'OPTIONS'])
@token_required
def logout():
    """Выход из системы"""
    if request.method == 'OPTIONS':
        return '', 200
    
    username = request.current_user.get('username', '')
    print(f"[LOGOUT] User: {username}")
    return jsonify({"success": True})


@app.route('/api/currentUser', methods=['POST', 'OPTIONS'])
@token_required
def current_user():
    """Получить текущего пользователя"""
    if request.method == 'OPTIONS':
        return '', 200
    
    username = request.current_user.get('username', '')
    if username not in users_db:
        return jsonify({"error": "User not found"})
    
    user = users_db[username]
    return jsonify({
        "name": username,
        "email": user['email'],
        "status": user['status'],
        "is_admin": user['is_admin'],
        "info": {
            "email_verification": False,
            "email_alarm_notification": False
        }
    })


@app.route('/api/ab/get', methods=['POST', 'OPTIONS'])
@token_required
def get_address_book():
    """Получить адресную книгу"""
    if request.method == 'OPTIONS':
        return '', 200
    
    username = request.current_user.get('username', '')
    ab = address_books_db.get(username, {"tags": [], "peers": []})
    
    return jsonify({
        "updated_at": int(time.time()),
        "data": json.dumps(ab)
    })


@app.route('/api/ab', methods=['POST', 'OPTIONS'])
@token_required
def update_address_book():
    """Обновить адресную книгу"""
    if request.method == 'OPTIONS':
        return '', 200
    
    username = request.current_user.get('username', '')
    data = request.json or {}
    ab_data = data.get('data', '')
    
    if ab_data:
        try:
            address_books_db[username] = json.loads(ab_data)
            print(f"[AB] Updated for user: {username}")
        except json.JSONDecodeError:
            return jsonify({"error": "Invalid JSON"})
    
    return jsonify({"success": True})


@app.route('/api/heartbeat', methods=['POST', 'OPTIONS'])
def heartbeat():
    """Heartbeat от клиента"""
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    device_id = data.get('id', '')
    uuid = data.get('uuid', '')
    ver = data.get('ver', 0)
    conns = data.get('conns', [])
    modified_at = data.get('modified_at', 0)
    
    if device_id:
        devices_db[device_id] = {
            **devices_db.get(device_id, {}),
            "uuid": uuid,
            "ver": ver,
            "conns": conns,
            "last_heartbeat": time.time()
        }
    
    # Можно отправить стратегию настроек
    response = {
        "modified_at": int(time.time())
    }
    
    # Пример: принудительное отключение
    # response["disconnect"] = [1, 2, 3]
    
    # Пример: обновление настроек
    # response["strategy"] = {
    #     "config_options": {
    #         "allow-auto-disconnect": "Y"
    #     }
    # }
    
    return jsonify(response)


@app.route('/api/sysinfo', methods=['POST', 'OPTIONS'])
def sysinfo():
    """Информация о системе клиента"""
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    device_id = data.get('id', '')
    
    if not device_id:
        return make_response("ID_NOT_FOUND", 200)
    
    devices_db[device_id] = {
        **devices_db.get(device_id, {}),
        "uuid": data.get('uuid', ''),
        "version": data.get('version', ''),
        "hostname": data.get('hostname', ''),
        "os": data.get('os', ''),
        "username": data.get('username', ''),
        "cpu": data.get('cpu', ''),
        "memory": data.get('memory', ''),
        "last_sysinfo": time.time()
    }
    
    print(f"[SYSINFO] Device: {device_id}, Hostname: {data.get('hostname', '')}")
    return make_response("SYSINFO_UPDATED", 200)


@app.route('/api/sysinfo_ver', methods=['POST', 'OPTIONS'])
def sysinfo_ver():
    """Версия sysinfo"""
    if request.method == 'OPTIONS':
        return '', 200
    return make_response("1", 200)


@app.route('/api/audit/<typ>', methods=['POST', 'OPTIONS'])
def audit(typ):
    """Аудит действий (conn, file, alarm)"""
    if request.method == 'OPTIONS':
        return '', 200
    
    data = request.json or {}
    log_entry = {
        "type": typ,
        "timestamp": time.time(),
        "data": data
    }
    audit_log.append(log_entry)
    print(f"[AUDIT:{typ}] {data}")
    
    return jsonify({"success": True})


@app.route('/api/record', methods=['POST', 'OPTIONS'])
def record():
    """Загрузка записей сессий"""
    if request.method == 'OPTIONS':
        return '', 200
    
    # Здесь можно сохранять записи сессий
    print(f"[RECORD] Received recording data")
    return jsonify({"success": True})


# ==================== ADMIN API ====================

@app.route('/api/admin/users', methods=['GET'])
@token_required
def admin_list_users():
    """Список пользователей (только для админов)"""
    username = request.current_user.get('username', '')
    if not users_db.get(username, {}).get('is_admin'):
        return jsonify({"error": "Access denied"}), 403
    
    users_list = []
    for name, data in users_db.items():
        users_list.append({
            "name": name,
            "email": data['email'],
            "is_admin": data['is_admin'],
            "status": data['status']
        })
    
    return jsonify({"users": users_list})


@app.route('/api/admin/devices', methods=['GET'])
@token_required
def admin_list_devices():
    """Список устройств (только для админов)"""
    username = request.current_user.get('username', '')
    if not users_db.get(username, {}).get('is_admin'):
        return jsonify({"error": "Access denied"}), 403
    
    devices_list = []
    for device_id, data in devices_db.items():
        devices_list.append({
            "id": device_id,
            **data
        })
    
    return jsonify({"devices": devices_list})


@app.route('/api/admin/audit', methods=['GET'])
@token_required
def admin_audit_log():
    """Аудит лог (только для админов)"""
    username = request.current_user.get('username', '')
    if not users_db.get(username, {}).get('is_admin'):
        return jsonify({"error": "Access denied"}), 403
    
    return jsonify({"logs": audit_log[-100:]})  # Последние 100 записей


# ==================== MAIN ====================

if __name__ == '__main__':
    print(f"""
╔══════════════════════════════════════════════════════════════╗
║              🔌 RustDesk API Server                          ║
╠══════════════════════════════════════════════════════════════╣
║  Server: http://{HOST}:{PORT}                               ║
║                                                              ║
║  Default users:                                              ║
║    admin / admin123 (administrator)                          ║
║    user  / user123  (regular user)                           ║
║                                                              ║
║  Configure in RustDesk:                                      ║
║    API Server: http://YOUR_IP:{PORT}                        ║
╚══════════════════════════════════════════════════════════════╝
    """)
    
    app.run(host=HOST, port=PORT, debug=True)






