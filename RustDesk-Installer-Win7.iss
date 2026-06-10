; RustDesk Full Installer - Pre-configured for Cislink Server (Windows 7 Compatible)
; This installer contains RustDesk 1.3.4 client and pre-configured self-hosted server settings
; Compatible with Windows 7 and later versions
;
; Build Requirements:
; 1. Inno Setup 6.x or higher
; 2. RustDesk client (rustdesk-1.3.4.exe) in same directory as this script
; 3. Run Inno Setup to compile this script
;
; Server Information:
; - ID Server: hbbs.cislink.nl
; - Relay Server: hbbr.cislink.nl
; - Public Key: VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=

#define MyAppName "RustDesk - Cislink Edition"
#define MyAppVersion "1.3.4"
#define MyAppPublisher "Cislink"
#define MyAppURL "https://cislink.nl"
#define MyAppExeName "rustdesk-host=hbbs.cislink.nl,key=VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY=,relay=hbbr.cislink.nl,.exe"

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
OutputBaseFilename=RustDesk_Cislink_Installer_v{#MyAppVersion}_Win7
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ShowLanguageDialog=auto
; Custom icon configuration - using Cislink custom icon
SetupIconFile=res\cislink.ico
UninstallDisplayIcon={app}\cislink.ico
; Windows 7 compatibility
MinVersion=6.1

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "dutch"; MessagesFile: "compiler:Languages\Dutch.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "autostart"; Description: "Start RustDesk automatically"; GroupDescription: "Additional options:"

[Files]
Source: "rustdesk-1.3.4.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "res\cislink.ico"; DestDir: "{app}"; Flags: ignoreversion
Source: "RustDesk_Config_Template.toml"; DestDir: "{tmp}"; Flags: dontcopy

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\cislink.ico"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; IconFilename: "{app}\cislink.ico"
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: autostart; IconFilename: "{app}\cislink.ico"

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
  // Stop all rustdesk processes regardless of filename
  Exec('taskkill', '/F /IM rustdesk*.exe /T', '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
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
                   'key = "VXz1DqnNLuvAnsiTM6N1BnOkN37zCiEEikhsrZumpfY="' + #13#10 +
                   'enable-check-update = "N"' + #13#10 +
                   'disable-installation = true' + #13#10;

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

  // Check for any rustdesk process (wildcard not supported in tasklist filter, so we check the output)
  if Exec('powershell', '-Command "Get-Process rustdesk* -ErrorAction SilentlyContinue"', '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
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

procedure CleanupOldRustDeskFiles();
var
  FindRec: TFindRec;
  InstallPath: String;
  OldFile: String;
  NewExeName: String;
begin
  Log('Cleaning up old RustDesk files...');
  InstallPath := ExpandConstant('{app}');
  NewExeName := '{#MyAppExeName}';

  if FindFirst(InstallPath + '\rustdesk*.exe', FindRec) then
  begin
    try
      repeat
        // Delete all rustdesk*.exe files except our new one
        if CompareText(FindRec.Name, NewExeName) <> 0 then
        begin
          OldFile := InstallPath + '\' + FindRec.Name;
          Log('Deleting old file: ' + OldFile);
          DeleteFile(OldFile);
        end;
      until not FindNext(FindRec);
    finally
      FindClose(FindRec);
    end;
  end;
  Log('Old files cleanup completed');
end;

procedure UpdateAllShortcuts();
var
  NewExePath: String;
  ShortcutPath: String;
begin
  Log('Updating shortcuts to point to new executable...');
  NewExePath := ExpandConstant('{app}\{#MyAppExeName}');

  // Update desktop shortcut if it exists
  ShortcutPath := ExpandConstant('{autodesktop}\{#MyAppName}.lnk');
  if FileExists(ShortcutPath) then
  begin
    DeleteFile(ShortcutPath);
    CreateShellLink(
      ShortcutPath,
      '{#MyAppName}',
      NewExePath,
      '',
      '',
      '',
      0,
      SW_SHOWNORMAL
    );
    Log('Updated desktop shortcut');
  end;

  // Update start menu shortcut
  ShortcutPath := ExpandConstant('{group}\{#MyAppName}.lnk');
  if FileExists(ShortcutPath) then
  begin
    DeleteFile(ShortcutPath);
    CreateShellLink(
      ShortcutPath,
      '{#MyAppName}',
      NewExePath,
      '',
      '',
      '',
      0,
      SW_SHOWNORMAL
    );
    Log('Updated start menu shortcut');
  end;
end;

procedure ForceResetServerSettings();
var
  UserConfigDir: String;
  SystemConfigDir: String;
begin
  Log('Force resetting all server settings...');

  // Delete all existing configuration files to ensure clean slate
  UserConfigDir := ExpandConstant('{userappdata}\RustDesk\config');
  SystemConfigDir := ExpandConstant('{commonappdata}\RustDesk\config');

  // Remove user configs
  if DirExists(UserConfigDir) then
  begin
    DelTree(UserConfigDir, True, False, True);
    Log('Deleted user config directory');
  end;

  // Remove system configs
  if DirExists(SystemConfigDir) then
  begin
    DelTree(SystemConfigDir, True, False, True);
    Log('Deleted system config directory');
  end;

  // Also delete RustDesk2.toml files (runtime user settings)
  DeleteFile(ExpandConstant('{userappdata}\RustDesk\config\RustDesk2.toml'));
  DeleteFile(ExpandConstant('{commonappdata}\RustDesk\config\RustDesk2.toml'));

  Log('All old configurations cleared');
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    StopRustDesk();
    CleanupOldRustDeskFiles();
    ForceResetServerSettings();  // Force clear all old settings
    CreateConfigFiles();         // Create fresh config with our server
    UpdateAllShortcuts();
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
