import { handler,svg_send,translate,msgbox } from "./common.js";
import {$} from "@sciter";

var remote_home_dir;

const svg_add_folder = (<svg viewBox="0 0 443.29 443.29">
<path d="m277.06 332.47h27.706v-55.412h55.412v-27.706h-55.412v-55.412h-27.706v55.412h-55.412v27.706h55.412z"/>
<path d="m415.59 83.118h-202.06l-51.353-51.353c-2.597-2.597-6.115-4.058-9.794-4.058h-124.68c-15.274-1e-3 -27.706 12.431-27.706 27.705v332.47c0 15.273 12.432 27.706 27.706 27.706h387.88c15.273 0 27.706-12.432 27.706-27.706v-277.06c0-15.274-12.432-27.706-27.706-27.706zm0 304.76h-387.88v-332.47h118.94l51.354 51.353c2.597 2.597 6.115 4.058 9.794 4.058h207.79z"/>
</svg>);
const svg_trash = (<svg viewBox="0 0 473.41 473.41">
<path d="m443.82 88.765h-88.765v-73.971c0-8.177-6.617-14.794-14.794-14.794h-207.12c-8.177 0-14.794 6.617-14.794 14.794v73.971h-88.764v29.588h14.39l57.116 342.69c1.185 7.137 7.354 12.367 14.592 12.367h241.64c7.238 0 13.407-5.23 14.592-12.367l57.116-342.69h14.794c-1e-3 0-1e-3 -29.588-1e-3 -29.588zm-295.88-59.177h177.53v59.176h-177.53zm196.85 414.24h-216.58l-54.241-325.47h325.06z"/>
<path transform="matrix(.064 -.998 .998 .064 -.546 19.418)" d="m-360.4 301.29h207.54v29.592h-207.54z"/>
<path transform="matrix(.998 -.064 .064 .998 -.628 .399)" d="m141.64 202.35h29.592v207.54h-29.592z"/>
</svg>);
const svg_arrow = (<svg viewBox="0 0 482.24 482.24">
<path d="m206.81 447.79-206.81-206.67 206.74-206.67 24.353 24.284-165.17 165.17h416.31v34.445h-416.31l165.24 165.24z"/>
</svg>);
const svg_home = (<svg viewBox="0 0 476.91 476.91">
<path d="m461.78 209.41-212.21-204.89c-6.182-6.026-16.042-6.026-22.224 0l-212.2 204.88c-3.124 3.015-4.888 7.17-4.888 11.512 0 8.837 7.164 16 16 16h28.2v224c0 8.837 7.163 16 16 16h112c8.837 0 16-7.163 16-16v-128h80v128c0 8.837 7.163 16 16 16h112c8.837 0 16-7.163 16-16v-224h28.2c4.338 0 8.489-1.761 11.504-4.88 6.141-6.354 5.969-16.483-0.384-22.624zm-39.32 11.504c-8.837 0-16 7.163-16 16v224h-112v-128c0-8.837-7.163-16-16-16h-80c-8.837 0-16 7.163-16 16v128h-112v-224c0-8.837-7.163-16-16-16h-28.2l212.2-204.88 212.28 204.88h-28.28z"/>
</svg>);
const svg_refresh = (<svg viewBox="0 0 551.13 551.13">
<path d="m482.24 310.01c0 113.97-92.707 206.67-206.67 206.67s-206.67-92.708-206.67-206.67c0-102.21 74.639-187.09 172.23-203.56v65.78l86.114-86.114-86.114-86.115v71.641c-116.65 16.802-206.67 117.14-206.67 238.37 0 132.96 108.16 241.12 241.12 241.12s241.12-108.16 241.12-241.12z"/>
</svg>);
const svg_cancel = (<svg class="cancel" viewBox="0 0 612 612"><polygon points="612 36.004 576.52 0.603 306 270.61 35.478 0.603 0 36.004 270.52 306.01 0 576 35.478 611.4 306 341.41 576.52 611.4 612 576 341.46 306.01"/></svg>);
const svg_computer = (<svg class="computer" viewBox="0 0 480 480">
<g>
<path fill="#2C8CFF" d="m276 395v11.148c0 2.327-1.978 4.15-4.299 3.985-21.145-1.506-42.392-1.509-63.401-0.011-2.322 0.166-4.3-1.657-4.3-3.985v-11.137c0-2.209 1.791-4 4-4h64c2.209 0 4 1.791 4 4zm204-340v288c0 17.65-14.35 32-32 32h-416c-17.65 0-32-14.35-32-32v-288c0-17.65 14.35-32 32-32h416c17.65 0 32 14.35 32 32zm-125.62 386.36c-70.231-21.843-158.71-21.784-228.76 0-4.22 1.31-6.57 5.8-5.26 10.02 1.278 4.085 5.639 6.591 10.02 5.26 66.093-20.58 151.37-21.125 219.24 0 4.22 1.31 8.71-1.04 10.02-5.26s-1.04-8.71-5.26-10.02z"/>
</g>
</svg>);

