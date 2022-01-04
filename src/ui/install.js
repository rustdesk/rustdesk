function self.ready() {
    centerize(800, 600);
}

class Install: Reactor.Component {
    function render() {
        return <div .content>
            <div style="font-size: 2em;">{translate('Installation')}</div>
            <div style="margin: 2em 0;">{translate('Installation Path')} {": "}<input|text disabled value={view.install_path()} /></div>
            <div><button|checkbox #startmenu checked>{translate('Create start menu shortcuts')}</button></div>
            <div><button|checkbox #desktopicon checked>{translate('Create desktop icon')}</button></div>
            <div #aggrement .link style="margin-top: 2em;">{translate('End-user license agreement')}</div>
            <div>{translate('agreement_tip')}</div>
            <div style="height: 1px; background: gray; margin-top: 1em" />
            <div style="text-align: right;">
                <progress style={"color:" + color} style="display: none" /> 
                <button .button id="cancel" .outline style="margin-right: 2em;">{translate('Cancel')}</button>
                <button .button id="submit">{translate('Accept and Install')}</button>
            </div>
        </div>;
    }

    event click $(#cancel) {
        view.close();
    }

    event click $(#aggrement) {
        view.open_url("http://rustdesk.com/privacy");
    }

    event click $(#submit) {
        for (var el in $$(button)) el.state.disabled = true;
        $(progress).style.set{ display: "inline-block" };
        var args = "";
        if ($(#startmenu).value) {
            args += "startmenu ";
        }
        if ($(#desktopicon).value) {
            args += "desktopicon ";
        }
        view.install_me(args);
    }
}

$(body).content(<Install />);
