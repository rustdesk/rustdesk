import { is_osx,view,OS,handler,translate,msgbox,is_win,svg_checkmark,svg_edit,isReasonableSize,centerize,svg_eye } from "./common";
import { SearchBar,SessionStyle,SessionList } from "./ab.js";
import {$} from "@sciter"; //TEST $$ import

if (is_osx) view.blurBehind = "light";
console.log("current platform:", OS);
console.log("wayland",handler.xcall("is_login_wayland"));
// html min-width, min-height not working on mac, below works for all
view.minSize = [500, 300]; // TODO not work on ubuntu

export var app; // 注意判空
var tmp = handler.xcall("get_connect_status");
var connect_status = tmp[0];
var service_stopped = false;
var software_update_url = "";
var key_confirmed = tmp[1];
var system_error = "";

export const svg_menu = <svg id="menu" viewBox="0 0 512 512">
	<circle cx="256" cy="256" r="64"/>
	<circle cx="256" cy="448" r="64"/>
	<circle cx="256" cy="64" r="64"/>
</svg>;

var my_id = "";
function get_id() {
  my_id = handler.xcall("get_id");
  return my_id;
}

class ConnectStatus extends Element {
    render() {
        return(<div class="connect-status">
                <span class={"connect-status-icon connect-status" + (service_stopped ? 0 : connect_status)} />
                {this.getConnectStatusStr()}
                {service_stopped ? <span class="link" id="start-service">{translate('Start Service')}</span> : ""}
            </div>);
    }

    getConnectStatusStr() {
        if (service_stopped) {
            return translate("Service is not running");
        } else if (connect_status == -1) {
            return translate('not_ready_status');
        } else if (connect_status == 0) {
            return translate('connecting_status');
        }
        return translate("Ready");
    }

    ["on click at #start-service"]() {
        handler.xcall("set_option","stop-service", "");
    }
}

class RecentSessions extends Element {
    sessionList;
    componentDidMount(){
        this.sessionList = this.$("#SessionList")
    }
    render() {
        let sessions = handler.xcall("get_recent_sessions");
        if (sessions.length == 0) return <span />;
        return (<div style="width: *">
            <div class="sessions-bar">
                <div style="width:*">
                    {translate("Recent Sessions")}
                </div>
                {!app.hidden && <SearchBar parent={this} />}
                {!app.hidden && <SessionStyle />}
            </div>
            {!app.hidden && <SessionList id="SessionList" sessions={sessions} />} 
        </div>);
    }

    filter(v) {
        this.sessionList.filter(v);
    }
}

export function createNewConnect(id, type) {
    id = id.replace(/\s/g, "");
    app.remote_id.value = formatId(id);
    if (!id) return;
    if (id == my_id) {
        msgbox("custom-error", "Error", "You cannot connect to your own computer");
        return;
    }
    handler.xcall("set_remote_id",id);
    handler.xcall("new_remote",id, type);
}

var myIdMenu;
var audioInputMenu;
class AudioInputs extends Element {
    this() {
        audioInputMenu = this;
    }

    render() {
        // TODO this.show
        if (!this.show) return <li />;
        let inputs = handler.xcall("get_sound_inputs");
        if (is_win) inputs = ["System Sound"].concat(inputs);
        if (!inputs.length) return <div/>;
        inputs = ["Mute"].concat(inputs);
        setTimeout(()=>this.toggleMenuState(),1);
        return (<li>{translate('Audio Input')}
            <menu id="audio-input" key={inputs.length}>
                {inputs.map((name)=><li id={name}><span>{svg_checkmark}</span>{translate(name)}</li>)}
            </menu>
        </li>);
    }

    get_default() {
        if (is_win) return "System Sound";
        return "";
    }

    get_value() {
        return handler.xcall("get_option","audio-input") || this.get_default();
    }

    toggleMenuState() {
        let v = this.get_value();
        for (let el of this.$$("menu#audio-input>li")) {
            let selected = el.id == v;
            el.classList.toggle("selected", selected);
        }
    }