// TODO
function getSize(type, size) {
  if (!size) {
    if (type <= 3) return "";
    return "0B";
  }
  size = size.toFloat();
  var toFixed = function(size) {
    size = (size * 100).toInteger();
    var a = (size / 100).toInteger();
    if (size % 100 == 0) return a;
    if (size % 10 == 0) return a + '.' + (size % 10);
    var b = size % 100;
    if (b < 10) b = '0' + b;
    return a + '.' + b;
  }
  if (size < 1024) return size.toInteger() + "B";
  if (size < 1024 * 1024) return toFixed(size / 1024) + "K";
  if (size < 1024 * 1024 * 1024) return toFixed(size / (1024 * 1024)) + "M";
  return toFixed(size / (1024 * 1024 * 1024)) + "G";
}

function getParentPath(is_remote, path) {
  let sep = handler.xcall("get_path_sep",is_remote);
  let res = path.lastIndexOf(sep);
  if (res <= 0) return "/";
  return path.substr(0, res);
}

function getFileName(is_remote, path) {
  let sep = handler.xcall("get_path_sep",is_remote);
  let res = path.lastIndexOf(sep);
  return path.substr(res + 1);
}

function getExt(name) {
  if (name.indexOf(".") == 0) {
    return "";
  }
  let i = name.lastIndexOf(".");
  if (i > 0) return name.substr(i + 1);
  return "";
}

var jobIdCounter = 1;

class JobTable extends Element {
  jobs = [];
  job_map = {};

  render() {
    let rows = this.jobs.map((job, i)=>this.renderRow(job, i));
    return (<section><table class="has_current job-table">    
      <tbody key={rows.length}>
      {rows}
      </tbody>
    </table></section>);
  }
    
  ["on click at svg.cancel"](_, me) {
    let job = this.jobs[me.parentElement.parentElement.index];
    let id = job.id;
    handler.xcall("cancel_job",id);
    delete this.job_map[id];
    let i = -1;
    this.jobs.map(function(job, idx) {
      if (job.id == id) i = idx;
    });
    this.jobs.splice(i, 1);
    this.componentUpdate();
    let is_remote = job.is_remote;
    if (job.type != "del-dir") is_remote = !is_remote;
    refreshDir(is_remote);
  }

  send(path, is_remote) {
    let to;
    let show_hidden;
    if (is_remote) {
      to = file_transfer.local_folder_view.fd.path; // NULL
      show_hidden = file_transfer.remote_folder_view.show_hidden;
    } else {
      to = file_transfer.remote_folder_view.fd.path;
      show_hidden = file_transfer.local_folder_view.show_hidden;
    }
    if (!to) return;
    to += handler.xcall("get_path_sep",!is_remote) + getFileName(is_remote, path);
    let id = jobIdCounter;
    jobIdCounter += 1;
    this.jobs.push({ type: "transfer",
                     id: id, path: path, to: to,
                     include_hidden: show_hidden,
                     is_remote: is_remote });
    this.job_map[id] = this.jobs[this.jobs.length - 1];
    handler.xcall("send_files",id, path, to, show_hidden, is_remote);
    this.componentUpdate();
  }

  addDelDir(path, is_remote) {
    let id = jobIdCounter;
    jobIdCounter += 1;
    this.jobs.push({ type: "del-dir", id: id, path: path, is_remote: is_remote });
    this.job_map[id] = this.jobs[this.jobs.length - 1];
    handler.xcall("remove_dir_all",id, path, is_remote);
    this.componentUpdate();
  }

