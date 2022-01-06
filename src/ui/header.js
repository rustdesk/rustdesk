import { handler,view,setWindowButontsAndIcon,translate,msgbox,adjustBorder,is_osx,is_xfce,svg_chat,svg_checkmark, is_linux } from "./common.js";
import {$,$$} from "@sciter";
import { adaptDisplay, audio_enabled, clipboard_enabled, keyboard_enabled } from "./remote.js";
var pi = handler.xcall("get_default_pi"); // peer information

var chat_msgs = [];

const svg_fullscreen = (<svg viewBox="0 0 357 357">
    <path d="M51,229.5H0V357h127.5v-51H51V229.5z M0,127.5h51V51h76.5V0H0V127.5z M306,306h-76.5v51H357V229.5h-51V306z M229.5,0v51    H306v76.5h51V0H229.5z"/>
</svg>);
const svg_action = (<svg viewBox="-91 0 512 512"><path d="M315 211H191L298 22a15 15 0 00-13-22H105c-6 0-12 4-14 10L1 281a15 15 0 0014 20h127L61 491a15 15 0 0025 16l240-271a15 15 0 00-11-25z"/></svg>);
const svg_display = (<svg viewBox="0 0 640 512">
    <path d="M592 0H48A48 48 0 0 0 0 48v320a48 48 0 0 0 48 48h240v32H112a16 16 0 0 0-16 16v32a16 16 0 0 0 16 16h416a16 16 0 0 0 16-16v-32a16 16 0 0 0-16-16H352v-32h240a48 48 0 0 0 48-48V48a48 48 0 0 0-48-48zm-16 352H64V64h512z"/>
</svg>);
const svg_secure = (<svg viewBox="0 0 347.97 347.97">
<path fill="#3F7D46" d="m317.31 54.367c-59.376 0-104.86-16.964-143.33-54.367-38.461 37.403-83.947 54.367-143.32 54.367 0 97.405-20.155 236.94 143.32 293.6 163.48-56.666 143.33-196.2 143.33-293.6zm-155.2 171.41-47.749-47.756 21.379-21.378 26.37 26.376 50.121-50.122 21.378 21.378-71.499 71.502z"/>
</svg>);
const svg_insecure = (<svg viewBox="0 0 347.97 347.97"><path d="M317.469 61.615c-59.442 0-104.976-16.082-143.489-51.539-38.504 35.457-84.04 51.539-143.479 51.539 0 92.337-20.177 224.612 143.479 278.324 163.661-53.717 143.489-185.992 143.489-278.324z" fill="none" stroke="red" stroke-width="14.827"/><g fill="red"><path d="M238.802 115.023l-111.573 114.68-8.6-8.367L230.2 106.656z"/><path d="M125.559 108.093l114.68 111.572-8.368 8.601-114.68-111.572z"/></g></svg>);
const svg_insecure_relay = (<svg viewBox="0 0 347.97 347.97"><path d="M317.469 61.615c-59.442 0-104.976-16.082-143.489-51.539-38.504 35.457-84.04 51.539-143.479 51.539 0 92.337-20.177 224.612 143.479 278.324 163.661-53.717 143.489-185.992 143.489-278.324z" fill="none" stroke="red" stroke-width="14.827"/><g fill="red"><path d="M231.442 247.498l-7.754-10.205c-17.268 12.441-38.391 17.705-59.478 14.822-21.087-2.883-39.613-13.569-52.166-30.088-25.916-34.101-17.997-82.738 17.65-108.42 32.871-23.685 78.02-19.704 105.172 7.802l-32.052 7.987 3.082 12.369 48.722-12.142-11.712-46.998-12.822 3.196 4.496 18.039c-31.933-24.008-78.103-25.342-112.642-.458-31.361 22.596-44.3 60.436-35.754 94.723 2.77 11.115 7.801 21.862 15.192 31.588 30.19 39.727 88.538 47.705 130.066 17.785z"/></g></svg>);
const svg_secure_relay = (<svg viewBox="0 0 347.97 347.97"><path d="M317.469 61.615c-59.442 0-104.976-16.082-143.489-51.539-38.504 35.457-84.04 51.539-143.479 51.539 0 92.337-20.177 224.612 143.479 278.324 163.661-53.717 143.489-185.992 143.489-278.324z" fill="#3f7d46" stroke="#3f7d46" stroke-width="14.827"/><g fill="red"><path d="M231.442 247.498l-7.754-10.205c-17.268 12.441-38.391 17.705-59.478 14.822-21.087-2.883-39.613-13.569-52.166-30.088-25.916-34.101-17.997-82.738 17.65-108.42 32.871-23.685 78.02-19.704 105.172 7.802l-32.052 7.987 3.082 12.369 48.722-12.142-11.712-46.998-12.822 3.196 4.496 18.039c-31.933-24.008-78.103-25.342-112.642-.458-31.361 22.596-44.3 60.436-35.754 94.723 2.77 11.115 7.801 21.862 15.192 31.588 30.19 39.727 88.538 47.705 130.066 17.785z" fill="#fff"/></g></svg>);

