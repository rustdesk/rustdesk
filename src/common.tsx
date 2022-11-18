import { appWindow, WebviewWindow } from '@tauri-apps/api/window'
import { invoke } from "@tauri-apps/api"
import React, { KeyboardEvent } from 'react';
import { currentMonitor } from '@tauri-apps/api/window';
import { is_port_forward, is_file_transfer } from './remote';
import { closeMsgbox } from './msgbox';

// const [monitorWidth, monitorHeight, scaleFactor] = await invoke<
//         [number, number, number]
//       >("get_monitor_size")
const view = appWindow;
export const OS = await invoke<string>("get_os");
export const is_osx = OS == "OSX";
export const is_win = OS == "Windows";
export const is_linux = OS == "Linux";
export let is_xfce = false;

try { is_xfce = await invoke<boolean>("is_xfce"); } catch(e) {}

// function isEnterKey(evt) {
//     return (evt.keyCode == Event.VK_ENTER || 
//              (is_osx && evt.keyCode == 0x4C) ||
//               (is_linux && evt.keyCode == 65421));
// }
export const isEnterKey = (event: KeyboardEvent<HTMLImageElement>) => {
    return event.key === 'Enter'
}
// <div onKeyPress={isEnterKey}>{/** Some code */}</div>;

export const scaleFactor = async () => {
    
    const monitor = await currentMonitor();
    if (monitor == null) {
        console.log("monitor is null");
        return 0;
    }
    return monitor.scaleFactor
};

// TODO: on resolution change refrash scaleFactor

export const scaleIt = async (x: number) => {
    return x * await scaleFactor();
}

export const hashCode = (str: string) => {
  var hash = 160 << 16 + 114 << 8 + 91; 
  for (var i = 0; i < str.length; i += 1) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  return hash % 16777216;
}

export const intToRGB = (i: number, a: number = 1) => {
  return 'rgba(' + ((i >> 16) & 0xFF) + ', ' + ((i >> 8) & 0x7F)
    + ',' + (i & 0xFF) + ',' + a + ')';
}

export const string2RGB = (s: string, a = 1) => {
  return intToRGB(hashCode(s), a); 
}

export const getTime = () => {
    var now = new Date();
    return now.valueOf();
}