  getSvg(job) {
    if (job.type == "transfer") {
      return svg_send;
    } else if (job.type == "del-dir") {
      return svg_trash;
    }
  }

  getStatus(job) {
    if (!job.entries) return translate("Waiting");
    let i = job.file_num + 1;
    let n = job.num_entries || job.entries.length;
    if (i > n) i = n;
    let res = i + ' / ' + n + " " + translate("files");
    if (job.total_size > 0) {
      let s = getSize(0, job.finished_size);
      if (s) s += " / ";
      res += ", " + s + getSize(0, job.total_size);
    }
    // below has problem if some file skipped
    let percent = job.total_size == 0 ? 100 : (100. * job.finished_size / job.total_size).toInteger(); // (100. * i / (n || 1)).toInteger();
    if (job.finished) percent = '100';
    if (percent) res += ", " + percent + "%";
    if (job.finished) res = translate("Finished") + " " + res;
    if (job.speed) res += ", " + getSize(0, job.speed) + "/s";
    return res;
  }

  updateJob(job) {
    let el = this.$("div#s" + job.id);  // TODO TEST
    console.log("updateJob el",el);
    if (el) el.text = this.getStatus(job);
  }

  updateJobStatus(id, file_num = -1, err = null, speed = null, finished_size = 0) {
    let job = this.job_map[id];
    if (!job) return;
    if (file_num < job.file_num) return;
    job.file_num = file_num;
    let n = job.num_entries || job.entries.length;
    job.finished = job.file_num >= n - 1 || err == "cancel";
    job.finished_size = finished_size;
    job.speed = speed || 0;
    this.updateJob(job);
    if (job.type == "del-dir") {
      if (job.finished) {
        if (!err) {
          handler.xcall("remove_dir",job.id, job.path, job.is_remote);
          refreshDir(job.is_remote);
        }
      } else if (!job.no_confirm) {
        handler.xcall("confirm_delete_files",id, job.file_num + 1);
      }
    } else if (job.finished || file_num == -1) {
      refreshDir(!job.is_remote);
    }
  }

  renderRow(job, i) {
    svg = this.getSvg(job);
    return (<tr class={job.is_remote ? "is_remote" : ""}><td>
      {svg}
      <div class="text">
        <div class="path">{job.path}</div>
        <div id={"s" + job.id}>{this.getStatus(job)}</div>
      </div>
      {svg_cancel}
    </td></tr>);
  }
}

class FolderView extends Element {
    fd = {};
    history = [];
    show_hidden = false;
    select_dir;

    sep() {
      return handler.xcall("get_path_sep",this.is_remote);
    }

    this(params) {
      this.is_remote = params.is_remote;
      if (this.is_remote) {
        this.show_hidden = !!handler.xcall("get_option","remote_show_hidden");
      } else {
        this.show_hidden = !!handler.xcall("get_option","local_show_hidden");
      }
      if (!this.is_remote) {
        let dir = handler.xcall("get_option","local_dir");
        if (dir) {
          this.fd = handler.xcall("read_dir",dir, this.show_hidden);
          if (this.fd) return;
        }
        this.fd = handler.xcall("read_dir",handler.xcall("get_home_dir"), this.show_hidden);
      }
    }

    // sort predicate
    foldersFirst(a, b) {
      if (a.type <= 3 && b.type > 3) return -1;
      if (a.type > 3 && b.type <= 3) return +1;
      if (a.name == b.name) return 0;
      return a.name.toLowerCase().lexicalCompare(b.name.toLowerCase()); // TODO lexicalCompare
    }

    render() 
    {
      return (<section>
            {this.renderTitle()}
            {this.renderNavBar()}
            {this.renderOpBar()}
            {this.renderTable()}
            </section>);
    }

    renderTitle() {
      return (<div class="title">
        {svg_computer}
        <div class="platform">{platformSvg(handler.xcall("get_platform",this.is_remote), "white")}</div>
        <div><span>{translate(this.is_remote ? "Remote Computer" : "Local Computer")}</span></div>
      </div>)
    }

    renderNavBar() {
      return <div class="toolbar navbar">
        <div class="home button">{svg_home}</div>
        <div class="goback button">{svg_arrow}</div>
        <div class="goup button">{svg_arrow}</div>
        {this.renderSelect()}
        <div class="refresh button">{svg_refresh}</div>
      </div>;
    }

