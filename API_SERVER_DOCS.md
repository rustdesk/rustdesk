# 🔌 RustDesk API Server Documentation

## Обзор

RustDesk клиент взаимодействует с API сервером для:
- Авторизации пользователей
- Синхронизации адресной книги
- Heartbeat (проверка связи)
- Аудита подключений
- OIDC авторизации

## 🔗 API Endpoints

### 1. `/api/login` - Авторизация

**Method:** `POST`

**Request:**
```json
{
    "username": "user@example.com",
    "password": "password123",
    "id": "123456789",           // RustDesk ID клиента
    "uuid": "device-uuid",       // UUID устройства
    "type": "account",           // Тип авторизации
    "deviceInfo": {
        "os": "Windows",
        "type": "client",
        "name": "Desktop-PC"
    }
}
```

**Response (успех):**
```json
{
    "access_token": "eyJhbGciOiJIUzI1NiIs...",
    "type": "access_token",
    "user": {
        "name": "username",
        "email": "user@example.com",
        "status": 1,
        "is_admin": false,
        "info": {
            "email_verification": false,
            "email_alarm_notification": false
        }
    }
}
```

**Response (ошибка):**
```json
{
    "error": "Invalid credentials"
}
```

---

### 2. `/api/logout` - Выход

**Method:** `POST`

**Request:**
```json
{
    "id": "123456789",
    "uuid": "device-uuid"
}
```

**Headers:**
```
Authorization: Bearer <access_token>
```

---

### 3. `/api/currentUser` - Текущий пользователь

**Method:** `POST`

**Request:**
```json
{
    "id": "123456789",
    "uuid": "device-uuid"
}
```

**Headers:**
```
Authorization: Bearer <access_token>
```

**Response:**
```json
{
    "name": "username",
    "email": "user@example.com",
    "status": 1,
    "is_admin": false
}
```

---

### 4. `/api/ab/get` - Получить адресную книгу

**Method:** `POST`

**Headers:**
```
Authorization: Bearer <access_token>
```

**Response:**
```json
{
    "updated_at": 1699999999,
    "data": "{\"tags\":[\"work\",\"home\"],\"peers\":[{\"id\":\"123456789\",\"username\":\"PC1\",\"hostname\":\"Desktop\",\"alias\":\"Office PC\",\"tags\":[\"work\"]}]}"
}
```

---

### 5. `/api/ab` - Обновить адресную книгу

**Method:** `POST`

**Headers:**
```
Authorization: Bearer <access_token>
```

**Request:**
```json
{
    "data": "{\"tags\":[\"work\",\"home\"],\"peers\":[...]}"
}
```

---

### 6. `/api/heartbeat` - Heartbeat

**Method:** `POST`

**Request:**
```json
{
    "id": "123456789",
    "uuid": "base64-encoded-uuid",
    "ver": 1002003,
    "conns": [1, 2, 3],
    "modified_at": 1699999999
}
```

**Response:**
```json
{
    "modified_at": 1699999999,
    "strategy": {
        "config_options": {
            "allow-auto-disconnect": "Y"
        }
    }
}
```

---

### 7. `/api/sysinfo` - Информация о системе

**Method:** `POST`

**Request:**
```json
{
    "id": "123456789",
    "uuid": "base64-encoded-uuid",
    "version": "1.2.3",
    "os": "Windows 10",
    "hostname": "Desktop-PC",
    "username": "user",
    "cpu": "Intel i7",
    "memory": "16GB"
}
```

**Response:**
```
SYSINFO_UPDATED
```
или
```
ID_NOT_FOUND
```

---

### 8. `/api/audit/conn` - Аудит подключений

**Method:** `POST`

**Request:**
```json
{
    "action": "new",
    "id": "123456789",
    "uuid": "device-uuid",
    "peer_id": "987654321",
    "conn_id": 1
}
```

---

### 9. `/api/login-options` - Опции авторизации

**Method:** `GET`

**Response:**
```json
{
    "oidc": ["google", "github", "azure"],
    "2fa": true
}
```

---

### 10. `/api/oidc/auth` - OIDC авторизация

**Method:** `POST`

**Request:**
```json
{
    "op": "google",
    "id": "123456789",
    "uuid": "device-uuid",
    "deviceInfo": {...}
}
```

**Response:**
```json
{
    "code": "auth-code-123",
    "url": "https://accounts.google.com/oauth2/..."
}
```

---

### 11. `/api/oidc/auth-query` - Проверка OIDC

**Method:** `GET`

**Query params:** `?code=auth-code-123&id=123456789&uuid=device-uuid`

**Response (ожидание):**
```json
{
    "error": "No authed oidc is found"
}
```

**Response (успех):**
```json
{
    "access_token": "...",
    "type": "access_token",
    "user": {...}
}
```

