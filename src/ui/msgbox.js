import {$,$$} from "@sciter"; //TEST $$ import
const view = Window.this;
var type, title, text, getParams, remember, retry, callback, contentStyle;
var my_translate; // TEST 

function updateParams(params) {
    type = params.type;
    title = params.title;
    text = params.text;
    getParams = params.getParams;
    remember = params.remember;
    callback = params.callback;
    my_translate = params.translate;
    retry = params.retry;
    contentStyle = params.contentStyle;
    console.log("params",type,title,text,getParams,remember,callback,my_translate,retry,contentStyle)
    try { text = translate_text(text); } catch (e) {}
    if (retry > 0) {
        setTimeout(()=>view.close({ reconnect: true }),retry * 1000);// TEST
    }
}

function translate_text(text) {
    if (text.indexOf('Failed') == 0 && text.indexOf(': ') > 0) {
        let fds = text.split(': ');
        for (let i = 0; i < fds.length; ++i) {
            fds[i] = my_translate(fds[i]);
        }
        text = fds.join(': ');
    }
    return text;
}

var params = view.parameters; // TEST
updateParams(params);

var body;

class Body extends Element {
    this() {
        body = this;
    }

    getIcon(color) {
        if (type == "input-password") {
            return <svg viewBox="0 0 505 505"><circle cx="252.5" cy="252.5" r="252.5" fill={color}/><path d="M271.9 246.1c29.2 17.5 67.6 13.6 92.7-11.5 29.7-29.7 29.7-77.8 0-107.4s-77.8-29.7-107.4 0c-25.1 25.1-29 63.5-11.5 92.7L118.1 347.4l26.2 26.2 26.4 26.4 10.6-10.6-10.1-10.1 9.7-9.7 10.1 10.1 10.6-10.6-10.1-10 9.7-9.7 10.1 10.1 10.6-10.6-26.4-26.3 76.4-76.5z" fill="#fff"/><circle cx="337.4" cy="154.4" r="17.7" fill={color}/></svg>;
        }
        if (type == "connecting") {
            return <svg viewBox="0 0 300 300"><g fill={color}><path d="m221.76 89.414h-143.51c-1.432 0-2.594 1.162-2.594 2.594v95.963c0 1.432 1.162 2.594 2.594 2.594h143.51c1.432 0 2.594-1.162 2.594-2.594v-95.964c0-1.431-1.162-2.593-2.594-2.593z"/><path d="m150 0c-82.839 0-150 67.161-150 150s67.156 150 150 150 150-67.163 150-150-67.164-150-150-150zm92.508 187.97c0 11.458-9.29 20.749-20.749 20.749h-47.144v11.588h23.801c4.298 0 7.781 3.483 7.781 7.781s-3.483 7.781-7.781 7.781h-96.826c-4.298 0-7.781-3.483-7.781-7.781s3.483-7.781 7.781-7.781h23.801v-11.588h-47.145c-11.458 0-20.749-9.29-20.749-20.749v-95.963c0-11.458 9.29-20.749 20.749-20.749h143.51c11.458 0 20.749 9.29 20.749 20.749v95.963z"/></g><path d="m169.62 154.35c-5.0276-5.0336-11.97-8.1508-19.624-8.1508-7.6551 0-14.597 3.1172-19.624 8.1508l-11.077-11.091c7.8656-7.8752 18.725-12.754 30.701-12.754s22.835 4.8788 30.701 12.754l-11.077 11.091zm-32.184 7.0728 12.56 12.576 12.56-12.576c-3.2147-3.2172-7.6555-5.208-12.56-5.208-4.9054 0-9.3457 1.9908-12.56 5.208zm12.56-39.731c14.403 0 27.464 5.8656 36.923 15.338l11.078-11.091c-12.298-12.314-29.276-19.94-48-19.94-18.724 0-35.703 7.626-48 19.94l11.077 11.091c9.4592-9.4728 22.52-15.338 36.923-15.338z" fill="#fff"/></svg>;
        }
        if (type == "success") {
            return <svg viewBox="0 0 512 512"><circle cx="256" cy="256" r="256" fill={color} /><path fill="#fff" d="M235.472 392.08l-121.04-94.296 34.416-44.168 74.328 57.904 122.672-177.016 46.032 31.888z"/></svg>;
        }
        if (type.indexOf("error") >= 0 || type == "re-input-password") {
            return <svg viewBox="0 0 512 512"><ellipse cx="256" cy="256" rx="256" ry="255.832" fill={color}/><g fill="#fff"><path d="M376.812 337.18l-39.592 39.593-201.998-201.999 39.592-39.592z"/><path d="M376.818 174.825L174.819 376.824l-39.592-39.592 201.999-201.999z"/></g></svg>;
        }
        return <span />;
    }

    getInputPasswordContent() {
        var ts = remember ? { checked: true } : {};
        // TODO <button type="checkbox" ... 
        // <div><button|checkbox(remember) {ts}>{my_translate('Remember password')}</button></div>
        return <div class="form">
            <div>{my_translate('Please enter your password')}</div>
            <PasswordComponent />
            <div><button type="checkbox">{my_translate('Remember password')}</button></div>
        </div>;
    }

