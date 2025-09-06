function self.ready() {
    centerize(scaleIt(800), scaleIt(600));
}

var install_path = "";

class Install: Reactor.Component {
    function render() {
        const install_options = JSON.parse(view.install_options());
        const desktop_icon = { checked: install_options?.DESKTOPSHORTCUTS == '0' ? false : true };
        const startmenu_shortcuts = { checked: install_options?.STARTMENUSHORTCUTS == '0' ? false : true };
        return <div .content>
            <div style="font-size: 2em;">{translate('Installation')}</div>
            <div style="margin: 2em 0;">{translate('Installation Path')} {": "}<input|text disabled value={view.install_path()} #path_input />
            <button .button .outline #path style="margin-left: 1em">{translate('Change Path')}</button>
            </div>
            <div><button|checkbox #startmenu {startmenu_shortcuts}>{translate('Create start menu shortcuts')}</button></div>
            <div><button|checkbox #desktopicon {desktop_icon}>{translate('Create desktop icon')}</button></div>
            <div #agreement .link style="margin-top: 2em;">{translate('End-user license agreement')}</div>
            <div>{translate('agreement_tip')}</div>
            <div style="height: 1px; background: gray; margin-top: 1em" />
            <div style="text-align: right;">
                <progress style={"color:" + color} style="display: none" /> 
                <button .button id="cancel" .outline style="margin-right: 2em;">{translate('Cancel')}</button>
                <button .button id="submit">{translate('Accept and Install')}</button>
                {handler.show_run_without_install() && <button .button #run-without-install .outline style="margin-left: 2em;">
                    {translate('Run without install')}
                </button>}
            </div>
        </div>;
    }

    event click $(#cancel) {
        view.close();
    }

    event click $(#run-without-install) {
        handler.run_without_install();
    }

    event click $(#path) {
        install_path = view.selectFolder() || "";
        if (install_path) {
          install_path = install_path.urlUnescape();
          install_path = install_path.replace("file://", "").replace("/", "\\");
          if (install_path[install_path.length - 1] != "\\") install_path += "\\";
          install_path += handler.get_app_name();
          $(#path_input).value = install_path;
        }
    }

    event click $(#agreement) {
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
        view.install_me(args, install_path);
    }
}

$(body).content(<Install />);