    // TODO
    componentDidMount(){
      this.select_dir = this.$("select.select-dir")
    }

    renderSelect() {
      return (<select editable class="select-dir">    
        <option>/</option>    
      </select>);
    }

    renderOpBar() {
      if (this.is_remote) {
        return (<div class="toolbar remote">
          <div class="send button">{svg_send}<span>{translate('Receive')}</span></div>
          <div class="spacer"></div>
          <div class="add-folder button">{svg_add_folder}</div>
          <div class="trash button">{svg_trash}</div>
        </div>);
      }
      return (<div class="toolbar">
        <div class="add-folder button">{svg_add_folder}</div>
        <div class="trash button">{svg_trash}</div>
        <div class="spacer"></div>
        <div class="send button"><span>{translate('Send')}</span>{svg_send}</div>
      </div>);
    }

    get_updated() {
      this.table.sortRows(false); // TODO sortRows
      if (this.fd && this.fd.path) this.select_dir.value = this.fd.path;
    }

    renderTable() {
      let fd = this.fd;
      let entries = fd.entries || [];
      let table = this.table;
      if (!table || !table.sortBy) {
        entries.sort(this.foldersFirst); // TODO sort function
      }
      let path = fd.path;
      if (path != "/" && path) {
        entries = [{ name: "..", type: 1 }].concat(entries);        
      }
      let rows = entries.map(e=>this.renderRow(e));
      let id = (this.is_remote ? "remote" : "local") + "-folder-view";
      //@{}  return (<table @{this.table} .folder-view .has_current id={id}>    

      return (<table class="folder-view has_current" id={id}>    
        <thead>    
          <tr><th></th><th class="sortable">{translate('Name')}</th><th class="sortable">{translate('Modified')}</th><th class="sortable">{translate('Size')}</th></tr>    
        </thead>      
        <tbody> 
          {rows}
        </tbody>
        <popup>
          <menu class="context" id={id}>
            <li id="switch-hidden" class={this.show_hidden ? "selected" : ""}><span>{svg_checkmark}</span>{translate('Show Hidden Files')}</li>
          </menu>
        </popup>
      </table>);
    }

    joinPath(name) {
      let path = this.fd.path;
      if (path == "/") {
        if (this.sep() == "/") return this.sep() + name;
        else return name;
      }
      return path + (path[path.length - 1] == this.sep() ? "" : this.sep()) + name;
    }
    
    attached() {
      this.table.onRowDoubleClick = (row)=>{
        let type = row[0].attributes["type"];
        if (type > 3) return;
        let name = row[1].text;
        let path = name == ".." ? getParentPath(this.is_remote, this.fd.path) : this.joinPath(name);
        this.goto(path, true);
      }
      this.get_updated();
    }

    goto(path, push) {
      if (!path) return;
      if (this.sep() == "\\" && path.length == 2) { // windows drive
        path += "\\";
      }
      if (push) this.pushHistory();
      if (this.is_remote) {
        handler.xcall("read_remote_dir",path, this.show_hidden);
      } else {
        var fd = handler.xcall("read_dir",path, this.show_hidden);
        this.refresh({ fd: fd });
      }
    }

    refresh(data) {
      if (!data.fd || !data.fd.path) return;
      if (this.is_remote && !remote_home_dir) {
        remote_home_dir = data.fd.path;
      }
      this.componentUpdate(data);
      setTimeout(()=>this.get_updated(),1);
    }

    renderRow(entry) {
      let path;
      if (this.is_remote) {
        path = handler.xcall("get_icon_path",entry.type, getExt(entry.name));
      } else {
        path = this.joinPath(entry.name);
      }
      let tm = entry.time ? new Date(entry.time.toFloat() * 1000.).toLocaleString() : 0; // TODO toFloat()
      return (<tr role="option">
        <td type={entry.type} filename={path}></td>
        <td>{entry.name}</td>
        <td value={entry.time || 0}>{tm || ""}</td>
        <td value={entry.size || 0}>{getSize(entry.type, entry.size)}</td>
      </tr>);
    }

    ["on click at #switch-hidden"]() {
      this.show_hidden = !this.show_hidden;
      this.refreshDir();
    }

    ["on click at .goup"]() {
      let path = this.fd.path;
      if (!path || path == "/") return;
      path = getParentPath(this.is_remote, path);
      this.goto(path, true);
    }

