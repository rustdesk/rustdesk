# 📋 Добавление отдельного меню для настроек в RustDesk

## Описание
Текущий UI имеет все настройки в одном выпадающем меню при нажатии на троеточие (три точки). Это изменение создаст отдельные меню для:
- **Settings (Настройки)** - основные настройки соединения и сервера
- **Preferences (Параметры)** - локальные предпочтения (тема, язык, обновления)
- **Account (Аккаунт)** - вход/выход

## Файлы для изменения
- `src/ui/index.tis` - основной UI файл

## Изменения

### Шаг 1: Добавить SVG иконки для новых кнопок (найти строку 45-50)

```javascript
// Добавить после svg_menu:
var svg_settings = <svg #settings viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <circle cx="12" cy="12" r="3"/>
    <path d="M12 1v6m0 6v4M4.22 4.22l4.24 4.24m3.08 3.08l4.24 4.24M1 12h6m6 0h4M4.22 19.78l4.24-4.24m3.08-3.08l4.24-4.24M19.78 19.78l-4.24-4.24m-3.08-3.08l-4.24-4.24M23 12h-6m-6 0h-4"/>
</svg>;

var svg_preferences = <svg #preferences viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <polyline points="4 6 10 10 4 14 4 6"/>
    <polyline points="20 6 14 10 20 14 20 6"/>
    <line x1="10" y1="10" x2="14" y2="10"/>
</svg>;

var svg_user = <svg #user viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <path d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"/>
    <circle cx="12" cy="7" r="4"/>
</svg>;
```

### Шаг 2: Модифицировать класс MyIdMenu (строки 471-623)

Текущий код:
```javascript
class MyIdMenu: Reactor.Component {
    function render() {
        return <div #myid>
            {this.renderPop()}
            ID{svg_menu}
        </div>;
    }
```

Новый код:
```javascript
class MyIdMenu: Reactor.Component {
    function render() {
        return <div #myid>
            {this.renderPopSettings()}
            {this.renderPopPreferences()}
            {this.renderPopAccount()}
            ID{svg_menu}
            <span .separator-menu>|</span>
            {svg_settings}
            <span .separator-menu>|</span>
            {svg_preferences}
            <span .separator-menu>|</span>
            {svg_user}
        </div>;
    }

    function renderPopSettings() {
        return <popup>
            <menu.context #config-settings>
                {!disable_settings && <li #enable-keyboard><span>{svg_checkmark}</span>{translate('Enable keyboard/mouse')}</li>}
                {!disable_settings && <li #enable-clipboard><span>{svg_checkmark}</span>{translate('Enable clipboard')}</li>}
                {!disable_settings && <li #enable-file-transfer><span>{svg_checkmark}</span>{translate('Enable file transfer')}</li>}
                {!disable_settings && <li #enable-camera><span>{svg_checkmark}</span>{translate('Enable camera')}</li>}
                {!disable_settings && <li #enable-terminal><span>{svg_checkmark}</span>{translate('Enable terminal')}</li>}
                {!disable_settings && <li #enable-remote-restart><span>{svg_checkmark}</span>{translate('Enable remote restart')}</li>}
                {!disable_settings && <li #enable-tunnel><span>{svg_checkmark}</span>{translate('Enable TCP tunneling')}</li>}
                {!disable_settings && is_win ? <li #enable-block-input><span>{svg_checkmark}</span>{translate('Enable blocking user input')}</li> : ""}
                {!disable_settings && <li #enable-lan-discovery><span>{svg_checkmark}</span>{translate('Enable LAN discovery')}</li>}
                <AudioInputs />
                <Enhancements />
                {!disable_settings && <li #allow-remote-config-modification><span>{svg_checkmark}</span>{translate('Enable remote configuration modification')}</li>}
                <div .separator />
                {!disable_settings && !hide_server_settings && <li #custom-server>{translate('ID/Relay Server')}</li>}
                {!disable_settings && <li #whitelist title={translate('whitelist_tip')}>{translate('IP Whitelisting')}</li>}
                {!disable_settings && !hide_proxy_settings && <li #socks5-server>{translate('Socks5/Http(s) Proxy')}</li>}
                {!disable_settings && !hide_websocket_settings && <li #allow-websocket><span>{svg_checkmark}</span>{translate('Use WebSocket')}</li>}
                {!disable_settings && !using_public_server && !outgoing_only && <li #disable-udp class={disable_udp ? "selected" : "line-through"}><span>{svg_checkmark}</span>{translate('Disable UDP')}</li>}
                {!disable_settings && !using_public_server && <li #allow-insecure-tls-fallback><span>{svg_checkmark}</span>{translate('Allow insecure TLS fallback')}</li>}
                <li #stop-service class={service_stopped ? "line-through" : "selected"}><span>{svg_checkmark}</span>{translate("Enable service")}</li>
                {!disable_settings && is_win && handler.is_installed() ? <ShareRdp /> : ""}
                {!disable_settings && <DirectServer />}
            </menu>
        </popup>;
    }

    function renderPopPreferences() {
        return <popup>
            <menu.context #config-preferences>
                <li #allow-darktheme><span>{svg_checkmark}</span>{translate('Dark Theme')}</li>
                <Languages />
                {disable_installation ? "" : <li #allow-auto-update><span>{svg_checkmark}</span>{translate('Auto update')}</li>}
                <li #about>{translate('About')} {" "}{handler.get_app_name()}</li>
            </menu>
        </popup>;
    }

    function renderPopAccount() {
        var username = handler.get_local_option("access_token") ? getUserName() : '';
        return <popup>
            <menu.context #config-account>
                {!disable_account && (username ? 
                <li #logout>{translate('Logout')} ({username})</li> :
                <li #login>{translate('Login')}</li>)}
                {!disable_settings && handler.is_ok_change_id() && key_confirmed && connect_status > 0 ? <li #change-id>{translate('Change ID')}</li> : ""}
            </menu>
        </popup>;
    }
```