var cur_window_state = view.state;


if (is_linux) {
    // check_state_change;
    setInterval(() => {
        if (view.state != cur_window_state) {
            stateChanged();
        }    
    }, 30);
} else {
    view.on("statechange",()=>{
        stateChanged();
    })
}

function get_id() {
    return handler.xcall("get_option","alias") || handler.xcall("get_id")
}

function stateChanged() {
    console.log('state changed from ' + cur_window_state + ' -> ' + view.state);
    cur_window_state = view.state;
    adjustBorder();
    adaptDisplay();
    if (cur_window_state != Window.WINDOW_MINIMIZED) {
        view.focus = handler; // to make focus away from restore/maximize button, so that enter key work
    }
    let fs = view.state == Window.WINDOW_FULL_SCREEN;
    let el = $("#fullscreen");
    if (el) el.classList.toggle("active", fs);
    el = $("#maximize");
    if (el) {
        el.state.disabled = fs; // TODO TEST
    }
    if (fs) {
        $("header").style.setProperty("display","none");
    }
}

export var header;
var old_window_state = Window.WINDOW_SHOWN;
var input_blocked;

class Header extends Element {
    this() {
        header = this;
    }

    render() {
        let icon_conn;
        let title_conn;
        if (this.secure_connection && this.direct_connection) {
            icon_conn = svg_secure;
            title_conn = translate("Direct and encrypted connection");
        } else if (this.secure_connection && !this.direct_connection) {
            icon_conn = svg_secure_relay;
            title_conn = translate("Relayed and encrypted connection");
        } else if (!this.secure_connection && this.direct_connection) {
            icon_conn = svg_insecure;
            title_conn = translate("Direct and unencrypted connection");
        } else {
            icon_conn = svg_insecure_relay;
            title_conn = translate("Relayed and unencrypted connection");
        }
        let title = get_id();
        if (pi.hostname) title += "(" + pi.username + "@" + pi.hostname + ")";
        if ((pi.displays || []).length == 0) {
            return (<div class="ellipsis" style="size:*;text-align:center;margin:*;">{title}</div>);
        }
        let screens = pi.displays.map(function(d, i) {
            return <div id="screen" class={pi.current_display == i ? "current" : ""}>
                {i+1}
            </div>;
        });
        updateWindowToolbarPosition();
        let style = "flow:horizontal;";
        if (is_osx) style += "margin:*";
        setTimeout(toggleMenuState,1);
        
        return (<div style={style}>
            {is_osx || is_xfce ? "" : <span id="fullscreen">{svg_fullscreen}</span>}
            <div id="screens">
                <span id="secure" title={title_conn}>{icon_conn}</span>
                <div class="remote-id">{get_id()}</div>
                <div style="flow:horizontal;border-spacing: 0.5em;">{screens}</div>
                {this.renderGlobalScreens()}
            </div>
            <span id="chat">{svg_chat}</span>
            <span id="action">{svg_action}</span>
            <span id="display">{svg_display}</span>
            {this.renderDisplayPop()}
            {this.renderActionPop()}
        </div>);
    }    

    renderDisplayPop() {
        return (<popup>
            <menu class="context" id="display-options">
                <li id="adjust-window" style="display:none">{translate('Adjust Window')}</li> 
                <div id="adjust-window" class="separator" style="display:none"/>
                <li id="original" type="view-style"><span>{svg_checkmark}</span>{translate('Original')}</li> 
                <li id="shrink" type="view-style"><span>{svg_checkmark}</span>{translate('Shrink')}</li> 
                <li id="stretch" type="view-style"><span>{svg_checkmark}</span>{translate('Stretch')}</li> 
                <div class="separator" />
                <li id="best" type="image-quality"><span>{svg_checkmark}</span>{translate('Good image quality')}</li> 
                <li id="balanced" type="image-quality"><span>{svg_checkmark}</span>{translate('Balanced')}</li> 
                <li id="low" type="image-quality"><span>{svg_checkmark}</span>{translate('Optimize reaction time')}</li> 
                <li id="custom" type="image-quality"><span>{svg_checkmark}</span>{translate('Custom')}</li>
                <div class="separator" />
                <li id="show-remote-cursor" class="toggle-option"><span>{svg_checkmark}</span>{translate('Show remote cursor')}</li> 
                {audio_enabled ? <li id="disable-audio" class="toggle-option"><span>{svg_checkmark}</span>{translate('Mute')}</li> : ""}
                {keyboard_enabled && clipboard_enabled ? <li id="disable-clipboard" class="toggle-option"><span>{svg_checkmark}</span>{translate('Disable clipboard')}</li> : ""} 
                {keyboard_enabled ? <li id="lock-after-session-end" class="toggle-option"><span>{svg_checkmark}</span>{translate('Lock after session end')}</li> : ""} 
                {false && pi.platform == "Windows" ? <li id="privacy-mode" class="toggle-option"><span>{svg_checkmark}</span>{translate('Privacy mode')}</li> : ""}
            </menu>
        </popup>);
    }