    ["on click at .goback"] () {
      let path = this.history.pop();
      if (!path) return;
      this.goto(path, false);
    }

    ["on click at .trash"]() {
      let rows = this.getCurrentRows();
      if (!rows || rows.length == 0) return;

      let delete_dirs = new Array();

      for (let i = 0; i < rows.length; ++i) {
        let row = rows[i];

        let path = row[0];
        let type = row[1];

        let new_history = [];
        for (let j = 0; j < this.history.length; ++j) {
          let h = this.history[j];
          if ((h + this.sep()).indexOf(path + this.sep()) == -1) new_history.push(h);
        }
        this.history = new_history;
        if (type == 1) {
          delete_dirs.push(path);
        } else {
          confirmDelete(path, this.is_remote);
        }
      }
      for (let i = 0; i < delete_dirs.length; ++i) {
        file_transfer.job_table.addDelDir(delete_dirs[i], this.is_remote);
      }
    }

    ["on click at .add-folder"]() {
      let me = this;
      msgbox("custom", translate("Create Folder"), "<div .form> \
            <div>" + translate("Please enter the folder name") + ":</div> \
            <div><input|text(name) .outline-focus /></div> \
        </div>", function(res=null) {
          if (!res) return;
          if (!res.name) return;
          let name = res.name.trim();
          if (!name) return;
          if (name.indexOf(me.sep()) >= 0) {
            msgbox("custom-error", "Create Folder", "Invalid folder name");
            return;
          }
          let path = me.joinPath(name);
           handler.xcall("create_dir",jobIdCounter, path, me.is_remote);
           create_dir_jobs[jobIdCounter] = { is_remote: me.is_remote, path: path };
          jobIdCounter += 1;
        });
    }

    refreshDir() {
      this.goto(this.fd.path, false);
    }

    ["on click at .refresh"]() {
      this.refreshDir();
    }

    ["on click at .home"]() {
      let path = this.is_remote ? remote_home_dir : handler.xcall("get_home_dir");
      if (!path) return;
      if (path == this.fd.path) return;
      this.goto(path, true);
    }

    getCurrentRow() {
      let row = this.table.getCurrentRow();  // TEST getCurrentRow
      if (!row) return;
      let name = row[1].text;
      if (!name || name == "..") return;
      let type = row[0].attributes["type"];
      return [this.joinPath(name), type];
    }

    getCurrentRows() {
      let rows = this.table.getCurrentRows();
      if (!rows || rows.length== 0) return;

      let records = new Array();

      for (let i = 0; i < rows.length; ++i) {
        let name = rows[i][1].text;
        if (!name || name == "..") continue;

        let type = rows[i][0].attributes["type"];
        records.push([this.joinPath(name), type]);
      }
      return records;
    }

    ["on click at .send"]() {
      let rows = this.getCurrentRows();
      if (!rows || rows.length == 0) return;
      for (let i = 0; i < rows.length; ++i) {
        file_transfer.job_table.send(rows[i][0], this.is_remote);
      }
    }

    ["on change at .select-dir"](_, el) {
      var x = getTime() - last_key_time;  // TODO getTime
      if (x < 1000) return;
      if (this.fd.path != el.value) {
        this.goto(el.value, true);
      }
    }

    ["on keydown at .select-dir"](evt, me) {
      if (evt.code == "KeyRETURN") { // TODO TEST mac
        this.goto(me.value, true);
      }
    }

    pushHistory() {
      let path = this.fd.path;
      if (!path) return;
      if (path != this.history[this.history.length - 1]) this.history.push(path);
    }
}

var file_transfer;

class FileTransfer extends Element {
    this() {
      file_transfer = this;
    }
    // TODO @{}
    // <FolderView is_remote={false} @{this.local_folder_view} />
    // <FolderView is_remote={true} @{this.remote_folder_view}/>
    // <JobTable @{this.job_table} />

    render() {
      return (<div id="file-transfer">
            <FolderView is_remote={false} />
            <FolderView is_remote={true} />
            <JobTable />
          </div>);
    }
}

export function initializeFileTransfer() 
{
  $("#file-transfer-wrapper").content(<FileTransfer />);
  $("#video-wrapper").style.setProperty("visibility","hidden");
  $("#video-wrapper").style.setProperty("position","absolute");
  $("#file-transfer-wrapper").style.setProperty("display","block");
}