    ["on click at menu#audio-input>li"](_, me) {
        let v = me.id;
        if (v == this.get_value()) return;
        if (v == this.get_default()) v = "";
        handler.xcall("set_option","audio-input", v);
        this.toggleMenuState();
    }
}

class MyIdMenu extends Element {
    this() {
        myIdMenu = this;
    }

    render() {
        return (<div id="myid">
            {this.renderPop()}
            ID{svg_menu}
        </div>);
    }

    renderPop() {
        return (<popup>
            <menu class="context" id="config-options">
                <li id="enable-keyboard"><span>{svg_checkmark}</span>{translate('Enable Keyboard/Mouse')}</li>
                <li id="enable-clipboard"><span>{svg_checkmark}</span>{translate('Enable Clipboard')}</li>
                <li id="enable-file-transfer"><span>{svg_checkmark}</span>{translate('Enable File Transfer')}</li> 
                <li id="enable-tunnel"><span>{svg_checkmark}</span>{translate('Enable TCP Tunneling')}</li>
                <AudioInputs />
                <div class="separator" />
                <li id="whitelist" title={translate('whitelist_tip')}>{translate('IP Whitelisting')}</li>
                <li id="custom-server">{translate('ID/Relay Server')}</li>
                <div class="separator" />
                <li id="stop-service" class={service_stopped ? "line-through" : "selected"}><span>{svg_checkmark}</span>{translate("Enable Service")}</li>
                <div class="separator" />
                <li id="about">{translate('About')} {" "} {handler.xcall("get_app_name")}</li>
            </menu>
        </popup>);
    }

    // TEST svg#menu  // .popup()
    ["on click at svg#menu"](_, me) {
        console.log("open menu")
        audioInputMenu.componentUpdate({ show: true });
        this.toggleMenuState();
        let menu = this.$("menu#config-options");
        me.popup(menu); 
    }

    toggleMenuState() {
        for (let el of this.$$("menu#config-options>li")) {
            if (el.id && el.id.indexOf("enable-") == 0) {
                let enabled = handler.xcall("get_option",el.id) != "N";
                console.log(el.id,enabled)
                el.classList.toggle("selected", enabled);
                el.classList.toggle("line-through", !enabled);
            }
        }
    }