---

## 📊 Структуры данных

### UserStatus (enum)
```
0  = Disabled
1  = Normal
-1 = Unverified
```

### DeviceInfo
```json
{
    "os": "Windows",      // Linux, Windows, Android, iOS, macOS
    "type": "client",     // client или browser
    "name": "Device Name"
}
```

### AddressBook
```json
{
    "tags": ["work", "home", "servers"],
    "peers": [
        {
            "id": "123456789",
            "username": "admin",
            "hostname": "Server-1",
            "platform": "Windows",
            "alias": "Main Server",
            "tags": ["work", "servers"],
            "hash": "password-hash"
        }
    ]
}
```

---

## 🔒 Авторизация

Все защищённые endpoints требуют заголовок:
```
Authorization: Bearer <access_token>
```

---

## 🛠️ Минимальный API сервер (Python)

```python
from flask import Flask, request, jsonify
from functools import wraps
import jwt
import json
import time

app = Flask(__name__)
SECRET_KEY = "your-secret-key"

# Простая база данных в памяти
users = {
    "admin": {"password": "admin123", "email": "admin@example.com"}
}
address_books = {}
devices = {}

def token_required(f):
    @wraps(f)
    def decorated(*args, **kwargs):
        token = request.headers.get('Authorization', '').replace('Bearer ', '')
        if not token:
            return jsonify({"error": "Token required"}), 401
        try:
            data = jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
            request.user = data
        except:
            return jsonify({"error": "Invalid token"}), 401
        return f(*args, **kwargs)
    return decorated

@app.route('/api/login', methods=['POST'])
def login():
    data = request.json
    username = data.get('username')
    password = data.get('password')
    
    if username in users and users[username]['password'] == password:
        token = jwt.encode({
            'username': username,
            'exp': time.time() + 86400 * 30
        }, SECRET_KEY, algorithm="HS256")
        
        return jsonify({
            "access_token": token,
            "type": "access_token",
            "user": {
                "name": username,
                "email": users[username]['email'],
                "status": 1,
                "is_admin": username == "admin",
                "info": {}
            }
        })
    
    return jsonify({"error": "Invalid credentials"})

@app.route('/api/logout', methods=['POST'])
@token_required
def logout():
    return jsonify({"success": True})

@app.route('/api/currentUser', methods=['POST'])
@token_required
def current_user():
    username = request.user['username']
    return jsonify({
        "name": username,
        "email": users[username]['email'],
        "status": 1,
        "is_admin": username == "admin"
    })

@app.route('/api/ab/get', methods=['POST'])
@token_required
def get_address_book():
    username = request.user['username']
    ab = address_books.get(username, {"tags": [], "peers": []})
    return jsonify({
        "updated_at": int(time.time()),
        "data": json.dumps(ab)
    })

@app.route('/api/ab', methods=['POST'])
@token_required
def update_address_book():
    username = request.user['username']
    data = request.json.get('data')
    if data:
        address_books[username] = json.loads(data)
    return jsonify({"success": True})

@app.route('/api/heartbeat', methods=['POST'])
def heartbeat():
    data = request.json
    device_id = data.get('id')
    devices[device_id] = {
        "uuid": data.get('uuid'),
        "ver": data.get('ver'),
        "last_seen": time.time()
    }
    return jsonify({"modified_at": int(time.time())})

@app.route('/api/sysinfo', methods=['POST'])
def sysinfo():
    data = request.json
    device_id = data.get('id')
    if device_id:
        devices[device_id] = {
            **devices.get(device_id, {}),
            "hostname": data.get('hostname'),
            "os": data.get('os'),
            "username": data.get('username'),
            "version": data.get('version')
        }
        return "SYSINFO_UPDATED"
    return "ID_NOT_FOUND"

@app.route('/api/audit/<typ>', methods=['POST'])
def audit(typ):
    # Логирование аудита
    data = request.json
    print(f"AUDIT [{typ}]: {data}")
    return jsonify({"success": True})

@app.route('/api/login-options', methods=['GET'])
def login_options():
    return jsonify({
        "oidc": [],  # ["google", "github"] если настроено
        "2fa": False
    })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=21114, debug=True)
```

---

## 🚀 Запуск

```bash
pip install flask pyjwt
python api_server.py
```

Сервер запустится на `http://0.0.0.0:21114`

---

## ⚙️ Настройка RustDesk клиента

В настройках RustDesk укажите:
- **API Server:** `http://10.21.31.11:21114`

Или предустановите в коде:
```rust
// libs/hbb_common/src/config.rs
pub const RENDEZVOUS_SERVERS: &[&str] = &["10.21.31.11"];
pub const RS_PUB_KEY: &str = "your-key=";
```

И добавьте API server в DEFAULT_SETTINGS.






