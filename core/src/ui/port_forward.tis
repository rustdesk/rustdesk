class PortForward: Reactor.Component {
    function render() {
      var args = handler.get_args();
      var is_rdp = handler.is_rdp();
      if (is_rdp) {
        this.pfs = [["", "", "RDP"]];
        args = ["rdp"];
      } else if (args.length) {
        this.pfs = [args];
      } else {
        this.pfs = handler.get_port_forwards();
      }
      var pfs =  this.pfs.map(function(pf, i) {
        return <tr key={i} .value>
            <td>{is_rdp ? <button .button #new-rdp>New RDP</button> : pf[0]}</td>
            <td .right-arrow style="text-align: center; padding-left: 0">{args.length ? svg_arrow : ""}</td>
            <td>{pf[1] || "localhost"}</td>
            <td>{pf[2]}</td>
            {args.length ? "" : <td .remove>{svg_cancel}</td>}
        </tr>;
      });
      return <div #file-transfer><section>
        {pfs.length ? <div style="background: green; color: white; text-align: center; padding: 0.5em;">
          <span style="font-size: 1.2em">{translate('Listening ...')}</span><br/>
          <span style="font-size: 0.8em; color: #ddd">{translate('not_close_tcp_tip')}</span>
        </div> : ""}
        <table #port-forward>    
        <thead>    
          <tr>
            <th>{translate('Local Port')}</th>
            <th style="width: 1em" />
            <th>{translate('Remote Host')}</th>
            <th>{translate('Remote Port')}</th>
            {args.length ? "" : <th style="width: 6em">{translate('Action')}</th>}
          </tr>    
        </thead>      
        <tbody key={pfs.length}> 
            {args.length ? "" : 
            <tr>
                <td><input|number #port /></td>
                <td .right-arrow style="text-align: center">{svg_arrow}</td>
                <td><input|text #remote-host novalue="localhost" /></td>
                <td><input|number #remote-port /></td>
                <td style="margin:0;"><button .button #add>{translate('Add')}</button></td>
            </tr>
            }
            {pfs}
        </tbody>
      </table></section></div>;
    }
    
    event click $(#add) () {
      var port = ($(#port).value || "").toInteger() || 0;
      var remote_host = $(#remote-host).value || "";
      var remote_port = ($(#remote-port).value || "").toInteger() || 0;
      if (port <= 0 || remote_port <= 0) return;
      handler.add_port_forward(port, remote_host, remote_port);
      this.update();
    }

    event click $(#new-rdp) {
      handler.new_rdp();
    }

    event click $(.remove svg) (_, me) {
      var pf = this.pfs[me.parent.parent.index - 1];
      handler.remove_port_forward(pf[0]);
      this.update();
    }
}

function initializePortForward() 
{
    $(#file-transfer-wrapper).content(<PortForward />);
    $(#video-wrapper).style.set { visibility: "hidden", position: "absolute" };
    $(#file-transfer-wrapper).style.set { display: "block" };
}