    ["on click at menu#config-options>li"] (_, me) {
        if (me.id && me.id.indexOf("enable-") == 0) {
            handler.xcall("set_option",me.id, handler.xcall("get_option",me.id) == "N" ? "" : "N");
        }
        if (me.id == "whitelist") {
            let old_value = handler.xcall("get_option","whitelist").split(",").join("\n");
            msgbox("custom-whitelist", translate("IP Whitelisting"), "<div class='form'> \
            <div>" + translate("whitelist_sep") + "</div> \
            <textarea spellcheck=\"false\" name=\"text\" novalue=\"0.0.0.0\" style=\"overflow: scroll-indicator; width:*; height: 160px; font-size: 1.2em; padding: 0.5em;\">" + old_value + "</textarea>\
            </div> \
            ",
            function(res=null) {
                if (!res) return;
                let value = (res.text || "").trim();
                if (value) {
                    let values = value.split(/[\s,;\n]+/g);
                    for (let ip in values) {
                        if (!ip.match(/^\d+\.\d+\.\d+\.\d+$/)) {
                            return translate("Invalid IP") + ": " + ip;
                        }
                    }
                    value = values.join("\n");
                }
                if (value == old_value) return;
                console.log("whitelist updated");
                handler.xcall("set_option","whitelist", value.replace("\n", ","));
            }, 300);
        } else if (me.id == "custom-server") {
            let configOptions = handler.xcall("get_options");
            let old_relay = configOptions["relay-server"] || "";
            let old_id = configOptions["custom-rendezvous-server"] || "";
            msgbox("custom-server", "ID/Relay Server", "<div class='form'> \
            <div><span style='width: 100px; display:inline-block'>" + translate("ID Server") + ": </span><input .outline-focus style='width: 250px' name='id' value='" + old_id + "' /></div> \
            <div><span style='width: 100px; display:inline-block'>" + translate("Relay Server") + ": </span><input style='width: 250px' name='relay' value='" + old_relay + "' /></div> \
            </div> \
            ", 
            function(res=null) {
                if (!res) return;
                let id = (res.id || "").trim();
                let relay = (res.relay || "").trim();
                if (id == old_id && relay == old_relay) return;
                if (id) {
                    let err = handler.xcall("test_if_valid_server",id);
                    if (err) return translate("ID Server") + ": " + err;
                }
                if (relay) {
                    let err = handler.xcall("test_if_valid_server",relay);
                    if (err) return translate("Relay Server") + ": " + err;
                }
                configOptions["custom-rendezvous-server"] = id;
                configOptions["relay-server"] = relay;
                handler.xcall("set_options",configOptions);
            }, 240);
        } else if (me.id == "stop-service") {
            handler.xcall("set_option","stop-service", service_stopped ? "" : "Y");
        } else if (me.id == "about") {
            let name = handler.xcall("get_app_name");
            msgbox("custom-nocancel-nook-hasclose", "About " + name, "<div style='line-height: 2em'> \
                <div>Version: " + handler.xcall("get_version") + " \
                <div class='link custom-event' url='http://rustdesk.com/privacy'>Privacy Statement</div> \
                <div class='link custom-event' url='http://rustdesk.com'>Website</div> \
                <div style='background: #2c8cff; color: white; padding: 1em; margin-top: 1em;'>Copyright &copy; 2020 CarrieZ Studio \
                <br /> Author: Carrie \
                <p style='font-weight: bold'>Made with heart in this chaotic world!</p>\
                </div>\
            </div>", 
            function(el) {
                if (el && el.attributes) {
                    handler.xcall("open_url",el.attributes['url']);
                };
            }, 400);
        }
    }
}

class App extends Element{
    remote_id;
    recent_sessions;
    connect_status;
    this() {
        app = this;
    }
    componentDidMount(){
        this.remote_id = this.$("#ID");
        this.recent_sessions = this.$("#RecentSessions");
        this.connect_status = this.$("#ConnectStatus");
    }

    render() {
        let is_can_screen_recording = handler.xcall("is_can_screen_recording",false);
        return(<div class="app">
              <popup>
              <menu class="context" id="edit-password-context">
                <li id="refresh-password">Refresh random password</li>
                <li id="set-password">Set your own password</li>
              </menu>
              </popup>
                <div class="left-pane">
                    <div>
                        <div class="title">{translate('Your Desktop')}</div>
                        <div class="lighter-text">{translate('desk_tip')}</div>
                        <div class="your-desktop">
                            <MyIdMenu />
                            {key_confirmed ? <input type="text" readonly value={formatId(get_id())}/> : translate("Generating ...")}
                        </div>
                        <div class="your-desktop">
                            <div>{translate('Password')}</div>
                            <Password />
                        </div>
                    </div>
                    {handler.xcall("is_installed") ? "": <InstallMe />}
                    {handler.xcall("is_installed") && software_update_url ? <UpdateMe /> : ""}
                    {handler.xcall("is_installed") && !software_update_url && handler.xcall("is_installed_lower_version") ? <UpgradeMe /> : ""}
                    {is_can_screen_recording ? "": <CanScreenRecording />}
                    {is_can_screen_recording && !handler.xcall("is_process_trusted",false) ? <TrustMe /> : ""}
                    {system_error ? <SystemError /> : ""}
                    {!system_error && handler.xcall("is_login_wayland") && !handler.xcall("current_is_wayland") ? <FixWayland /> : ""}
                    {!system_error && handler.xcall("current_is_wayland") ? <ModifyDefaultLogin /> : ""}
                </div>
                <div class="right-pane">
                    <div class="right-content">
                        <div class="card-connect">
                            <div class="title">{translate('Control Remote Desktop')}</div>
                            <ID id="ID" />
                            <div class="right-buttons">
                                <button class="button outline" id="file-transfer">{translate('Transfer File')}</button>
                                <button class="button" id="connect">{translate('Connect')}</button>
                            </div>
                        </div>
                        <RecentSessions id="RecentSessions" />
                    </div>
                    <ConnectStatus id="ConnectStatus" />
                </div>
            </div>);
    }