// function platformSvg(platform, color) {
//     platform = (platform || "").toLowerCase();
//     if (platform == "linux") {
//         return <svg viewBox="0 0 256 256">
//                     <g transform="translate(0 256) scale(.1 -.1)" fill={color}>
//                         <path d="m1215 2537c-140-37-242-135-286-278-23-75-23-131 1-383l18-200-54-60c-203-224-383-615-384-831v-51l-66-43c-113-75-194-199-194-300 0-110 99-234 244-305 103-50 185-69 296-69 100 0 156 14 211 54 26 18 35 19 78 10 86-18 233-24 335-12 85 10 222 38 269 56 9 4 19-7 29-35 20-50 52-64 136-57 98 8 180 52 282 156 124 125 180 244 180 380 0 80-28 142-79 179l-36 26 4 119c5 175-22 292-105 460-74 149-142 246-286 409-43 49-78 92-78 97 0 4-7 52-15 107-8 54-19 140-24 189-13 121-41 192-103 260-95 104-248 154-373 122zm172-112c62-19 134-80 163-140 15-31 28-92 41-193 27-214 38-276 57-304 9-14 59-74 111-134 92-106 191-246 236-334 69-137 115-339 101-451l-7-55-71 10c-100 13-234-5-265-36-54-55-85-207-82-412l1-141-51-17c-104-34-245-51-380-45-69 3-142 10-162 16-32 10-37 17-53 68-23 72-87 201-136 273-80 117-158 188-237 215-37 13-37 13-34 61 13 211 182 555 373 759 57 62 58 63 58 121 0 33-9 149-19 259-21 224-18 266 26 347 67 122 193 174 330 133zm687-1720c32-9 71-25 87-36 60-42 59-151-4-274-59-119-221-250-317-257-34-3-35-2-48 47-18 65-20 329-3 413 16 83 29 110 55 115 51 10 177 6 230-8zm-1418-80c79-46 187-195 247-340 41-99 43-121 12-141-39-25-148-30-238-10-142 32-264 112-307 202-20 41-21 50-10 87 24 83 102 166 192 207 54 25 53 25 104-5z"/>
//                         <path d="m1395 1945c-92-16-220-52-256-70-28-15-29-18-29-89 0-247 165-397 345-312 60 28 77 46 106 111 54 123 0 378-80 374-9 0-47-7-86-14zm74-156c15-69 14-112-5-159s-55-70-111-70c-48 0-78 20-102 68-15 29-41 131-41 159 0 9 230 63 242 57 3-2 11-27 17-55z"/>
//                     </g>
//                 </svg>;
//     }
//     if (platform == "mac os") {
//         return <svg viewBox="0 0 384 512">
//                     <path d="M318.7 268.7c-.2-36.7 16.4-64.4 50-84.8-18.8-26.9-47.2-41.7-84.7-44.6-35.5-2.8-74.3 20.7-88.5 20.7-15 0-49.4-19.7-76.4-19.7C63.3 141.2 4 184.8 4 273.5q0 39.3 14.4 81.2c12.8 36.7 59 126.7 107.2 125.2 25.2-.6 43-17.9 75.8-17.9 31.8 0 48.3 17.9 76.4 17.9 48.6-.7 90.4-82.5 102.6-119.3-65.2-30.7-61.7-90-61.7-91.9zm-56.6-164.2c27.3-32.4 24.8-61.9 24-72.5-24.1 1.4-52 16.4-67.9 34.9-17.5 19.8-27.8 44.3-25.6 71.9 26.1 2 49.9-11.4 69.5-34.3z" fill={color}/>
//                 </svg>;
//     }
//     if (platform == "android") {
//         return <svg xmlns="http://www.w3.org/2000/svg" width="553" height="553"><path fill="white" d="M77 179a33 33 0 0 0-25 10 33 33 0 0 0-9 24v143a33 33 0 0 0 10 24 33 33 0 0 0 24 10c9 0 17-3 24-10a33 33 0 0 0 10-24V213c0-9-4-17-10-24a33 33 0 0 0-24-10zM352 51l24-44c1-3 1-5-2-6-3-2-5-1-7 2l-24 43a163 163 0 0 0-133 0L186 3c-2-3-4-4-7-2-2 1-3 3-1 6l23 44c-24 12-43 29-57 51a129 129 0 0 0-21 72h307c0-26-7-50-21-72a146 146 0 0 0-57-51zm-136 63a13 13 0 0 1-10 4 13 13 0 0 1-12-13c0-4 1-7 3-9 3-3 6-4 9-4s7 1 10 4c2 2 3 5 3 9s-1 7-3 9zm140 0a12 12 0 0 1-9 4c-4 0-7-1-9-4a12 12 0 0 1-4-9c0-4 1-7 4-9 2-3 5-4 9-4a12 12 0 0 1 9 4c2 2 3 5 3 9s-1 7-3 9zM124 407c0 10 4 19 11 26s15 10 26 10h24v76c0 9 4 17 10 24s15 10 24 10c10 0 18-3 25-10s10-15 10-24v-76h45v76c0 9 4 17 10 24s15 10 25 10c9 0 17-3 24-10s10-15 10-24v-76h25a35 35 0 0 0 25-10c7-7 11-16 11-26V185H124v222zm352-228a33 33 0 0 0-24 10 33 33 0 0 0-10 24v143a34 34 0 0 0 34 34c10 0 18-3 25-10s10-15 10-24V213c0-9-4-17-10-24a33 33 0 0 0-25-10z"/></svg>;
//     }
//     return <svg viewBox="0 0 448 512">
//                 <path d="M0 93.7l183.6-25.3v177.4H0V93.7zm0 324.6l183.6 25.3V268.4H0v149.9zm203.8 28L448 480V268.4H203.8v177.9zm0-380.6v180.1H448V32L203.8 65.7z" fill={color}/>
//             </svg>;
// }


export const centerize = async (w: number, h: number) => {
    await invoke('centerize', {
        w: w,
        h: h
    })
}

// function setWindowButontsAndIcon(only_min=false) {
//     ...
// }

// function adjustBorder() {
//     ...
// }

