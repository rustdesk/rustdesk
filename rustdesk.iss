//This installation package automatically recognizes the X86, X64 environment
//The material comes from the Internet, I have searched many search engines, I hope it can help you
#define MyAppName "RustDesk"
#define MyAppVersion "1.1.9"
#define MyAppPublisher "马齿苋科技（北京）有限公司"
#define MyAppURL "https://rustdesk.com/"
#define MyAppExeName "RustDesk.exe"

[Setup]
AppId={{F4279221-60E8-4AD0-9968-3C7524D4B8D3}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={pf}\{#MyAppName}
DisableProgramGroupPage=yes
DefaultGroupName={#MyAppName}
OutputBaseFilename=RustDesk_1.1.9_x86_x64
//custom pack icon
SetupIconFile=D:\NAS\RustDesk\42550.ico
Compression=lzma
SolidCompression=yes
UninstallDisplayIcon={app}\{#MyAppExeName}
ArchitecturesInstallIn64BitMode=x64
//file path
InfoBeforeFile=D:\NAS\RustDesk\Notice.txt
//Please modify it into your own language
[Languages]
Name: "chinesesimp"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: checkablealone

[Files]
Source: "D:\NAS\RustDesk\x86\1.1.9\RustDesk.exe"; DestDir: "{app}";  Flags: ignoreversion; check: not IsWin64
Source: "D:\NAS\RustDesk\x64\1.1.9\RustDesk.exe"; DestDir: "{app}";  Flags: ignoreversion; check: IsWin64;
//The purpose of setting RustDesk2.toml is to preset the address and encryption information of the server in advance. After the installation is completed, you can use it without entering the software to configure the connection parameters and key parameters.
Source: "D:\NAS\RustDesk\rust\RustDesk.toml"; DestDir: "C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\RustDesk\config"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "D:\NAS\RustDesk\rust\RustDesk2.toml"; DestDir: "C:\Windows\ServiceProfiles\LocalService\AppData\Roaming\RustDesk\config"; Flags: ignoreversion
//The purpose of setting RustDesk2.toml is to preset the address and encryption information of the server in advance. After the installation is completed, you can use it without entering the software to configure the connection parameters and key parameters.
Source: "D:\NAS\RustDesk\rust\RustDesk.toml"; DestDir: "{userdocs}\..\AppData\Roaming\RustDesk\config"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "D:\NAS\RustDesk\rust\RustDesk2.toml"; DestDir: "{userdocs}\..\AppData\Roaming\RustDesk\config"; Flags: ignoreversion
Source: compiler:psvince.dll;Flags: dontcopy noencryption

[code]
// function IsModuleLoaded to call at install time
// added also setuponly flag
function IsModuleLoaded(modulename: String ):  Boolean;
external 'IsModuleLoaded@files:psvince.dll stdcall setuponly';

//;Check if a process exists
function IsAppRunning(const FileName : string): Boolean;
var
    FSWbemLocator: Variant;
    FWMIService   : Variant;
    FWbemObjectSet: Variant;
begin
    Result := false;
    try
      FSWbemLocator := CreateOleObject('WBEMScripting.SWBEMLocator');
      FWMIService := FSWbemLocator.ConnectServer('', 'root\CIMV2', '', '');
      FWbemObjectSet := FWMIService.ExecQuery(Format('SELECT Name FROM Win32_Process Where Name="%s"',[FileName]));
      Result := (FWbemObjectSet.Count > 0);
      FWbemObjectSet := Unassigned;
      FWMIService := Unassigned;
      FSWbemLocator := Unassigned;
    except
      if (IsModuleLoaded(FileName)) then
        begin
          Result := false;
        end
      else
        begin
          Result := true;
        end
      end;
end;

//;Terminate process by name
procedure TaskKillProcessByName(AppName: String);
var
  WbemLocator : Variant;
  WMIService   : Variant;
  WbemObjectSet: Variant;
  WbemObject   : Variant;
begin;
  WbemLocator := CreateOleObject('WbemScripting.SWbemLocator');
  WMIService := WbemLocator.ConnectServer('localhost', 'root\CIMV2');
  WbemObjectSet := WMIService.ExecQuery('SELECT * FROM Win32_Process Where Name="' + AppName + '"');
  if not VarIsNull(WbemObjectSet) and (WbemObjectSet.Count > 0) then
  begin
    WbemObject := WbemObjectSet.ItemIndex(0);
    if not VarIsNull(WbemObject) then
    begin
      WbemObject.Terminate();
      WbemObject := Unassigned;
    end;
  end;
end;

//;When installing, it is judged whether the process exists, and if it exists, it will prompt whether to end the process first.
function InitializeSetup(): Boolean;
begin
  Result := true;
  if  IsAppRunning('{#MyAppExeName}') then
  begin
    if MsgBox('Installer detected {#MyAppName} running！'#13''#13'Click the "Yes" button to close the program and continue with the installation；'#13''#13'Click the "No" button to exit the installation！', mbConfirmation, MB_YESNO) = IDYES then
    begin
      TaskKillProcessByName('{#MyAppExeName}');
      TaskKillProcessByName('{#MyAppExeName}');
      Result:= true;
    end
    else
      Result:= false;
  end;
end;

[Icons]
Name: "{group}\RustDesk"; Filename: "{app}\{#MyAppExeName}"; Check: not IsWin64
Name: "{group}\RustDesk"; Filename: "{app}\{#MyAppExeName}"; Check: IsWin64
Name: "{commondesktop}\RustDesk"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; Check: not IsWin64
Name: "{commondesktop}\RustDesk"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; Check: IsWin64

[Run]
Filename: "sc.exe"; Parameters: "create {#MyAppName} start= auto DisplayName= ""{#MyAppName} Service"" binPath= ""\""{app}\{#MyAppExeName}\"" --service"""; Flags: runhidden
Filename: "netsh.exe"; Parameters: "advfirewall firewall add rule name=""{#MyAppName} Service"" dir=in action=allow program=""{app}\{#MyAppExeName}"" enable=yes"; Flags: runhidden
Filename: "sc.exe"; Parameters: "start {#MyAppName}" ; Flags: runhidden
Filename: "{app}\{#MyAppExeName}"; Description:"Please tick Run now";Flags: postinstall nowait skipifsilent

[UninstallRun]
Filename: "sc.exe"; Parameters: "stop {#MyAppName}" ; Flags: runhidden   
Filename: "sc.exe"; Parameters: "delete {#MyAppName}" ; Flags: runhidden
Filename: "netsh.exe"; Parameters: "advfirewall firewall delete rule name=""{#MyAppName} Service"""; Flags: runhidden

