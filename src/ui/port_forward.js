import { translate } from "./common.js";

class PortForward extends Element {
    render() {
      let args = handler.xcall("get_args");
      let is_rdp = handler.xcall("is_rdp");
      if (is_rdp) {
        this.pfs = [["", "", "RDP"]];
        args = ["rdp"];
      } else if (args.length) {
        this.pfs = [args];
      } else {
        this.pfs = handler.xcall("get_port_forwards");
      }
      let pfs =  this.pfs.map(function(pf, i) {
        return (<tr key={i} class="value">
            <td>{is_rdp ? <button class="button" id="new-rdp">New RDP</button> : pf[0]}</td>
            <td class="right-arrow" style="text-align: center; padding-left: 0">{args.length ? svg_arrow : ""}</td>
            <td>{pf[1] || "localhost"}</td>
            <td>{pf[2]}</td>
            {args.length ? "" : <td class="remove">{svg_cancel}</td>}
        </tr>);
      });
      return <div id="file-transfer"><section>
        {pfs.length ? <div style="background: green; color: white; text-align: center; padding: 0.5em;">
          <span style="font-size: 1.2em">{translate('Listening ...')}</span><br/>
          <span style="font-size: 0.8em; color: #ddd">{translate('not_close_tcp_tip')}</span>
        </div> : ""}
        <table id="port-forward">    
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
                <td><input type="number" id="port" /></td>
                <td class="right-arrow" style="text-align: center">{svg_arrow}</td>
                <td><input type="text" id="remote-host" novalue="localhost" /></td>
                <td><input type="number" id="remote-port" /></td>
                <td style="margin:0;"><button class="button" id="add">{translate('Add')}</button></td>
            </tr>
            }
            {pfs}
        </tbody>
      </table></section></div>;
    }
    
    ["on click at #add"] () {
      let port = ($("#port").value || "").toInteger() || 0; // TODO toInteger
      let remote_host = $("#remote-host").value || "";
      let remote_port = ($("#remote-port").value || "").toInteger() || 0; // TODO toInteger
      if (port <= 0 || remote_port <= 0) return;
      handler.xcall("add_port_forward",port, remote_host, remote_port);
      this.componentUpdate();
    }

    ["on click at #new-rdp"] () {
      handler.xcall("new_rdp");
    }

    ["on click at .remove svg"](_, me) {
      let pf = this.pfs[me.parentElement.parentElement.index - 1];
      handler.xcall("remove_port_forward",pf[0]);
      this.componentUpdate();
    }
}

export function initializePortForward() 
{
    $("#file-transfer-wrapper").content(<PortForward />);
    $("#video-wrapper").style.setProperty("visibility","hidden");
    $("#video-wrapper").style.setProperty("position","absolute")
    $("#file-transfer-wrapper").style.setProperty("display","block");
}
