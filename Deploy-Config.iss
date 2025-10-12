[Setup]
AppName=RustDesk Configuration Deployment
AppVersion=1.0
AppPublisher=Cislink
DefaultDirName={tmp}\RustDeskConfigDeploy
DisableDirPage=yes
DisableProgramGroupPage=yes
OutputBaseFilename=RustDesk_Config_Installer
Compression=lzma
SolidCompression=yes
PrivilegesRequired=admin
; 不创建卸载程序
Uninstallable=no
; 自动检测系统语言
ShowLanguageDialog=no
; 使用现代化的向导样式
WizardStyle=modern
; 设置图标（如果有的话）
; SetupIconFile=icon.ico

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; PowerShell 部署脚本
Source: "Deploy-RustDeskConfig.ps1"; Flags: dontcopy

[Code]
procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  PowerShellCmd: String;
  ScriptPath: String;
begin
  if CurStep = ssInstall then
  begin
    // 提取 PowerShell 脚本到临时目录
    ExtractTemporaryFile('Deploy-RustDeskConfig.ps1');
    ScriptPath := ExpandConstant('{tmp}\Deploy-RustDeskConfig.ps1');
    
    // 构建 PowerShell 命令
    PowerShellCmd := Format('-NoProfile -ExecutionPolicy Bypass -File "%s" -Silent -RestartService', [ScriptPath]);
    
    // 显示进度
    WizardForm.StatusLabel.Caption := 'Deploying RustDesk configuration...';
    WizardForm.ProgressGauge.Style := npbstMarquee;
    
    // 执行 PowerShell 脚本
    if not Exec('powershell.exe', PowerShellCmd, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    begin
      MsgBox('Failed to execute deployment script. Error code: ' + IntToStr(ResultCode), mbError, MB_OK);
    end
    else
    begin
      if ResultCode = 0 then
      begin
        MsgBox('RustDesk configuration has been successfully deployed!' + #13#10 + #13#10 + 
               'Server: hbbs.cislink.nl' + #13#10 +
               'Key: wrrkMLBXkBGYVlvErzCFMHabakrxKQCsEX2lIbap5Jo=', 
               mbInformation, MB_OK);
      end
      else
      begin
        MsgBox('Deployment completed with warnings. Please check the log file in %TEMP%\RustDesk_Config_Deploy.log', 
               mbError, MB_OK);
      end;
    end;
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
  // 检查是否以管理员身份运行
  if not IsAdminLoggedOn then
  begin
    MsgBox('This installer requires Administrator privileges.' + #13#10 + 
           'Please run as Administrator.', mbError, MB_OK);
    Result := False;
  end;
end;

procedure InitializeWizard();
begin
  WizardForm.LicenseAcceptedRadio.Checked := True;
end;
