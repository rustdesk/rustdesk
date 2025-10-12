; RustDesk Full Installer - Pre-configured for Cislink Server
; This installer contains RustDesk client and pre-configured self-hosted server settings
;
; Build Requirements:
; 1. Inno Setup 6.x or higher
; 2. RustDesk client (rustdesk.exe) in same directory as this script
; 3. Run Inno Setup to compile this script
;
; Server Information:
; - ID Server: hbbs.cislink.nl
; - Relay Server: hbbr.cislink.nl
; - Public Key: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=

#define MyAppName "RustDesk - Cislink Edition"
#define MyAppVersion "1.0"
#define MyAppPublisher "Cislink"
#define MyAppURL "https://cislink.nl"
#define MyAppExeName "rustdesk.exe"

[Setup]
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
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ShowLanguageDialog=auto
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "dutch"; MessagesFile: "compiler:Languages\Dutch.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start RustDesk automatically"; GroupDescription: "Additional options:"

[Files]
Source: "rustdesk.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "RustDesk_Config_Template.toml"; DestDir: "{tmp}"; Flags: dontcopy

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: autostart

[Registry]
Root: HKCR; Subkey: "rustdesk"; ValueType: string; ValueName: ""; ValueData: "URL:rustdesk Protocol"; Flags: uninsdeletekey
Root: HKCR; Subkey: "rustdesk"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; 
Root: HKCR; Subkey: "rustdesk\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

[Code]
var
  NeedRestart: Boolean;

function StopRustDesk(): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  Log('Stopping RustDesk processes...');
  Exec('taskkill', '/F /IM rustdesk.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
  Sleep(1000);
end;

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
  
  ConfigContent := '[options]' + #13#10 +
                   'custom-rendezvous-server = "hbbs.cislink.nl"' + #13#10 +
                   'relay-server = "hbbr.cislink.nl"' + #13#10 +
                   'key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="' + #13#10;
  
  UserConfigDir := ExpandConstant('{userappdata}\RustDesk\config');
  SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk\config');
  
  ForceDirectories(UserConfigDir);
  ForceDirectories(SystemConfigDir);
  
  UserConfigFile := UserConfigDir + '\RustDesk.toml';
  UserConfig2File := UserConfigDir + '\RustDesk2.toml';
  SystemConfigFile := SystemConfigDir + '\RustDesk.toml';
  SystemConfig2File := SystemConfigDir + '\RustDesk2.toml';
  
  SaveStringToFile(UserConfigFile, ConfigContent, False);
  SaveStringToFile(UserConfig2File, ConfigContent, False);
  SaveStringToFile(SystemConfigFile, ConfigContent, False);
  SaveStringToFile(SystemConfig2File, ConfigContent, False);
  
  Log('Configuration files created successfully');
end;

function InitializeSetup(): Boolean;
var
  ResultCode: Integer;
begin
  Result := True;
  
  if Exec('tasklist', '/FI "IMAGENAME eq rustdesk.exe" /NH', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
  begin
    if ResultCode = 0 then
    begin
      if MsgBox('RustDesk is currently running. Setup needs to close it to continue.' + #13#10 + #13#10 + 
                'Continue?', mbConfirmation, MB_YESNO) = IDYES then
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

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    StopRustDesk();
    CreateConfigFiles();
    Sleep(500);
  end;
end;

function InitializeUninstall(): Boolean;
begin
  Result := True;
  
  if MsgBox('Do you want to completely remove RustDesk (including configuration files)?' + #13#10 + #13#10 +
            'Yes = Delete all configurations' + #13#10 +
            'No = Keep configuration files', mbConfirmation, MB_YESNO) = IDYES then
  begin
    RegWriteStringValue(HKEY_CURRENT_USER, 'Software\RustDesk', 'DeleteConfig', '1');
  end;
  
  StopRustDesk();
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  DeleteConfig: String;
  UserConfigDir: String;
  SystemConfigDir: String;
begin
  if CurUninstallStep = usPostUninstall then
  begin
    if RegQueryStringValue(HKEY_CURRENT_USER, 'Software\RustDesk', 'DeleteConfig', DeleteConfig) then
    begin
      if DeleteConfig = '1' then
      begin
        UserConfigDir := ExpandConstant('{userappdata}\RustDesk');
        SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk');
        
        DelTree(UserConfigDir, True, True, True);
        DelTree(SystemConfigDir, True, True, True);
        
        RegDeleteKeyIncludingSubkeys(HKEY_CURRENT_USER, 'Software\RustDesk');
      end;
    end;
  end;
end;

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
Type: filesandordirs; Name: "{userappdata}\RustDesk\logs"
Type: filesandordirs; Name: "{localappdata}\RustDesk"
