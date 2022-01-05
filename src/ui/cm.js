import { handler,view,is_osx,string2RGB,adjustBorder,svg_chat,translate,ChatBox,getNowStr,setWindowButontsAndIcon,is_linux } from "./common.js";
import {$} from "@sciter";
// TODO in sciterjs window-frame
// view.windowFrame = is_osx ? #extended : #solid;

var body;
var connections = [];
var show_chat = false;

class Body extends Element {
    cur = 0;

    this() {
        body = this;
    }

    render() {
        if (connections.length == 0) return <div />;
        let c = connections[this.cur];
        this.connection = c;
        this.cid = c.id;
        let auth = c.authorized;
        let callback = (msg)=> {
            this.sendMsg(msg);
        };
        setTimeout(adaptSize, 1);
        let right_style = show_chat ? "" : "display: none";
        return (<div class="content">
            <div class="left-panel">
                <div class="icon-and-id">
                    <div class="icon" style={"background: " + string2RGB(c.name, 1)}>
                    {c.name[0].toUpperCase()}
                    </div>
                    <div>
                        <div class="id" style="font-weight: bold; font-size: 1.2em;">{c.name}</div>
                        <div class="id">({c.peer_id})</div>
                        <div style="margin-top: 1.2em">{translate('Connected')} {" "} <span id="time">{getElaspsed(c.time)}</span></div>
                    </div>
                </div>
                <div />
                {c.is_file_transfer || c.port_forward ? "" : <div>{translate('Permissions')}</div>}
                {c.is_file_transfer || c.port_forward ? "" : <div class="permissions">
                    <div class={!c.keyboard ? "disabled" : ""} title={translate('Allow using keyboard and mouse')}><icon class="keyboard" /></div>
                    <div class={!c.clipboard ? "disabled" : ""} title={translate('Allow using clipboard')}><icon class="clipboard" /></div>
                    <div class={!c.audio ? "disabled" : ""} title={translate('Allow hearing sound')}><icon class="audio" /></div>
                </div>}
                {c.port_forward ? <div>Port Forwarding: {c.port_forward}</div> : ""}
                <div style="size:*"/>
                <div class="buttons">
                     {auth ? "" : <button class="button" tabindex="-1" id="accept">{translate('Accept')}</button>}
                     {auth ? "" : <button class="button" tabindex="-1" class="outline" id="dismiss">{translate('Dismiss')}</button>}
                     {auth ? <button class="button" tabindex="-1" id="disconnect">{translate('Disconnect')}</button> : ""}
                </div>
                {c.is_file_transfer || c.port_forward ? "" : <div class="chaticon">{svg_chat}</div>}
            </div>
            <div class="right-panel" style={right_style}>
                {c.is_file_transfer || c.port_forward ? "" : <ChatBox msgs={c.msgs} callback={callback} />}
            </div>
        </div>);
    }

    sendMsg(text) {
        if (!text) return;
        let { cid, connection } = this;
        checkClickTime(function() {
            connection.msgs.push({ name: "me", text: text, time: getNowStr()});
            handler.xcall("send_msg",cid, text);
            body.componentUpdate();
        });
    }

    ["on click at icon.keyboard"](e) {
        let { cid, connection } = this;
        checkClickTime(function() {
            connection.keyboard = !connection.keyboard;
            body.componentUpdate();
            handler.xcall("switch_permission",cid, "keyboard", connection.keyboard);
        });
    }

    ["on click at icon.clipboard"]() {
        let { cid, connection } = this;
        checkClickTime(function() {
            connection.clipboard = !connection.clipboard;
            body.componentUpdate();
            handler.xcall("switch_permission",cid, "clipboard", connection.clipboard);
        });
    }

    ["on click at icon.audio"]() {
        let { cid, connection } = this;
        checkClickTime(function() {
            connection.audio = !connection.audio;
            body.componentUpdate();
            handler.xcall("switch_permission",cid, "audio", connection.audio);
        });
    }