    ["on click at button#connect"](){
        this.newRemote("connect");
    }

    ["on click at button#file-transfer"]() {
        this.newRemote("file-transfer");
    }

    ["on keydown"](evt) {
        if (!evt.shortcutKey) {
            // TODO TEST Windows/Mac
            if (evt.code == "KeyRETURN") {
                var el = $("button#connect");
                view.focus = el;
                el.click();
                // simulate button click effect, windows does not have this issue
                el.classList.toggle("active", true);
                el.timer(300, ()=> el.classList.toggle("active", false));
            }
        }
    }

    newRemote(type) {
        createNewConnect(this.remote_id.value, type);
    }
}

class InstallMe extends Element {
    render() {
        return (<div class="install-me">
            <span />
            <div>{translate('install_tip')}</div>
            <button id="install-me" class="button">{translate('Install')}</button>
        </div>);
    }

    ["on click at #install-me"]() {
        handler.xcall("goto_install");
    }
}

const http = function() { 
  function makeRequest(httpverb) {
    return function( params ) {
      params.type = httpverb;
      // TODO request
      view.request(params);
    };
  }
  function download(from, to, ...args) {
      // TODO #get 
      let rqp = { type:"get", url: from, toFile: to };
      let fn = 0;
      let on = 0;
      // TODO p in / p of?
      for( let p in args )
        if( p instanceof Function )
        {
          switch(++fn) {
            case 1: rqp.success = p; break;
            case 2: rqp.error = p; break;
            case 3: rqp.progress = p; break;
          }
        } else if( p instanceof Object )
        {
          switch(++on) {
            case 1: rqp.params = p; break;
            case 2: rqp.headers = p; break;
          }
        }  
        // TODO request
      view.request(rqp);
  }
  
  return {
    get:  makeRequest("get"),
    post: makeRequest("post"),
    put:  makeRequest("put"),
    del:  makeRequest("delete"),
    download: download
  };

}();

class UpgradeMe extends Element {
    render() {
        let update_or_download = is_osx ? "download" : "update";
        return (<div class="install-me">
            <div>{translate('Status')}</div>
            <div>{translate('Your installation is lower version.')}</div>
            <div id="install-me" class="link" style="padding-top: 1em">{translate('Click to upgrade')}</div>
        </div>);
    }

    ["on click at #install-me"]() {
        handler.xcall("update_me");
    }
}

class UpdateMe extends Element {
    render() {
        let update_or_download = "download"; // !is_win ? "download" : "update";
        return (<div class="install-me">
            <div>{translate('Status')}</div>
            <div>There is a newer version of {handler.xcall("get_app_name")} ({handler.xcall("get_new_version")}) available.</div>
            <div id="install-me" class="link" style="padding-top: 1em">Click to {update_or_download}</div>
            <div id="download-percent" style="display:hidden; padding-top: 1em;" />
        </div>);
    }