// var svg_checkmark = <svg class="checkmark" viewBox="0 0 492 492"><path d="M484 105l-16-17a27 27 0 00-38 0L204 315 62 173c-5-5-12-7-19-7s-14 2-19 7L8 189a27 27 0 000 38l160 160v1l16 16c5 5 12 8 19 8 8 0 14-3 20-8l16-16v-1l245-244a27 27 0 000-38z"/></svg>;
// var svg_edit = <svg #edit viewBox="0 0 384 384">
//     <path d="M0 304v80h80l236-236-80-80zM378 56L328 6c-8-8-22-8-30 0l-39 39 80 80 39-39c8-8 8-22 0-30z"/>
// </svg>;
// var svg_eye = <svg viewBox="0 0 469.33 469.33">
// 	<path d="m234.67 170.67c-35.307 0-64 28.693-64 64s28.693 64 64 64 64-28.693 64-64-28.694-64-64-64z"/>
// 	<path d="m234.67 74.667c-106.67 0-197.76 66.346-234.67 160 36.907 93.653 128 160 234.67 160 106.77 0 197.76-66.347 234.67-160-36.907-93.654-127.89-160-234.67-160zm0 266.67c-58.88 0-106.67-47.787-106.67-106.67s47.787-106.67 106.67-106.67 106.67 47.787 106.67 106.67-47.787 106.67-106.67 106.67z"/>
// </svg>;
// var svg_send = <svg viewBox="0 0 448 448">
// <polygon points="0.213 32 0 181.33 320 224 0 266.67 0.213 416 448 224"/>
// </svg>;
// var svg_chat = <svg viewBox="0 0 511.07 511.07">
//     <path d="m74.39 480.54h-36.213l25.607-25.607c13.807-13.807 22.429-31.765 24.747-51.246-36.029-23.644-62.375-54.751-76.478-90.425-14.093-35.647-15.864-74.888-5.121-113.48 12.89-46.309 43.123-88.518 85.128-118.85 45.646-32.963 102.47-50.387 164.33-50.387 77.927 0 143.61 22.389 189.95 64.745 41.744 38.159 64.734 89.63 64.734 144.93 0 26.868-5.471 53.011-16.26 77.703-11.165 25.551-27.514 48.302-48.593 67.619-46.399 42.523-112.04 65-189.83 65-28.877 0-59.01-3.855-85.913-10.929-25.465 26.123-59.972 40.929-96.086 40.929zm182-420c-124.04 0-200.15 73.973-220.56 147.28-19.284 69.28 9.143 134.74 76.043 175.12l7.475 4.511-0.23 8.727c-0.456 17.274-4.574 33.912-11.945 48.952 17.949-6.073 34.236-17.083 46.99-32.151l6.342-7.493 9.405 2.813c26.393 7.894 57.104 12.241 86.477 12.241 154.37 0 224.68-93.473 224.68-180.32 0-46.776-19.524-90.384-54.976-122.79-40.713-37.216-99.397-56.888-169.71-56.888z"/>
// </svg>;
// var svg_keyboard = <svg viewBox="0 0 511.07 511.07"><path d="M491.979 217.631H110.205a48.41 48.41 0 0 1 .637-4.061c4.408-21.755 23.676-38.152 46.71-38.152h149.314c39.306 0 71.282-32.246 71.282-71.552 0-11.28-9.145-20.56-20.426-20.56s-20.426 9.077-20.426 20.359c0 3.419-.575 6.941-1.619 10.01-4.082 11.998-15.451 20.893-28.812 20.893H157.553c-46.995 0-85.535 36.766-88.331 83.064H20.021C8.739 217.631 0 226.296 0 237.578v170.345c0 11.28 8.739 20.773 20.021 20.773H491.98c11.28 0 20.021-9.492 20.021-20.773V237.578c-.001-11.282-8.74-19.947-20.022-19.947zm-20.83 170.213H40.851V258.482h430.298v129.362z"/><path d="M113.021 273.461H89.872c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h23.149c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426zM190.638 273.461h-23.149c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h23.149c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426zM268.255 273.461h-23.149c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h23.149c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426zM345.872 273.461h-23.149c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h23.149c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426zM423.489 273.461H400.34c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h23.149c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426zM113.021 325.206H89.872c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.425 20.426 20.425h23.149c11.28 0 20.426-9.145 20.426-20.425s-9.145-20.426-20.426-20.426zM423.489 325.206H400.34c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.425 20.426 20.425h23.149c11.28 0 20.426-9.145 20.426-20.425s-9.145-20.426-20.426-20.426zM345.872 329.291H167.489c-11.28 0-20.426 9.145-20.426 20.426s9.145 20.426 20.426 20.426h178.383c11.28 0 20.426-9.145 20.426-20.426s-9.145-20.426-20.426-20.426z"/></svg>;