### Шаг 3: Обновить обработчики событий

Заменить:
```javascript
event click $(svg#menu) (_, me) {
    this.showSettingMenu();
}
```

На:
```javascript
event click $(svg#settings) (_, me) {
    this.showSettingsMenu();
}

event click $(svg#preferences) (_, me) {
    this.showPreferencesMenu();
}

event click $(svg#user) (_, me) {
    this.showAccountMenu();
}

event click $(svg#menu) (_, me) {
    // Оставить для обратной совместимости или удалить
    this.showSettingMenu();
}
```

### Шаг 4: Добавить новые функции для меню

```javascript
function showSettingsMenu() {
    audioInputMenu.update({ show: true });
    this.toggleMenuState("config-settings");
    if (direct_server) direct_server.update();
    var menu = this.$(menu#config-settings);
    this.$("svg#settings").popup(menu);
}

function showPreferencesMenu() {
    var menu = this.$(menu#config-preferences);
    this.$("svg#preferences").popup(menu);
}

function showAccountMenu() {
    var menu = this.$(menu#config-account);
    this.$("svg#user").popup(menu);
}

function showSettingMenu() {
    // Оставить для совместимости, перенаправить на showSettingsMenu
    this.showSettingsMenu();
}
```

### Шаг 5: Обновить функцию toggleMenuState

Изменить:
```javascript
function toggleMenuState() {
    for (var el in $$(menu#config-options>li)) {
```

На:
```javascript
function toggleMenuState(menuId = "config-settings") {
    var selector = "menu#" + menuId + ">li";
    for (var el in $$(selector)) {
```

## CSS Стили (добавить в style.css)

```css
/* Settings menu buttons */
#myid {
    display: flex;
    align-items: center;
    gap: 10px;
}

#myid svg[id] {
    width: 24px;
    height: 24px;
    cursor: pointer;
    transition: all 0.3s ease;
}

#myid svg[id]:hover {
    color: #0066cc;
    transform: scale(1.1);
}

.separator-menu {
    color: #ccc;
    margin: 0 2px;
}
```

## Тестирование

1. Откомпилировать RustDesk с изменениями
2. Проверить, что три новые кнопки видны рядом с ID
3. Тестировать каждое меню:
   - Settings - проверить все опции соединения
   - Preferences - проверить тему, язык, обновления
   - Account - проверить вход/выход

## Возможные улучшения

1. Добавить подменю в Settings (Network, Permissions, Advanced)
2. Сделать иконки более выразительными
3. Добавить горячие клавиши (Ctrl+,) для быстрого доступа к настройкам
4. Сделать полноценное окно Settings вместо выпадающих меню

---