    ["on click at #install-me"]() {
        handler.xcall("open_url","https://rustdesk.com");
        return;
        // TODO return?
        if (!is_win) {
            handler.xcall("open_url","https://rustdesk.com");
            return;
        }
        let url = software_update_url + '.' + handler.xcall("get_software_ext");
        let path = handler.xcall("get_software_store_path");
        let onsuccess = function(md5) {
            this.$("#download-percent").content(translate("Installing ..."));
            handler.xcall("update_me",path);
        };
        let onerror = function(err) {
            msgbox("custom-error", "Download Error", "Failed to download"); 
        };
        let onprogress = function(loaded, total) {
            if (!total) total = 5 * 1024 * 1024;
            let el = this.$("#download-percent");
            el.style.setProperty("display","block");
            el.content("Downloading %" + (loaded * 100 / total));
        };
        console.log("Downloading " + url + " to " + path);
        http.download(
            url,
            document.url(path),
            onsuccess, onerror, onprogress);
    }
}

class SystemError extends Element {
    render() {
        return (<div class="install-me">
            <div>{system_error}</div>
        </div>);
    }
}

class TrustMe extends Element {
    render() {
        return (<div class="trust-me">
            <div>{translate('Configuration Permissions')}</div>
            <div>{translate('config_acc')}</div>
            <div id="trust-me" class="link">{translate('Configure')}</div>
        </div>);
    }

    ["on click at #trust-me"] () {
        handler.xcall("is_process_trusted",true);
        watch_trust();
    }
}

class CanScreenRecording extends Element {
    render() {
        return (<div class="trust-me">
            <div>{translate('Configuration Permissions')}</div>
            <div>{translate('config_screen')}</div>
            <div id="screen-recording" class="link">{translate('Configure')}</div>
        </div>);
    }

    ["on click at #screen-recording"]() {
        handler.xcall("is_can_screen_recording",true);
        watch_trust();
    }
}

class FixWayland extends Element {
    render() {
        return (<div class="trust-me">
            <div>{translate('Warning')}</div>
            <div>{translate('Login screen using Wayland is not supported')}</div>
            <div id="fix-wayland" class="link">{translate('Fix it')}</div>
            <div style="text-align: center">({translate('Reboot required')})</div>
        </div>);
    }

    ["on click at #fix-wayland"] () {
        handler.xcall("fix_login_wayland");
        app.componentUpdate();
    }
}

class ModifyDefaultLogin extends Element {
    render() {
        return (<div class="trust-me">
            <div>{translate('Warning')}</div>
            <div>{translate('Current Wayland display server is not supported')}</div>
            <div id="modify-default-login" class="link">{translate('Fix it')}</div>
            <div style="text-align: center">({translate('Reboot required')})</div>
        </div>);
    }

    ["on click at #modify-default-login"]() {
        // TEST change  tis old:
        // if (var r = handler.modify_default_login()) {
        //     msgbox("custom-error", "Error", r);
        // }
        let r = handler.xcall("modify_default_login");
        if (r) {
            msgbox("custom-error", "Error", r);
        }
        app.componentUpdate();
    }
}

function watch_trust() {
    // not use TrustMe::update, because it is buggy
    let trusted = handler.xcall("is_process_trusted",false);
    let el = document.$("div.trust-me");
    if (el) {
        el.style.setProperty("display", trusted ? "none" : "block");
    }
    // if (trusted) return;
    // TODO dont have exit?
    setTimeout(() => {
        watch_trust()
    }, 1000);
}

class PasswordEyeArea extends Element {
    render() {
        return (<div class="eye-area" style="width: *">
                <input type="text" readonly value="******" />
                {svg_eye}
            </div>);
    }
    
    ["on mouseenter"]() {
        this.leaved = false;
        setTimeout(()=> {
            if (this.leaved) return;
            this.$("input").value = handler.xcall("get_password");
        },300);
    }

    ["on mouseleave"]() {
        this.leaved = true;
        this.$("input").value = "******";
    }
}

class Password extends Element {
    render() {
        return (<div class="password" style="flow:horizontal">
            <PasswordEyeArea />
            {svg_edit}
        </div>);
    }

    // TODO expecting element to popup 这里组件无法触发
    ["on click at svg#edit"](_,me) {
        let menu = this.$("menu#edit-password-context");
        console.log("修改密码",me)
        me.popup(menu);
    }

