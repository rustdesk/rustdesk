import { promptAction, router, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { fileIo as fs, picker } from '@kit.CoreFileKit';
import { BusinessError, zlib } from '@kit.BasicServicesKit';
import { ffiReadDir, ffiSendFiles, ffiSetConfirmOverrideFile, ffiCancelJob,
         ffiSessionClose, ffiSessionLogin, ffiSessionSend2fa } from '../ohos_ffi';
import { globalEvents, RdeskEvent } from '../model/RdeskEvents';
import { evStr, evNum, parseJsonObject, arrayOfRecords, routerParams, joinPath,
         parentPath, fileNameOf, formatBytes } from '../model/Utils';

const LOG_DOMAIN = 0x5253;
const TAG = 'RustDeskFiles';

class FileEntry {
  name: string = '';
  path: string = '';
  isDir: boolean = false;
  size: number = 0;
}

class JobItem {
  id: number = 0;
  name: string = '';
  // Bytes of the source entry recorded at submit time; 0 = folder / unknown,
  // in which case no percentage can be shown (folder jobs show file_num).
  total: number = 0;
  isUpload: boolean = false;
  fileNum: number = 0;
  speed: number = 0;
  finished: number = 0;
  err: string = '';
  done: boolean = false;
}

function sortEntries(out: FileEntry[]): FileEntry[] {
  out.sort((a: FileEntry, b: FileEntry): number => {
    if (a.isDir !== b.isDir) {
      return a.isDir ? -1 : 1;
    }
    if (a.name < b.name) {
      return -1;
    }
    if (a.name > b.name) {
      return 1;
    }
    return 0;
  });
  return out;
}

@Entry
@Component
struct FileTransferPage {
  @State uuid: string = '';
  @State peerId: string = '';
  @State localDir: string = '';
  @State localEntries: FileEntry[] = [];
  @State localSel: FileEntry = new FileEntry();
  @State localHasSel: boolean = false;
  @State remoteDir: string = '';
  @State remoteEntries: FileEntry[] = [];
  @State remoteSel: FileEntry = new FileEntry();
  @State remoteHasSel: boolean = false;
  @State jobs: JobItem[] = [];
  @State showOverrideDialog: boolean = false;
  @State overrideText: string = '';
  // msgbox dialogs (same flow as RemotePage)
  @State showPasswordDialog: boolean = false;
  @State showTwofaDialog: boolean = false;
  @State msgboxText: string = '';
  @State passwordInput: string = '';
  @State rememberPassword: boolean = false;
  @State twofaInput: string = '';
  @State trustDevice: boolean = false;
  private localRoot: string = '';
  private actId: number = 1;
  private overrideActId: number = 0;
  private overrideFileNum: number = 0;
  private overrideIsUpload: boolean = false;
  private eventHandler: ((ev: RdeskEvent) => void) | null = null;

  aboutToAppear(): void {
    const p = routerParams();
    this.uuid = evStr(p['uuid']);
    this.peerId = evStr(p['peerId']);
    const ctx = getContext(this);
    this.localDir = ctx.filesDir;
    this.localRoot = ctx.filesDir;
    this.eventHandler = (ev: RdeskEvent): void => {
      this.handleEvent(ev);
    };
    globalEvents.subscribe(this.eventHandler);
    this.refreshLocal();
    // The peer drops file requests until the login is accepted; the remote
    // listing is fetched on the peer_info event instead of here.
    this.setKeepScreenOn(true);
  }

  aboutToDisappear(): void {
    if (this.eventHandler !== null) {
      globalEvents.unsubscribe(this.eventHandler);
      this.eventHandler = null;
    }
    this.setKeepScreenOn(false);
    ffiSessionClose(this.uuid);
  }

  // Long transfers must survive the screen-off timeout, which would otherwise
  // suspend the app and stall the jobs.
  private setKeepScreenOn(on: boolean): void {
    window.getLastWindow(getContext(this)).then((win: window.Window) => {
      return win.setWindowKeepScreenOn(on);
    }).catch((e: BusinessError) => {
      hilog.warn(LOG_DOMAIN, TAG, 'setWindowKeepScreenOn(%{public}s) failed: %{public}s',
                 `${on}`, `${e}`);
    });
  }

  // ------------------------------------------------------------------
  // Events
  // ------------------------------------------------------------------

  private handleEvent(ev: RdeskEvent): void {
    switch (ev.name) {
      case 'file_dir': {
        // Full listing response to ohos_read_dir (update_folder_files carries
        // only job counts, not listings). The ohos side only queries the
        // remote panel, so is_local is implicitly false.
        const fd = parseJsonObject(ev.get('value'));
        const p = evStr(fd['path']);
        if (p.length > 0) {
          // The peer echoes the canonical path; adopt it first so entry paths
          // and later navigation/upload targets match what it listed ('' ->
          // its home).
          this.remoteDir = p;
        }
        const list: FileEntry[] = [];
        for (const it of arrayOfRecords(fd['entries'])) {
          const e = new FileEntry();
          e.name = evStr(it['name']);
          // FileType proto: Dir=0, DirLink=2, DirDrive=3; File/FileLink=4/5.
          const t = evNum(it['entry_type']);
          e.isDir = t === 0 || t === 2 || t === 3;
          e.size = evNum(it['size']);
          e.path = joinPath(this.remoteDir, e.name);
          list.push(e);
        }
        this.remoteEntries = sortEntries(list);
        break;
      }
      case 'job_progress':
        this.upsertJob(ev.num('id'), ev.num('file_num'), ev.num('speed'), ev.num('finished_size'),
                       '', false);
        break;
      case 'job_done':
        this.upsertJob(ev.num('id'), ev.num('file_num'), 0, 0, '', true);
        break;
      case 'job_error':
        this.upsertJob(ev.num('id'), ev.num('file_num'), 0, 0, ev.get('err'), true);
        break;
      case 'override_file_confirm':
        this.overrideActId = ev.num('id');
        this.overrideFileNum = ev.num('file_num');
        this.overrideIsUpload = ev.bool('is_upload');
        this.overrideText = ev.get('read_path');
        this.showOverrideDialog = true;
        break;
      case 'msgbox': {
        const type = ev.get('type');
        this.msgboxText = ev.get('text');
        if (type === 'input-password' || type === 're-input-password') {
          this.showPasswordDialog = true;
        } else if (type === 'input-2fa') {
          this.showTwofaDialog = true;
        } else {
          promptAction.showToast({ message: this.msgboxText, duration: 3000 });
        }
        break;
      }
      case 'cancel_msgbox':
        this.showPasswordDialog = false;
        this.showTwofaDialog = false;
        break;
      case 'peer_info':
        // PeerInfo only arrives after the peer accepted the login request, so
        // any pending credential prompt is stale (mirrors RemotePage). The
        // initial ReadDir sent before authorization was dropped by the peer,
        // so fetch the remote listing now.
        this.showPasswordDialog = false;
        this.showTwofaDialog = false;
        this.refreshRemote();
        break;
      default:
        break;
    }
  }

  // Terminal rows are frozen: the cancel path marks jobs done locally (no
  // callback arrives from the peer), so late job_error/job_done events for a
  // cancelled id must not resurrect the row.
  private upsertJob(id: number, fileNum: number, speed: number, finished: number,
                    err: string, done: boolean): void {
    const next: JobItem[] = [];
    let found = false;
    for (const j of this.jobs) {
      if (j.id === id) {
        found = true;
        if (!j.done) {
          j.fileNum = fileNum;
          if (speed > 0) {
            j.speed = speed;
          }
          if (finished > 0) {
            j.finished = finished;
          }
          if (err.length > 0) {
            j.err = err;
          }
          j.done = done;
        }
      }
      next.push(j);
    }
    if (!found) {
      const j = new JobItem();
      j.id = id;
      j.fileNum = fileNum;
      j.speed = speed;
      j.finished = finished;
      j.err = err;
      j.done = done;
      next.push(j);
    }
    this.jobs = next;
  }

  // The peer sends no completion callback for cancelled jobs, so the row is
  // closed here immediately (same approach as the Flutter JobController).
  private cancelJob(job: JobItem): void {
    ffiCancelJob(this.uuid, job.id);
    const next: JobItem[] = [];
    for (const j of this.jobs) {
      if (j.id === job.id && !j.done) {
        j.done = true;
        j.err = 'cancel';
        j.speed = 0;
      }
      next.push(j);
    }
    this.jobs = next;
    promptAction.showToast({ message: $r('app.string.job_cancelled_toast'), duration: 1500 });
  }

  private clearFinishedJobs(): void {
    const next: JobItem[] = [];
    for (const j of this.jobs) {
      if (!j.done) {
        next.push(j);
      }
    }
    this.jobs = next;
  }

  private jobStateText(j: JobItem): ResourceStr {
    if (j.err === 'cancel') {
      return $r('app.string.transfer_state_cancelled');
    }
    if (j.err === 'skipped') {
      return $r('app.string.transfer_state_skipped');
    }
    if (j.err.length > 0) {
      return $r('app.string.transfer_state_failed');
    }
    return $r('app.string.transfer_state_finished');
  }

  private jobStateColor(j: JobItem): ResourceColor {
    if (j.err === 'cancel') {
      return '#999999';
    }
    if (j.err === 'skipped') {
      return '#FF9800';
    }
    return '#F44336';
  }

  private jobPercent(j: JobItem): number {
    if (j.total <= 0) {
      return 0;
    }
    const pct = Math.floor(j.finished / j.total * 100);
    return Math.max(0, Math.min(100, pct));
  }

  @Builder
  JobRow(j: JobItem) {
    Column({ space: 4 }) {
      Row({ space: 6 }) {
        Text(j.isUpload ? '↑' : '↓')
          .fontSize(14)
          .fontWeight(FontWeight.Bold)
          .fontColor(j.isUpload ? '#3A7BD5' : '#4CAF50')
          .width(18)
          .textAlign(TextAlign.Center)
        Text(j.name)
          .fontSize(13)
          .fontColor('#333333')
          .maxLines(1)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .layoutWeight(1)
        if (!j.done) {
          Text($r('app.string.cancel_job'))
            .fontSize(12)
            .fontColor('#F44336')
            .padding({ left: 8, right: 2, top: 4, bottom: 4 })
            .onClick(() => {
              this.cancelJob(j);
            })
        }
      }
      .width('100%')
      if (j.total > 0) {
        Row({ space: 6 }) {
          Progress({ value: this.jobPercent(j), total: 100, type: ProgressType.Linear })
            .color(j.isUpload ? '#3A7BD5' : '#4CAF50')
            .style({ strokeWidth: 4 })
            .layoutWeight(1)
          Text(`${this.jobPercent(j)}%`)
            .fontSize(11)
            .fontColor('#666666')
            .width(36)
            .textAlign(TextAlign.End)
        }
        .width('100%')
      }
      Row({ space: 6 }) {
        if (j.done) {
          if (j.err.length > 0) {
            Text(this.jobStateText(j)).fontSize(11).fontColor(this.jobStateColor(j))
          } else {
            Text($r('app.string.transfer_state_finished')).fontSize(11).fontColor('#4CAF50')
          }
        } else {
          if (j.speed > 0) {
            Text($r('app.string.transfer_state_transferring')).fontSize(11).fontColor('#3A7BD5')
          } else {
            Text($r('app.string.transfer_state_waiting')).fontSize(11).fontColor('#999999')
          }
        }
        if (j.total === 0) {
          Text($r('app.string.file_num')).fontSize(11).fontColor('#666666')
          Text(`${j.fileNum}`).fontSize(11).fontColor('#666666')
        }
        Blank()
        if (!j.done && j.speed > 0) {
          Text(`${formatBytes(j.speed)}/s`).fontSize(11).fontColor('#666666')
        }
        Text(j.total > 0 ? `${formatBytes(j.finished)} / ${formatBytes(j.total)}`
                         : formatBytes(j.finished))
          .fontSize(11)
          .fontColor('#999999')
      }
      .width('100%')
    }
    .width('100%')
    .padding(8)
    .borderRadius(8)
    .backgroundColor('#FFFFFF')
  }

  // ------------------------------------------------------------------
  // Local panel (app sandbox via @ohos.file.fs)
  // ------------------------------------------------------------------

  private refreshLocal(): void {
    const out: FileEntry[] = [];
    try {
      const names = fs.listFileSync(this.localDir);
      for (const n of names) {
        const p = joinPath(this.localDir, n);
        try {
          const st = fs.statSync(p);
          const e = new FileEntry();
          e.name = n;
          e.path = p;
          e.isDir = st.isDirectory();
          e.size = st.size;
          out.push(e);
        } catch (err) {
          hilog.warn(LOG_DOMAIN, TAG, 'stat %{public}s failed', p);
        }
      }
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'list %{public}s failed: %{public}s', this.localDir, `${e}`);
    }
    this.localEntries = sortEntries(out);
  }

  private enterLocal(e: FileEntry): void {
    if (!e.isDir) {
      return;
    }
    this.localDir = e.path;
    this.refreshLocal();
  }

  private localUp(): void {
    if (this.localDir === this.localRoot) {
      promptAction.showToast({ message: $r('app.string.local_root_reached'), duration: 1500 });
      return;
    }
    this.localDir = parentPath(this.localDir);
    this.refreshLocal();
  }

  // ------------------------------------------------------------------
  // System picker import/export (outside the sandbox via @ohos.file.picker)
  //
  // Device limits of the picker itself drive the shapes below: folder and
  // mixed selection (DocumentSelectMode.FOLDER/MIXED) only exist on tablet /
  // foldable / PC - on phones the picker returns instantly with no result.
  // Import is therefore file-only, and a folder is exported by zipping it
  // (zlib.compressFile) and saving the archive through the save picker.
  // ------------------------------------------------------------------

  private importFromSystem(): void {
    const docPicker = new picker.DocumentViewPicker();
    const opt = new picker.DocumentSelectOptions();
    opt.maxSelectNumber = 50;
    docPicker.select(opt).then((uris: Array<string>) => {
      let ok = 0;
      for (const uri of uris) {
        try {
          fs.copyFileSync(uri, joinPath(this.localDir, fileNameOf(uri)));
          ok++;
        } catch (e) {
          hilog.warn(LOG_DOMAIN, TAG, 'import %{public}s failed: %{public}s', uri, `${e}`);
        }
      }
      this.refreshLocal();
      if (ok > 0) {
        promptAction.showToast({ message: $r('app.string.import_done'), duration: 2000 });
      }
    }).catch((e: BusinessError) => {
      // 13900042 = user dismissed the picker; anything else is worth a line.
      hilog.warn(LOG_DOMAIN, TAG, 'document select failed: %{public}s', `${e}`);
    });
  }

  private exportToSystem(): void {
    if (!this.localHasSel) {
      promptAction.showToast({ message: $r('app.string.select_local_file'), duration: 2000 });
      return;
    }
    const sel = this.localSel;
    if (!sel.isDir) {
      this.saveToFileSystem(sel.path, sel.name);
      return;
    }
    const ctx = getContext(this);
    const tmpZip = joinPath(ctx.cacheDir, `${sel.name}.zip`);
    const opts: zlib.Options = { level: zlib.CompressLevel.COMPRESS_LEVEL_DEFAULT_COMPRESSION };
    zlib.compressFile(sel.path, tmpZip, opts).then(() => {
      return this.saveToFileSystem(tmpZip, `${sel.name}.zip`);
    }).catch((e: BusinessError) => {
      // An empty folder cannot be compressed; anything else is IO.
      hilog.warn(LOG_DOMAIN, TAG, 'compress %{public}s failed: %{public}s', sel.path, `${e}`);
      promptAction.showToast({ message: $r('app.string.import_export_failed'), duration: 2000 });
    });
  }

  // Save one sandbox file at a user-chosen location; resolves after the copy.
  // Deletes the source afterwards when asked (the temporary zip).
  private async saveToFileSystem(path: string, name: string): Promise<void> {
    const savePicker = new picker.DocumentViewPicker();
    const opt = new picker.DocumentSaveOptions();
    opt.newFileNames = [name];
    try {
      const uris: Array<string> = await savePicker.save(opt);
      if (uris.length === 0) {
        return;
      }
      fs.copyFileSync(path, uris[0]);
      promptAction.showToast({ message: $r('app.string.export_done'), duration: 2000 });
    } catch (e) {
      hilog.warn(LOG_DOMAIN, TAG, 'save %{public}s failed: %{public}s', path, `${e}`);
      promptAction.showToast({ message: $r('app.string.import_export_failed'), duration: 2000 });
    } finally {
      if (path.endsWith('.zip') && path.indexOf(getContext(this).cacheDir) === 0) {
        try {
          fs.unlinkSync(path);
        } catch (e) {
          hilog.warn(LOG_DOMAIN, TAG, 'unlink %{public}s failed: %{public}s', path, `${e}`);
        }
      }
    }
  }

  // ------------------------------------------------------------------
  // Remote panel (ohos_read_dir + update_folder_files)
  // ------------------------------------------------------------------

  private refreshRemote(): void {
    this.remoteEntries = [];
    ffiReadDir(this.uuid, this.remoteDir, false, true);
  }

  private enterRemote(e: FileEntry): void {
    if (!e.isDir) {
      return;
    }
    this.remoteDir = e.path;
    // The old selection belongs to the previous directory.
    this.remoteHasSel = false;
    this.refreshRemote();
  }

  private remoteUp(): void {
    if (this.remoteDir === '/' || this.remoteDir.length === 0) {
      promptAction.showToast({ message: $r('app.string.remote_root_reached'), duration: 1500 });
      return;
    }
    this.remoteDir = parentPath(this.remoteDir);
    if (this.remoteDir.length === 0) {
      this.remoteDir = '/';
    }
    this.remoteHasSel = false;
    this.refreshRemote();
  }

  // ------------------------------------------------------------------
  // Transfer actions
  // ------------------------------------------------------------------

  private upload(): void {
    if (!this.localHasSel) {
      promptAction.showToast({ message: $r('app.string.select_local_file'), duration: 2000 });
      return;
    }
    const to = joinPath(this.remoteDir, this.localSel.name);
    const id = this.actId++;
    const job = new JobItem();
    job.id = id;
    job.name = this.localSel.name;
    job.total = this.localSel.isDir ? 0 : this.localSel.size;
    job.isUpload = true;
    this.jobs = this.jobs.concat([job]);
    hilog.info(LOG_DOMAIN, TAG, 'upload job %{public}d: %{public}s -> %{public}s', id,
               this.localSel.path, to);
    ffiSendFiles(this.uuid, id, this.localSel.path, to, 0, false, false);
  }

  private download(): void {
    if (!this.remoteHasSel) {
      promptAction.showToast({ message: $r('app.string.select_remote_file'), duration: 2000 });
      return;
    }
    // The selection carries its own full path: remoteDir may have changed
    // since it was picked.
    const from = this.remoteSel.path;
    const to = joinPath(this.localDir, this.remoteSel.name);
    const id = this.actId++;
    const job = new JobItem();
    job.id = id;
    job.name = this.remoteSel.name;
    job.total = this.remoteSel.isDir ? 0 : this.remoteSel.size;
    this.jobs = this.jobs.concat([job]);
    hilog.info(LOG_DOMAIN, TAG, 'download job %{public}d: %{public}s -> %{public}s', id, from, to);
    ffiSendFiles(this.uuid, id, from, to, 0, false, true);
  }

  private answerOverride(needOverride: boolean): void {
    this.showOverrideDialog = false;
    ffiSetConfirmOverrideFile(this.uuid, this.overrideActId, this.overrideFileNum, needOverride,
                              false, this.overrideIsUpload);
  }

  private answerPassword(): void {
    this.showPasswordDialog = false;
    ffiSessionLogin(this.uuid, this.passwordInput, this.rememberPassword);
    this.passwordInput = '';
  }

  private answer2fa(): void {
    this.showTwofaDialog = false;
    ffiSessionSend2fa(this.uuid, this.twofaInput, this.trustDevice);
    this.twofaInput = '';
  }

  @Builder
  PanelHeader(label: ResourceStr, dir: string, onUp: () => void, onRefresh: () => void) {
    Column({ space: 4 }) {
      Text(label).fontSize(14).fontWeight(FontWeight.Medium)
      Text(dir.length > 0 ? dir : '/')
        .fontSize(11)
        .fontColor('#666666')
        .maxLines(2)
      Row({ space: 6 }) {
        Button($r('app.string.parent_dir'), { buttonStyle: ButtonStyleMode.NORMAL })
          .height(28)
          .fontSize(12)
          .onClick(() => {
            onUp();
          })
        Button($r('app.string.refresh'), { buttonStyle: ButtonStyleMode.NORMAL })
          .height(28)
          .fontSize(12)
          .onClick(() => {
            onRefresh();
          })
        Blank()
      }
      .width('100%')
    }
    .width('100%')
    .alignItems(HorizontalAlign.Start)
    .padding(8)
  }

  @Builder
  FileRow(e: FileEntry, selected: boolean, onSelect: () => void, onOpen: () => void) {
    Row({ space: 6 }) {
      SymbolGlyph(e.isDir ? $r('sys.symbol.folder') : $r('sys.symbol.doc'))
        .fontSize(14)
        .fontColor(['#3A7BD5'])
      Text(e.name)
        .fontSize(13)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .layoutWeight(1)
      if (!e.isDir) {
        Text(formatBytes(e.size)).fontSize(11).fontColor('#999999')
      }
      if (e.isDir) {
        Text($r('app.string.open')).fontSize(12).fontColor('#3A7BD5')
          .onClick(() => {
            onOpen();
          })
      }
    }
    .width('100%')
    .padding({ left: 8, right: 8, top: 8, bottom: 8 })
    .borderRadius(6)
    .backgroundColor(selected ? '#D6E4FF' : Color.Transparent)
    .onClick(() => {
      onSelect();
    })
  }

  build() {
    Stack() {
      Column() {
        Row({ space: 8 }) {
          Text($r('app.string.file_transfer')).fontSize(16).fontWeight(FontWeight.Medium)
          if (this.peerId.length > 0) {
            Text(`- ${this.peerId}`).fontSize(14).fontColor('#666666')
          }
        }
        .width('100%')
        .padding(10)

        Row({ space: 6 }) {
          // ---- local panel ----
          Column() {
            this.PanelHeader($r('app.string.local'), this.localDir, (): void => {
              this.localUp();
            }, (): void => {
              this.refreshLocal();
            })
            Row({ space: 6 }) {
              Button($r('app.string.import_from_system'), { buttonStyle: ButtonStyleMode.NORMAL })
                .height(28)
                .fontSize(12)
                .onClick(() => {
                  this.importFromSystem();
                })
              Button($r('app.string.export_to_system'), { buttonStyle: ButtonStyleMode.NORMAL })
                .height(28)
                .fontSize(12)
                .onClick(() => {
                  this.exportToSystem();
                })
            }
            .width('100%')
            List({ space: 2 }) {
              ForEach(this.localEntries, (e: FileEntry) => {
                ListItem() {
                  this.FileRow(e, this.localHasSel && this.localSel.path === e.path, (): void => {
                    this.localSel = e;
                    this.localHasSel = true;
                  }, (): void => {
                    this.enterLocal(e);
                  })
                }
              }, (e: FileEntry) => `local_${e.path}`)
            }
            .width('100%')
            .layoutWeight(1)
            if (this.localEntries.length === 0) {
              Text($r('app.string.empty_dir')).fontSize(12).fontColor('#999999')
                .margin({ top: 20 })
            }
          }
          .layoutWeight(1)
          .height('100%')
          .backgroundColor('#FAFAFA')
          .borderRadius(8)
          .padding(4)

          // ---- transfer actions (Sunlogin-style middle column) ----
          Column({ space: 16 }) {
            Column({ space: 2 }) {
              Button('↑', { buttonStyle: ButtonStyleMode.NORMAL })
                .type(ButtonType.Circle)
                .width(44)
                .height(44)
                .fontSize(20)
                .fontColor(Color.White)
                .backgroundColor('#3A7BD5')
                .onClick(() => {
                  this.upload();
                })
              Text($r('app.string.upload_to_remote')).fontSize(10).fontColor('#666666')
            }
            .opacity(this.localHasSel ? 1 : 0.4)
            Column({ space: 2 }) {
              Button('↓', { buttonStyle: ButtonStyleMode.NORMAL })
                .type(ButtonType.Circle)
                .width(44)
                .height(44)
                .fontSize(20)
                .fontColor(Color.White)
                .backgroundColor('#4CAF50')
                .onClick(() => {
                  this.download();
                })
              Text($r('app.string.download_to_local')).fontSize(10).fontColor('#666666')
            }
            .opacity(this.remoteHasSel ? 1 : 0.4)
          }
          .width(52)
          .height('100%')
          .justifyContent(FlexAlign.Center)

          // ---- remote panel ----
          Column() {
            this.PanelHeader($r('app.string.remote'), this.remoteDir, (): void => {
              this.remoteUp();
            }, (): void => {
              this.refreshRemote();
            })
            List({ space: 2 }) {
              ForEach(this.remoteEntries, (e: FileEntry) => {
                ListItem() {
                  this.FileRow(e, this.remoteHasSel && this.remoteSel.path === e.path, (): void => {
                    this.remoteSel = e;
                    this.remoteHasSel = true;
                  }, (): void => {
                    this.enterRemote(e);
                  })
                }
              }, (e: FileEntry) => `remote_${e.path}`)
            }
            .width('100%')
            .layoutWeight(1)
            if (this.remoteEntries.length === 0) {
              Text($r('app.string.empty_dir')).fontSize(12).fontColor('#999999')
                .margin({ top: 20 })
            }
          }
          .layoutWeight(1)
          .height('100%')
          .backgroundColor('#FAFAFA')
          .borderRadius(8)
          .padding(4)
        }
        .width('100%')
        .layoutWeight(1)
        .padding(6)

        // ---- transfer job list ----
        if (this.jobs.length > 0) {
          Column() {
            Row() {
              Text($r('app.string.transfer_list'))
                .fontSize(12)
                .fontWeight(FontWeight.Medium)
                .fontColor('#666666')
              Blank()
              Text($r('app.string.transfer_clear'))
                .fontSize(12)
                .fontColor('#3A7BD5')
                .padding({ left: 8, right: 4, top: 4, bottom: 4 })
                .onClick(() => {
                  this.clearFinishedJobs();
                })
            }
            .width('100%')
            .padding({ left: 10, right: 6, top: 4 })
            List({ space: 4 }) {
              ForEach(this.jobs, (j: JobItem) => {
                ListItem() {
                  this.JobRow(j)
                }
              }, (j: JobItem) =>
                `job_${j.id}_${j.done ? 1 : 0}_${j.err}_${Math.floor(j.finished)}_${Math.floor(j.speed)}`)
            }
            .width('100%')
            .layoutWeight(1)
            .scrollBar(BarState.Auto)
          }
          .width('100%')
          .height(170)
          .backgroundColor('#F5F5F5')
        }
      }
      .width('100%')
      .height('100%')
      .backgroundColor('#FFFFFF')

      // ---- override confirm dialog ----
      if (this.showOverrideDialog) {
        Column() {
          Column() {
            Text($r('app.string.override_confirm_title'))
              .fontSize(16).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            Scroll() {
              Text(this.overrideText).fontSize(13).fontColor('#555555')
            }
            .constraintSize({ maxHeight: 120 })
            .align(Alignment.Start)
            Row({ space: 12 }) {
              Button($r('app.string.skip'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.answerOverride(false);
                })
              Button($r('app.string.override'))
                .layoutWeight(1)
                .onClick(() => {
                  this.answerOverride(true);
                })
            }
            .width('100%')
            .margin({ top: 12 })
          }
          .width('82%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0,0,0,0.45)')
        .justifyContent(FlexAlign.Center)
      }

      // ---- password dialog ----
      if (this.showPasswordDialog) {
        Column() {
          Column() {
            Text($r('app.string.password_required'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            if (this.msgboxText.length > 0) {
              Text(this.msgboxText).fontSize(13).fontColor('#F44336').margin({ bottom: 8 })
            }
            TextInput({ placeholder: $r('app.string.password_hint'), text: this.passwordInput })
              .type(InputType.Password)
              .width('100%')
              .height(42)
              .onChange((v: string) => {
                this.passwordInput = v;
              })
            Row({ space: 8 }) {
              Toggle({ type: ToggleType.Checkbox, isOn: this.rememberPassword })
                .onChange((v: boolean) => {
                  this.rememberPassword = v;
                })
              Text($r('app.string.remember_password')).fontSize(13)
              Blank()
            }
            .width('100%')
            .margin({ top: 10 })
            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showPasswordDialog = false;
                  router.back();
                })
              Button($r('app.string.ok'))
                .layoutWeight(1)
                .onClick(() => {
                  this.answerPassword();
                })
            }
            .width('100%')
            .margin({ top: 14 })
          }
          .width('82%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0,0,0,0.45)')
        .justifyContent(FlexAlign.Center)
      }

      // ---- 2FA dialog ----
      if (this.showTwofaDialog) {
        Column() {
          Column() {
            Text($r('app.string.twofa_required'))
              .fontSize(18).fontWeight(FontWeight.Medium).margin({ bottom: 8 })
            if (this.msgboxText.length > 0) {
              Text(this.msgboxText).fontSize(13).fontColor('#555555').margin({ bottom: 8 })
            }
            TextInput({ placeholder: $r('app.string.twofa_hint'), text: this.twofaInput })
              .type(InputType.Number)
              .width('100%')
              .height(42)
              .onChange((v: string) => {
                this.twofaInput = v;
              })
            Row({ space: 8 }) {
              Toggle({ type: ToggleType.Checkbox, isOn: this.trustDevice })
                .onChange((v: boolean) => {
                  this.trustDevice = v;
                })
              Text($r('app.string.trust_device')).fontSize(13)
              Blank()
            }
            .width('100%')
            .margin({ top: 10 })
            Row({ space: 12 }) {
              Button($r('app.string.cancel'), { buttonStyle: ButtonStyleMode.NORMAL })
                .layoutWeight(1)
                .onClick(() => {
                  this.showTwofaDialog = false;
                  router.back();
                })
              Button($r('app.string.ok'))
                .layoutWeight(1)
                .onClick(() => {
                  this.answer2fa();
                })
            }
            .width('100%')
            .margin({ top: 14 })
          }
          .width('82%')
          .padding(16)
          .borderRadius(12)
          .backgroundColor('#FFFFFF')
        }
        .width('100%')
        .height('100%')
        .backgroundColor('rgba(0,0,0,0.45)')
        .justifyContent(FlexAlign.Center)
      }
    }
    .width('100%')
    .height('100%')
  }
}