    ["on click at button#accept"]() {
        let { cid, connection } = this;
        checkClickTime(function() {
            connection.authorized = true;
            body.componentUpdate();
            handler.xcall("authorize",cid);
            setTimeout(()=>view.state = Window.WINDOW_MINIMIZED,30);
        });
    }

    ["on click at button#dismiss"]() {
        let cid = this.cid;
        checkClickTime(function() {
            handler.close(cid); // TEST
        });
    }

    ["on click at button#disconnect"]() {
        let cid = this.cid;
        checkClickTime(function() {
            handler.close(cid); // TEST
        });
    }
    ["on click at div.chaticon"]() {
        checkClickTime(function() {
            show_chat = !show_chat;
            adaptSize();
        });
    }
}

$("body").content(<Body />);

var header;

class Header extends Element {
    this() {
        header = this;
    }

    render() {
        let me = this;
        let conn = connections[body.cur];
        if (conn && conn.unreaded > 0) {
            let el = this.select("#unreaded" + conn.id); // TODO select
            if (el) el.style.setProperty("display","inline-block");
            setTimeout(function() {
                conn.unreaded = 0;
                let el = this.select("#unreaded" + conn.id); // TODO
                if (el) el.style.setProperty("display","none");
            },300);
        }
        let tabs = connections.map((c, i)=> this.renderTab(c, i));
        return (<div class="tabs-wrapper"><div class="tabs">
            {tabs}
            </div>
            <div class="tab-arrows">
                <span class="left-arrow">&lt;</span>
                <span class="right-arrow">&gt;</span>
            </div>
        </div>);
    }

    renderTab(c, i) {
        let cur = body.cur;
        return (<div class={i == cur ? "active-tab tab" : "tab"}>
            {c.name}
            {c.unreaded > 0 ? <span class="unreaded" id={"unreaded" + c.id}>{c.unreaded}</span> : ""}
        </div>);
    }

    update_cur(idx) {
        checkClickTime(function(){
            body.cur = idx;
            update();
            setTimeout(adjustHeader,1);
        });
    }

    ["on click at div.tab"] (_, me) {
        let idx = me.index;
        if (idx == body.cur) return;
        this.update_cur(idx);
    }

    ["on click at span.left-arrow"]() {
        let cur = body.cur;
        if (cur == 0) return;
        this.update_cur(cur - 1);
    }

    ["on click at span.right-arrow"]() {
        let cur = body.cur;
        if (cur == connections.length - 1) return;
        this.update_cur(cur + 1);
    }
}

if (is_osx) {
    $("header").content(<Header />);
    $("header").attributes["role"] = "window-caption"; // TODO 
} else {
    $("div.window-toolbar").content(<Header />);
    setWindowButontsAndIcon(true);
}

function checkClickTime(callback) {
    callback();
}

function adaptSize() {
    $("div.right-panel").style.setProperty("display",show_chat ? "block" : "none");
    let el = $("div.chaticon");
    if (el) el.classList.toggle("active", show_chat);
    let [x, y, w, h] = view.state.box("rectw", "border", "screen");
    if (show_chat && w < 600) {
        view.move(x - (600 - w), y, 600, h);
    } else if (!show_chat && w > 450) {
        view.move(x + (w - 300), y, 300, h);
    }
}

function update() {
    header.componentUpdate();
    body.componentUpdate();
}

function bring_to_top(idx=-1) {
    if (view.state == Window.WINDOW_HIDDEN || view.state == Window.WINDOW_MINIMIZED) {
        if (is_linux) {
            view.focus = $("body");
        } else {
            view.state = Window.WINDOW_SHOWN;
        }
        if (idx >= 0) body.cur = idx;
    } else {
        view.isTopmost = true; // TEST
        view.isTopmost = false; // TEST
    }
}

