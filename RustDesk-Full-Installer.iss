; RustDesk 客户端完整安装包 - 预配置 Cislink 服务器
; 此安装包包含 RustDesk 客户端和预配置的自托管服务器设置
; 
; 构建要求:
; 1. Inno Setup 6.x 或更高版本
; 2. RustDesk 客户端程序 (rustdesk.exe) 放在此目录下
; 3. 运行 Inno Setup 编译此脚本生成安装包
;
; 服务器信息:
; - ID 服务器: hbbs.cislink.nl
; - 中继服务器: hbbr.cislink.nl
; - 公钥: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=

#define MyAppName "RustDesk - Cislink Edition"
#define MyAppVersion "1.0"
#define MyAppPublisher "Cislink"
#define MyAppURL "https://cislink.nl"
#define MyAppExeName "rustdesk.exe"

[Setup]
; 基本信息
AppId={{A7B8C9D0-E1F2-4A5B-9C8D-7E6F5A4B3C2D}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\RustDesk
DefaultGroupName=RustDesk
DisableProgramGroupPage=yes
OutputBaseFilename=RustDesk_Cislink_Installer_v{#MyAppVersion}
; SetupIconFile=res\icon.ico
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
; 自动检测系统语言
ShowLanguageDialog=auto
; 创建卸载程序
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "dutch"; MessagesFile: "compiler:Languages\Dutch.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start RustDesk automatically on system startup"; GroupDescription: "Additional options:"; Flags: checked

[Files]
; 主程序 - 需要将 rustdesk.exe 放在脚本同目录下
Source: "rustdesk.exe"; DestDir: "{app}"; Flags: ignoreversion
; 配置文件模板
Source: "RustDesk_Config_Template.toml"; DestDir: "{tmp}"; Flags: dontcopy

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: autostart

[Registry]
; 创建 URL 协议处理
Root: HKCR; Subkey: "rustdesk"; ValueType: string; ValueName: ""; ValueData: "URL:rustdesk Protocol"; Flags: uninsdeletekey
Root: HKCR; Subkey: "rustdesk"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; 
Root: HKCR; Subkey: "rustdesk\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

[Code]
var
  NeedRestart: Boolean;

// 停止 RustDesk 进程
function StopRustDesk(): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  Log('Stopping RustDesk processes...');
  
  // 尝试通过 taskkill 停止进程
  Exec('taskkill', '/F /IM rustdesk.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  
  // 等待一秒确保进程完全停止
  Sleep(1000);
end;

// 创建配置文件
procedure CreateConfigFiles();
var
  ConfigContent: String;
  UserConfigDir: String;
  SystemConfigDir: String;
  UserConfigFile: String;
  SystemConfigFile: String;
  UserConfig2File: String;
  SystemConfig2File: String;
begin
  Log('Creating configuration files...');
  
  // 配置文件内容
  ConfigContent := '[options]' + #13#10 +
                   'custom-rendezvous-server = "hbbs.cislink.nl"' + #13#10 +
                   'relay-server = "hbbr.cislink.nl"' + #13#10 +
                   'key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="' + #13#10;
  
  // 用户配置目录
  UserConfigDir := ExpandConstant('{userappdata}\RustDesk\config');
  SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk\config');
  
  // 创建目录
  ForceDirectories(UserConfigDir);
  ForceDirectories(SystemConfigDir);
  
  // 文件路径
  UserConfigFile := UserConfigDir + '\RustDesk.toml';
  UserConfig2File := UserConfigDir + '\RustDesk2.toml';
  SystemConfigFile := SystemConfigDir + '\RustDesk.toml';
  SystemConfig2File := SystemConfigDir + '\RustDesk2.toml';
  
  // 写入配置文件
  SaveStringToFile(UserConfigFile, ConfigContent, False);
  SaveStringToFile(UserConfig2File, ConfigContent, False);
  SaveStringToFile(SystemConfigFile, ConfigContent, False);
  SaveStringToFile(SystemConfig2File, ConfigContent, False);
  
  Log('Configuration files created successfully');
end;

// 安装前检查
function InitializeSetup(): Boolean;
var
  ResultCode: Integer;
  RunningProcesses: String;
begin
  Result := True;
  
  // 检查是否有 RustDesk 进程正在运行
  if Exec('tasklist', '/FI "IMAGENAME eq rustdesk.exe" /NH', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    if ResultCode = 0 then
    begin
      if MsgBox('RustDesk 正在运行。安装程序需要关闭它才能继续。' + #13#10 + #13#10 + 
                '是否继续？', mbConfirmation, MB_YESNO) = IDYES then
      begin
        NeedRestart := True;
        StopRustDesk();
      end
      else
      begin
        Result := False;
      end;
    end;
  end;
end;

// 安装步骤改变时的处理
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    // 停止可能存在的 RustDesk 服务
    StopRustDesk();
    
    // 创建配置文件
    CreateConfigFiles();
    
    // 等待配置文件写入完成
    Sleep(500);
  end;
end;

// 卸载初始化
function InitializeUninstall(): Boolean;
begin
  Result := True;
  
  if MsgBox('是否完全删除 RustDesk（包括配置文件）？' + #13#10 + #13#10 +
            '选择"是"将删除所有配置' + #13#10 +
            '选择"否"将保留配置文件', mbConfirmation, MB_YESNO) = IDYES then
  begin
    // 标记需要删除配置
    RegWriteStringValue(HKEY_CURRENT_USER, 'Software\RustDesk', 'DeleteConfig', '1');
  end;
  
  // 停止 RustDesk
  StopRustDesk();
end;

// 卸载完成后
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DeleteConfig: String;
  UserConfigDir: String;
  SystemConfigDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // 检查是否需要删除配置
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Software\RustDesk', 'DeleteConfig', DeleteConfig) then
    begin
      if DeleteConfig = '1' then
      begin
        UserConfigDir := ExpandConstant('{userappdata}\RustDesk');
        SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk');
        
        // 删除配置目录
        DelTree(UserConfigDir, True, True, True);
        DelTree(SystemConfigDir, True, True, True);
        
        // 清理注册表
        RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, 'Software\RustDesk');
      end;
    end;
  end;
end;

[Run]
; 安装完成后可选择启动 RustDesk
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; 清理日志和临时文件
Type: filesandordirs; Name: "{userappdata}\RustDesk\logs"
Type: filesandordirs; Name: "{localappdata}\RustDesk"