// TODO:
// function scrollToBottom(el) {
//     var y = el.box(#height, #content) - el.box(#height, #client);
//     el.scrollTo(0, y);
// }

// TODO:
// function getNowStr() {
//     var now = new Date();
//     return String.printf("%02d:%02d:%02d", now.hour, now.minute, now.second);
// }

/******************** end of chatbox ****************************************/

/******************** start of msgbox ****************************************/
// var remember_password = false;
const msgbox = async (type: string, title: string, content: string, link="", callback: any=null, height=180, width=500, hasRetry=false, contentStyle="") => {
    // TODO:
    // $(body).scrollTo(0, 0);
    if (!type) {
        closeMsgbox();
        return;
    }
    var remember = false;
    try { remember = await invoke<boolean>("get_remember");} catch(e) {}
    var auto_login = false;
    try { auto_login = await invoke<string>("get_option", {key: "auto-login"}) != ''; } catch(e) {}
    width += is_xfce ? 50 : 0;
    height += is_xfce ? 50 : 0;

    if (type.indexOf("input-password") >= 0) {
        callback = async function (res: any) {
            if (!res) {
                view.close();
                return;
            }
            await invoke("login", {password: res.password, remember: res.remember}); 
            if (!(await is_port_forward())) {
              // Specially handling file transfer for no permission hanging issue (including 60ms
              // timer in setPermission.
              // For wrong password input hanging issue, we can not use handler.msgbox.
              // But how about wrong password for file transfer?
              if (await is_file_transfer()) handler_msgbox("connecting", "Connecting...", "Logging in...");
              else msgbox("connecting", "Connecting...", "Logging in...");
            }
        };
    } else if (type.indexOf("custom") < 0 && !is_port_forward && !callback) {
        callback = function() { view.close(); }
    }
    // TODO:
    // $(#msgbox).content(<MsgboxComponent width={width} height={height} auto_login={auto_login} type={type} title={title} content={content} link={link} remember={remember} callback={callback} contentStyle={contentStyle} hasRetry={hasRetry} />);
}

export const connecting = async () => {
    handler_msgbox("connecting", "Connecting...", "Connection in progress. Please wait.");
}

export const handler_msgbox = async (type: string, title: string, text: string, link = "", hasRetry=false) => {
    // crash somehow (when input wrong password), even with small time, for example, 1ms
    // self.timer(60ms, function() { msgbox(type, title, text, link, null, 180, 500, hasRetry); });
    setInterval(
        () => msgbox(type, title, text, link, null, 180, 500, hasRetry),
        60
    );

}

var reconnectTimeout = 1000;
const handler_msgbox_retry = async (type: string, title: string, text: string, link: string, hasRetry: boolean) => {
    handler_msgbox(type, title, text, link, hasRetry);
    if (hasRetry) {
        // self.timer(0, retryConnect);
        setInterval(retryConnect, 0);
        // self.timer(reconnectTimeout, retryConnect);
        setInterval(retryConnect, reconnectTimeout);
        reconnectTimeout *= 2;
    } else {
        reconnectTimeout = 1000;
    }
}

export const retryConnect = async (cancelTimer=false) => {
    if (cancelTimer) {
        // self.timer(0, retryConnect);
        setInterval(
            () => retryConnect(),
            0
        );
    }
    if (!is_port_forward) connecting();
    await invoke("reconnect");
}
// /******************** end of msgbox ****************************************/

// TODO: React component
// function Progress()
// {
//     var _val: any;
//     var pos = -0.25;

//     function step() {
//         if( _val !== undefined ) {
//             // TODO: 
//             // this.refresh(); 
//             return false; 
//         }
//         pos += 0.02;
//         if( pos > 1.25)
//             pos = -0.25;
//         // TODO: 
//         // this.refresh();
//         return true;
//     }
    
