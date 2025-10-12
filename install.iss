[Setup]
AppName=cisAdmin
AppVersion=1.0
DefaultDirName={autopf}\cislink-admin
DefaultGroupName=cisAdmin
OutputBaseFilename=cisAdmin_Installer
Compression=lzma
SolidCompression=yes
; 自动检测系统语言，不显示语言选择对话框
ShowLanguageDialog=auto

[Files]
; 主程序
Source: "rustdesk.exe"; DestDir: "{app}"; Flags: ignoreversion
; 配置文件放到用户 AppData 下（部署两个文件名以确保兼容性）
Source: "RustDesk.toml"; DestDir: "{userappdata}\RustDesk\config"; Flags: createallsubdirs recursesubdirs
Source: "RustDesk2.toml"; DestDir: "{userappdata}\RustDesk\config"; Flags: createallsubdirs recursesubdirs
; 配置文件同时放到系统 ProgramData 下（用于无人值守服务）
Source: "RustDesk.toml"; DestDir: "{commonappdata}\RustDesk\config"; Flags: createallsubdirs recursesubdirs
Source: "RustDesk2.toml"; DestDir: "{commonappdata}\RustDesk\config"; Flags: createallsubdirs recursesubdirs

[Icons]
; 桌面快捷方式
Name: "{userdesktop}\cisAdmin"; Filename: "{app}\rustdesk.exe"; WorkingDir: "{app}"
; 开始菜单快捷方式
Name: "{group}\cisAdmin"; Filename: "{app}\rustdesk.exe"; WorkingDir: "{app}"

[Run]
; 停止 RustDesk 服务（如果存在）
Filename: "sc"; Parameters: "stop RustDesk"; Flags: runhidden skipifsilent; StatusMsg: "Stopping RustDesk service..."
; 启动 RustDesk 服务（如果存在）
Filename: "sc"; Parameters: "start RustDesk"; Flags: runhidden skipifsilent; StatusMsg: "Starting RustDesk service..."
; 安装完成后自动运行程序
Filename: "{app}\rustdesk.exe"; Description: "Launch RustDesk"; Flags: nowait postinstall skipifsilent