    renderActionPop() {
        return (<popup>
            <menu class="context" id="action-options">
                <li id="transfer-file">{translate('Transfer File')}</li> 
                <li id="tunnel">{translate('TCP Tunneling')}</li> 
                <div class="separator" />
                {keyboard_enabled && (pi.platform == "Linux" || pi.sas_enabled) ? <li id="ctrl-alt-del">{translate('Insert')} Ctrl + Alt + Del</li> : ""}
                <div class="separator" />
                {keyboard_enabled ? <li id="lock-screen">{translate('Insert Lock')}</li> : ""}
                {false && pi.platform == "Windows" ? <li id="block-input">Block user input </li> : ""}
                {handler.xcall("support_refresh") ? <li id="refresh">{translate('Refresh')}</li> : ""}
            </menu>
        </popup>);
    }

    renderGlobalScreens() {
        if (pi.displays.length < 3) return "";
        let x0 = 9999999;
        let y0 = 9999999;
        let x = -9999999;
        let y = -9999999;
        pi.displays.map(function(d, i) {
            if (d.x < x0) x0 = d.x;
            if (d.y < y0) y0 = d.y;
            let dx = d.x + d.width;
            if (dx > x) x = dx;
            let dy = d.y + d.height;
            if (dy > y) y = dy;
        });
        let w = x - x0;
        let h = y - y0;
        let scale = 16. / h;
        let screens = pi.displays.map(function(d, i) {
            let min_wh = d.width > d.height ? d.height : d.width;
            let fs = min_wh * 0.9 * scale;
            let style = "width:" + (d.width * scale) + "px;" +
                        "height:" + (d.height * scale) + "px;" +
                        "left:" + ((d.x - x0) * scale) + "px;" +
                        "top:" + ((d.y - y0) * scale) + "px;" +
                        "font-size:" + fs + "px;";
            if (is_osx) {
              style += "line-height:" + fs + "px;";
            }
            return <div style={style} class={pi.current_display == i ? "current" : ""}>{i+1}</div>;
        });

        let style = "width:" + (w * scale) + "px; height:" + (h * scale) + "px;";
        return <div id="global-screens" style={style}>
            {screens}
        </div>;
    }

    ["on click at #fullscreen"](_, el) {
        if (view.state == Window.WINDOW_FULL_SCREEN) {
            if (old_window_state == Window.WINDOW_MAXIMIZED) {
                view.state = Window.WINDOW_SHOWN;
            }
            view.state = old_window_state;
        } else {
            old_window_state = view.state;
            if (view.state == Window.WINDOW_MAXIMIZED) {
                view.state = Window.WINDOW_SHOWN;
            }
            view.state = Window.WINDOW_FULL_SCREEN;
            if (is_linux) { setTimeout(()=>view.state = Window.WINDOW_FULL_SCREEN,150); }
        }
    }
    
    ["on click at #chat"]() {
        startChat();
    }
    
    ["on click at #action"](_, me) {
        let menu = $("menu#action-options");
        me.popup(menu);
    }

    ["on click at #display"](_, me) {
        let menu = $("menu#display-options");
        me.popup(menu);
    }

    ["on click at #screen"](_, me) {
        if (pi.current_display == me.index) return;
        handler.xcall("switch_display",me.index);
    }

    ["on click at #transfer-file"]() {
        handler.xcall("transfer_file");
    }

    ["on click at #tunnel"] () {
        handler.xcall("tunnel");
    }

    ["on click at #ctrl-alt-del"]() {
        handler.xcall("ctrl_alt_del");
    }
    
    ["on click at #lock-screen"]() {
        handler.xcall("lock_screen");
    }
    
    ["on click at #refresh"] () {
        handler.xcall("refresh_video");
    }

    ["on click at #block-input"] (_,me) {
        if (!input_blocked) {
            handler.xcall("toggle_option","block-input");
            input_blocked = true;
            me.text = "Unblock user input"; // TEST 
        } else {
            handler.xcall("toggle_option","unblock-input");
            input_blocked = false;
            me.text = "Block user input";
        }
    }