    getContent() {
        if (type == "input-password") {
            return this.getInputPasswordContent();
        }
        return text;
    }

    getColor() {
        if (type == "input-password") {
            return "#AD448E";
        }
        if (type == "success") {
            return "#32bea6";
        }
        if (type.indexOf("error") >= 0 || type == "re-input-password") {
            return "#e04f5f";
        }
        return "#2C8CFF";
    }

    hasSkip() {
        return type.indexOf("skip") >= 0;
    }

    render() {
        let color = this.getColor();
        let icon = this.getIcon(color);
        let content = this.getContent();
        let hasCancel = type.indexOf("error") < 0 && type != "success" && type.indexOf("nocancel") < 0;
        let hasOk = type != "connecting" && type.indexOf("nook") < 0;
        let hasClose = type.indexOf("hasclose") >= 0;
        let show_progress = type == "connecting";
        this.style.setProperty("border",(color + " solid 1px"));
        setTimeout(()=>{
            if (typeof content == "string")
                this.$("#content").html = my_translate(content);
            else
                this.$("#content").content(content);
        },1);
        return (
        <div style="size: *">
            <header style={"height: 2em; background: " + color}>
                <caption role="window-caption">{my_translate(title)}</caption>
            </header>
            <div style="padding: 1em 2em; size: *;">
                <div style="height: *; flow: horizontal">
                    {icon && <div style="height: *; margin: * 0; padding-right: 2em;">{icon}</div>}
                    <div id="content" style={contentStyle || "size: *; margin: * 0;"} />
                </div>
                <div style="text-align: right;">
                    <span id="error" style="display:inline-block; max-width: 260px; font-size:12px;" />
                    <progress id="progress" style={"color:" + color + "; display: " + (show_progress ? "inline-block" : "none")} />
                    {hasCancel || retry ? <button id="cancel" class="button outline">{my_translate(retry ? "OK" : "Cancel")}</button> : ""}
                    {this.hasSkip() ? <button id="skip" class="button outline">{my_translate('Skip')}</button> : ""}
                    {hasOk || retry ? <button id="submit" class="button" >{my_translate(retry ? "Retry" : "OK")}</button> : ""}
                    {hasClose ? <button id="cancel" class="button outline">{my_translate('Close')}</button> : ""}
                </div>
            </div>
        </div>);
    }

    // TEST
    ["on click at .custom-event"](_, me) {
        if (callback) callback(me);
    }

    ["on click at button#cancel"]() {
        view.close();
        if (callback) callback(null);
    }
    
    ["on click at button#skip"]() {
        let values = getValues();
        values.skip = true;
        view.close(values);
        if (callback) callback(values);
    }

    ["on click at button#submit"](){
        if (type == "error") {
            if (retry) {
                view.close({ reconnect: true });
            } else {
                view.close();
                if (callback) callback(null);
            }
            return;
        }
        if (type == "re-input-password") {
            type = "input-password";
            body.update();
            set_outline_focus();
            return;
        }
        var values = getValues();
        if (callback) {
            var err = callback(values, show_progress);
            if (err && !err.trim()) {
                return;
            }
            if (err) {
                show_progress(false, err);
                return;
            }
        }
        view.close(values);
    }
    ["on keydown"] (evt) { // TEST
        if (!evt.shortcutKey) {
            // TODO TEST Windows/Mac
            if (evt.code == "KeyRETURN") {
                submit();
            }
            if (evt.code == "KeyESCAPE") {
                cancel();
            }
        }
    }
}

function show_progress(show=1, err="") {
    if (show == -1) {
        view.close()
        return;
    }
    $("#progress").style.setProperty("display",show ? "inline-block" : "none");
    $("#error").text = err;
}

function submit() {
    if ($("button#submit")) {
        $("button#submit").click(); // TEST
    }
}

function cancel() {
    if ($("button#cancel")) {
        $("button#cancel").click();
    }
}

function getValues() {
    let values = { type: type };
    for (let el of $$(".form input")) {
        values[el.getAttribute("name")] = el.value;
    }
    for (let el of $$(".form textarea")) {
        values[el.getAttribute("name")] = el.value;
    }
    for (let el of $$(".form button")) {
        values[el.getAttribute("name")] = el.value;
    }
    if (type == "input-password") {
        values.password = (values.password || "").trim();
        if (!values.password) {
            return;
        }
    }
    return values;
}

function set_outline_focus() {
    setTimeout(function() {
        let el = $(".outline-focus");
        if (el) view.focus = el;
        else {
            el = $("#submit");
            if (el) view.focus = el;
        }
    },30);
}

set_outline_focus();

// checkParams
setInterval(function() {
    let tmp = getParams();
    if (!tmp || !tmp.type) {
        view.close("!alive");
        return;
    } else if (tmp != params) {
        params = tmp;
        updateParams(params);
        body.update();
        set_outline_focus();
    }
},30);

$("body").content(<Body />);