    ["on click at li#refresh-password"] () {
        handler.xcall("update_password");
        this.componentUpdate();
    }

    ["on click at li#set-password"] () {
        // option .form .set-password ...
        msgbox("custom-password", translate("Set Password"), "<div .form .set-password> \
            <div><span>" + translate('Password') + ":</span><input|password(password) .outline-focus /></div> \
            <div><span>" + translate('Confirmation') + ":</span><input|password(confirmation) /></div> \
        </div> \
        ", 
        function(res=null) {
            if (!res) return;
            let p0 = (res.password || "").trim();
            let p1 = (res.confirmation || "").trim();
            if (p0.length < 6) {
                return translate("Too short, at least 6 characters.");
            }
            if (p0 != p1) {
                return translate("The confirmation is not identical.");
            }
            handler.xcall("update_password",p0);
            this.componentUpdate();
        });
    }
}

class ID extends Element {
    render() {
        return <input type="text" id="remote_id" class="outline-focus" novalue={translate("Enter Remote ID")} maxlength="15" value={formatId(handler.xcall("get_remote_id"))} />;
    }

    // TEST
    // https://github.com/c-smile/sciter-sdk/blob/master/doc/content/sciter/Event.htm
    ["on change"]() {
        let fid = formatId(this.value);
        let d = this.value.length - (this.old_value || "").length;
        this.old_value = this.value;
        let start = this.xcall("selectionStart") || 0;
        let end = this.xcall("selectionEnd");
        if (fid == this.value || d <= 0 || start != end) {
            return;
        }
        // fix Caret position
        this.value = fid;
        let text_after_caret = this.old_value.substr(start);
        let n = fid.length - formatId(text_after_caret).length;
        this.xcall("setSelection", n, n);
    }
}

var reg = /^\d+$/;
export function formatId(id) {
    id = id.replace(/\s/g, "");
    if (reg.test(id) && id.length > 3) {
        let n = id.length;
        let a = n % 3 || 3;
        let new_id = id.substr(0, a);
        for (let i = a; i < n; i += 3) {
            new_id += " " + id.substr(i, 3);
        }
        return new_id;
    }
    return id;
}

document.body.content(<App />);

document.on("ready",()=>{
    let r = handler.xcall("get_size");
    if (isReasonableSize(r) && r[2] > 0) {
        view.move(r[0], r[1], r[2], r[3]);
    } else {
        centerize(800, 600);
    }
    if (!handler.xcall("get_remote_id")) {
        view.focus = $("#remote_id"); // TEST
    }
})

document.on("unloadequest",(evt)=>{
    // evt.preventDefault() // can prevent window close
    let [x, y, w, h] = view.box("rectw", "border", "desktop");
    handler.xcall("save_size",x, y, w, h);
})

// check connect status
setInterval(() => {
    let tmp = !!handler.xcall("get_option","stop-service");
        if (tmp != service_stopped) {
            service_stopped = tmp;
            app.connect_status.componentUpdate();
            myIdMenu.componentUpdate();
        }
        tmp = handler.xcall("get_connect_status");
        if (tmp[0] != connect_status) {
            connect_status = tmp[0];
            app.connect_status.componentUpdate();
        }
        if (tmp[1] != key_confirmed) {
            key_confirmed = tmp[1];
            app.componentUpdate();
        }
        if (tmp[2] && tmp[2] != my_id) {
            console.log("id updated");
            app.componentUpdate();
        }
        tmp = handler.xcall("get_error");
        if (system_error != tmp) {
            system_error = tmp;
            app.componentUpdate();
        }
        tmp = handler.xcall("get_software_update_url");
        if (tmp != software_update_url) {
            software_update_url = tmp;
            app.componentUpdate();
        }
        if (handler.xcall("recent_sessions_updated")) {
            console.log("recent sessions updated");
            app.recent_sessions.componentUpdate();
        }
}, 1000);