    ["on click at menu#display-options>li"] (_, me) {
        if (me.id == "custom") {
            handle_custom_image_quality();
        } else if (me.attributes.hasClass("toggle-option")) {
            handler.toggle_option(me.id);
            toggleMenuState();
        } else if (!me.attributes.hasClass("selected")) {
            let type =  me.attributes["type"];
            if (type == "image-quality") {
                handler.xcall("save_image_quality",me.id);
            } else if (type == "view-style") {
                handler.xcall("save_view_style",me.id);
                adaptDisplay();
            }
            toggleMenuState();
        }
    }
}

function handle_custom_image_quality() {
    let tmp = handler.xcall("get_custom_image_quality");
    let bitrate0 = tmp[0] || 50;
    let quantizer0 = tmp.length > 1 ? tmp[1] : 100;
    msgbox("custom", "Custom Image Quality", "<div .form> \
          <div><input type=\"hslider\" style=\"width: 50%\" name=\"bitrate\" max=\"100\" min=\"10\" value=\"" + bitrate0 + "\"/ buddy=\"bitrate-buddy\"><b #bitrate-buddy>x</b>% bitrate</div> \
          <div><input type=\"hslider\" style=\"width: 50%\" name=\"quantizer\" max=\"100\" min=\"0\" value=\"" + quantizer0 + "\"/ buddy=\"quantizer-buddy\"><b #quantizer-buddy>x</b>% quantizer</div> \
      </div>", function(res=null) {
        if (!res) return;
        if (!res.bitrate) return;
        handler.xcall("save_custom_image_quality",res.bitrate, res.quantizer);
        toggleMenuState();
      });
}

function toggleMenuState() {
    let values = [];
    let q = handler.xcall("get_image_quality");
    if (!q) q = "balanced";
    values.push(q);
    let s = handler.xcall("get_view_style");
    if (!s) s = "original";
    values.push(s);
    for (let el of $$("menu#display-options>li")) {
        el.classList.toggle("selected", values.indexOf(el.id) >= 0);
    }
    for (let id of ["show-remote-cursor", "disable-audio", "disable-clipboard", "lock-after-session-end", "privacy-mode"]) {
        let el = $('#' + id); // TEST
        if (el) {
            el.classList.toggle("selected", handler.xcall("get_toggle_option",id));
        }
    }
}

if (is_osx) {
    $("header").content(<Header />);
    $("header").attributes["role"] = "window-caption"; // TODO 
} else {
    if (handler.is_file_transfer || handler.is_port_forward) {
        $("caption").content(<Header />);
    } else {
        $("div.window-toolbar").content(<Header />);
    }
    setWindowButontsAndIcon();
}

if (!(handler.is_file_transfer || handler.is_port_forward)) {
    $("header").style.setProperty("height","32px");
    if (!is_osx) {
        $("div.window-icon").style.setProperty("size","32px");
    }
}

handler.updatePi = function(v) {
    pi = v;
    header.componentUpdate();
    if (handler.is_port_forward) {
        view.state = Window.WINDOW_MINIMIZED;
    }
}

handler.switchDisplay = function(i) {
    pi.current_display = i;
    header.componentUpdate();
}

function updateWindowToolbarPosition() {
    if (is_osx) return;
    setTimeout(function() {
        let el = $("div.window-toolbar");
        let w1 = el.state.box("width", "border"); // TEST
        let w2 = $("header").state.box("width", "border");
        let x = (w2 - w1) / 2;
        el.style.setProperty("left",x + "px");
        el.style.setProperty("display","block")
    },1);
}

view.onsizechange = function() {
    // ensure size is done, so add timer
    setTimeout(function() {
        updateWindowToolbarPosition();
        adaptDisplay();
    },1);
};

handler.newMessage = function(text) {
    chat_msgs.push({text: text, name: pi.username || "", time: getNowStr()});
    startChat();
}

function sendMsg(text) {
    chat_msgs.push({text: text, name: "me", time: getNowStr()});
    handler.xcall("send_chat",text);
    if (chatbox) chatbox.refresh();
}

var chatbox;
function startChat() {
    if (chatbox) {
        chatbox.state = Window.WINDOW_SHOWN; // TODO TEST el.state
        chatbox.refresh(); // TODO el.refresh
        return;
    }
    let icon = handler.xcall("get_icon");
    let [sx, sy, sw, sh] = view.screenBox("workarea", "rectw"); // TEST
    let w = 300;
    let h = 400;
    let x = (sx + sw - w) / 2;
    let y = sy + 80;
    let params = {
        type: Window.FRAME_WINDOW,
        x: x,
        y: y,
        width: w,
        height: h,
        client: true,
        parameters: { msgs: chat_msgs, callback: sendMsg, icon: icon },
        caption: get_id(),
    };
    let html = handler.xcall("get_chatbox");
    if (html) params.html = html;
    else params.url = document.url("chatbox.html");
    chatbox = view.window(params); // TEST
}

handler.setConnectionType = function(secured, direct) {
    // TEST
    header.componentUpdate({
       secure_connection: secured,
       direct_connection: direct, 
    });
}