//     // TODO:
//     // function paintNoValue(gfx: any)
//     // {
//     //     var (w,h) = this.box(#dimension,#inner);
//     //     var x = pos * w;
//     //     w = w * 0.25;
//     //     gfx.fillColor( this.style#color )
//     //          .pushLayer(#inner-box)
//     //          .rectangle(x,0,w,h)
//     //          .popLayer();
//     //     return true;
//     // }

//     // TODO:
//     // this[#value] = property(v) {
//     //     get return _val;
//     //     set {
//     //         _val = undefined;
//     //         pos = -0.25;
//     //         this.paintContent = paintNoValue;
//     //         this.animate(step);
//     //         this.refresh();
//     //     }
//     }

//     // TODO:
//     // this.value = "";
// }

// var svg_eye_cross = <svg viewBox="0 -21 511.96 511">
// <path d="m506.68 261.88c7.043-16.984 7.043-36.461 0-53.461-41.621-100.4-140.03-165.27-250.71-165.27-46.484 0-90.797 11.453-129.64 32.191l-68.605-68.609c-8.3438-8.3398-21.824-8.3398-30.168 0-8.3398 8.3398-8.3398 21.824 0 30.164l271.49 271.49 86.484 86.488 68.676 68.672c4.1797 4.1797 9.6406 6.2695 15.102 6.2695 5.4609 0 10.922-2.0898 15.082-6.25 8.3438-8.3398 8.3438-21.824 0-30.164l-62.145-62.145c36.633-27.883 66.094-65.109 84.438-109.38zm-293.91-100.1c12.648-7.5742 27.391-11.969 43.199-11.969 47.062 0 85.332 38.273 85.332 85.336 0 15.805-4.3945 30.547-11.969 43.199z"/>
// <path d="m255.97 320.48c-47.062 0-85.336-38.273-85.336-85.332 0-3.0938 0.59766-6.0195 0.91797-9.0039l-106.15-106.16c-25.344 24.707-46.059 54.465-60.117 88.43-7.043 16.98-7.043 36.457 0 53.461 41.598 100.39 140.01 165.27 250.69 165.27 34.496 0 67.797-6.3164 98.559-18.027l-89.559-89.559c-2.9844 0.32031-5.9062 0.91797-9 0.91797z"/>
// </svg>;

class PasswordComponent extends React.Component {
    constructor(props: any) {
        super(props);
        this.state = {
            date: new Date(),
            visible: false,
            value: "",
            name: "password",
        };
        
      }

    // TODO: test it
    // <PasswordComponent value="12345", name="Sara" />;

    // TODO:
    // render() {
    //     return <div .password>
    //         <input name={this.name} value={this.value} type={this.visible ? "text" : "password"} .outline-focus />
    //         {this.visible ? svg_eye_cross : svg_eye}
    //     </div>;
    // }

    // TODO:
    // event click $(svg) {
    //     var el = this.$(input);
    //     var value = el.value;
    //     var start = el.xcall(#selectionStart) || 0;
    //     var end = el.xcall(#selectionEnd);
    //     this.update({ visible: !this.visible });
    //     var me = this;
    //     self.timer(30ms, function() {
    //         var el = me.$(input);
    //         view.focus = el;
    //         el.value = value;
    //         el.xcall(#setSelection, start, end);
    //     });
    // }
}

// type: #post, #get, #delete, #put
export const httpRequest = async (url: string, type: string, params: any, _onSuccess: any, _onError: any, headers="") => {
    // TODO: define #post literal
    if (type != "#post") {
        console.log("Error. Only post ok.");
    }
    const body = JSON.stringify(params);
    await invoke("post_request", {url: url, body: body, header: headers});
    async function check_status() {
        var status = await invoke<string>("get_async_job_status");
        if (status == " ") setInterval(() => check_status(), 100);
        else {
            try {
                var data = JSON.parse(status);
                _onSuccess(data);
            } catch (e) {
                _onError(status, 0);
            }
        }
    }
    check_status();
}

export const isReasonableSize = async(r: any) => {
    var x = r[0];
    var y = r[1];
    var n = scaleIt(3200);
    return !(x < -n || x > n || y < -n || y > n);
}

// TODO: 
// export const awake = async() =>{
//     view.windowState = View.WINDOW_SHOWN;
//     view.focus = self;
// }