handler.addConnection = function(id, is_file_transfer, port_forward, peer_id, name, authorized, keyboard, clipboard, audio) {
    let conn;
    connections.map(function(c) {
        if (c.id == id) conn = c;
    });
    if (conn) {
        conn.authorized = authorized;
        update();
        return;
    }
    if (!name) name = "NA";
    connections.push({
        id: id, is_file_transfer: is_file_transfer, peer_id: peer_id,
        port_forward: port_forward,
        name: name, authorized: authorized, time: new Date(),
        keyboard: keyboard, clipboard: clipboard, msgs: [], unreaded: 0,
        audio: audio,
    });
    body.cur = connections.length - 1;
    bring_to_top();
    update();
    setTimeout(adjustHeader,1);
    if (authorized) {
        setTimeout(()=>view.state = Window.WINDOW_MINIMIZED,3000);
    }
}

handler.removeConnection = function(id) {
    let i = -1;
    connections.map(function(c, idx) {
        if (c.id == id) i = idx;
    });
    connections.splice(i, 1);
    if (connections.length == 0) {
        handler.xcall("exit");
    } else {
        if (body.cur >= i && body.cur > 0) body.cur -= 1;
        update();
    }
}

handler.newMessage = function(id, text) { 
    let idx = -1;
    connections.map(function(c, i) {
        if (c.id == id) idx = i;
    });
    let conn = connections[idx];
    if (!conn) return;
    conn.msgs.push({name: conn.name, text: text, time: getNowStr()});
    bring_to_top(idx);
    if (idx == body.cur) show_chat = true;
    conn.unreaded += 1;
    update();
}

handler.awake = function() {
    view.state = Window.WINDOW_SHOWN;
    view.focus = $("body");
}

// TEST
// view << event statechange {
//     adjustBorder();
// }
view.on("statechange",()=>{
    adjustBorder();
})

document.on("ready",()=>{
    adjustBorder();
    let [sw, sh] = view.screenBox("workarea", "dimension");
    let w = 300;
    let h = 400;
    view.move(sw - w, 0, w, h);
})

document.on("unloadequest",(evt)=>{
    view.state = Window.WINDOW_HIDDEN;
    console.log("cm unloadequest")
    evt.preventDefault(); // prevent unloading TEST
})

function getElaspsed(time) {
    // let now = new Date();
    // let seconds = Date.diff(time, now, #seconds);
    // let hours = seconds / 3600;
    // let days = hours / 24;
    // hours = hours % 24;
    // let minutes = seconds % 3600 / 60;
    // seconds = seconds % 60;
    // let out = String.printf("%02d:%02d:%02d", hours, minutes, seconds);
    // if (days > 0) {
    //     out = String.printf("%d day%s %s", days, days > 1 ? "s" : "", out);
    // }
    let out = "TIME TODO" + new Date(); // TODO
    return out;
}

// updateTime
setInterval(function() {
    let el = $("#time");
    if (el) {
        let c = connections[body.cur];
        if (c) {
            el.text = getElaspsed(c.time);
        }
    }
},1000);


function adjustHeader() {
    let hw = $("header").state.box("width");
    let tabswrapper = $("div.tabs-wrapper");
    let tabs = $("div.tabs");
    let arrows = $("div.tab-arrows");
    if (!arrows) return;
    let n = connections.length;
    let wtab = 80;
    let max = hw - 98;
    let need_width = n * wtab + 2; // include border of active tab
    if (need_width < max) {
        arrows.style.setProperty("display","none");
        tabs.style.setProperty("width",need_width);
        tabs.style.setProperty("margin-left",0);
        tabswrapper.style.setProperty("width",need_width);
    } else {
        let margin = (body.cur + 1) * wtab - max + 30;
        if (margin < 0) margin = 0;
        arrows.style.setProperty("display","block");
        tabs.style.setProperty("width",(max - 20 + margin) + 'px');
        tabs.style.setProperty("margin-left",-margin + 'px');
        tabswrapper.style.setProperty("width",(max + 10) + 'px');
    }
}

document.onsizechange = ()=>{
    console.log("cm onsizechange");
    adjustHeader();
}

// handler.addConnection(0, false, 0, "", "test1", true, false, false, false);
// handler.addConnection(1, false, 0, "", "test2--------", true, false, false, false);
// handler.addConnection(2, false, 0, "", "test3", true, false, false, false);
// handler.newMessage(0, 'h');