handler.updateFolderFiles = function(fd) {
  fd.entries = fd.entries || [];
  if (fd.id > 0) {
    let jt = file_transfer.job_table;
    let job = jt.job_map[fd.id];
    if (job) {
      job.file_num = -1;
      job.total_size = fd.total_size;
      job.entries = fd.entries;
      job.num_entries = fd.num_entries;
      file_transfer.job_table.updateJobStatus(job.id);
    }
  } else {
    file_transfer.remote_folder_view.refresh({ fd: fd });
  }
}

handler.jobProgress = function(id, file_num, speed, finished_size) {
  file_transfer.job_table.updateJobStatus(id, file_num, null, speed, finished_size);
}

handler.jobDone = function(id, file_num = -1) {
  let job = deleting_single_file_jobs[id] || create_dir_jobs[id];
  if (job) {
    refreshDir(job.is_remote);
    return;
  }
  file_transfer.job_table.updateJobStatus(id, file_num);
}

handler.jobError = function(id, err, file_num = -1) {
  var job = deleting_single_file_jobs[id];
  if (job) {
    msgbox("custom-error", "Delete File", err);
    return;
  }
  job = create_dir_jobs[id];
  if (job) {
    msgbox("custom-error", "Create Folder", err);
    return;
  }
  if (file_num < 0) {
    msgbox("custom-error", "Failed", err);
  }
  file_transfer.job_table.updateJobStatus(id, file_num, err);
}

function refreshDir(is_remote) {
  if (is_remote) file_transfer.remote_folder_view.refreshDir();
  else file_transfer.local_folder_view.refreshDir();
}

var deleting_single_file_jobs = {};
var create_dir_jobs = {}

function confirmDelete(path, is_remote) {
  msgbox("custom-skip", "Confirm Delete", "<div .form> \
        <div>" + translate('Are you sure you want to delete this file?') + "</div> \
        <div.ellipsis style=\"font-weight: bold;\">" + path + "</div> \
    </div>", function(res=null) {
      if (res) {
        handler.xcall("remove_file",jobIdCounter, path, 0, is_remote);
        deleting_single_file_jobs[jobIdCounter] = { is_remote: is_remote, path: path };
        jobIdCounter += 1;
      }
    });
}

handler.confirmDeleteFiles = function(id, i, name) {
  var jt = file_transfer.job_table;
  var job = jt.job_map[id];
  if (!job) return;
  var n = job.num_entries;
  if (i >= n) return;
  var file_path = job.path;
  if (name) file_path += handler.xcall("get_path_sep",job.is_remote) + name;
  msgbox("custom-skip", "Confirm Delete", "<div .form> \
        <div>" + translate('Deleting') + " #" + (i + 1) + " / " + n + " " + translate('files') + ".</div> \
        <div>" + translate('Are you sure you want to delete this file?') + "</div> \
        <div.ellipsis style=\"font-weight: bold;\" .text>" + name + "</div> \
        <div><button|checkbox(remember) {ts}>" + translate('Do this for all conflicts') + "</button></div> \
    </div>", function(res=null) {
      if (!res) {
        jt.updateJobStatus(id, i - 1, "cancel");
      } else if (res.skip) {
        if (res.remember) jt.updateJobStatus(id, i, "cancel");
        else handler.jobDone(id, i);
      } else {
        job.no_confirm = res.remember;
        if (job.no_confirm) handler.set_no_confirm(id);
        handler.xcall("remove_file",id, file_path, i, job.is_remote);
      }
    });
}

export function save_file_transfer_close_state() {
  var local_dir = file_transfer.local_folder_view.fd.path || "";
  var local_show_hidden = file_transfer.local_folder_view.show_hidden ? "Y" : "";
  var remote_dir = file_transfer.remote_folder_view.fd.path || "";
  var remote_show_hidden = file_transfer.remote_folder_view.show_hidden ? "Y" : "";
  handler.xcall("save_close_state","local_dir", local_dir);
  handler.xcall("save_close_state","local_show_hidden", local_show_hidden);
  handler.xcall("save_close_state","remote_dir", remote_dir);
  handler.xcall("save_close_state","remote_show_hidden", remote_show_hidden);
}
