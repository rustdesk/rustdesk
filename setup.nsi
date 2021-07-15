Unicode true

####################################################################
# Includes

!include nsDialogs.nsh
!include MUI2.nsh
!include x64.nsh
!include LogicLib.nsh

####################################################################
# File Info

!define PRODUCT_NAME "RustDesk"
!define PRODUCT_DESCRIPTION "Installer for ${PRODUCT_NAME}"
!define COPYRIGHT "Copyright © 2021"
!define VERSION "1.1.6"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "${PRODUCT_NAME}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "FileDescription" "${PRODUCT_DESCRIPTION}"
VIAddVersionKey "LegalCopyright" "${COPYRIGHT}"
VIAddVersionKey "FileVersion" "${VERSION}.0"

####################################################################
# Installer Attributes

Name "${PRODUCT_NAME}"
Outfile "rustdesk-${VERSION}-setup.exe"
Caption "Setup - ${PRODUCT_NAME}"
BrandingText "${PRODUCT_NAME}"

ShowInstDetails show
RequestExecutionLevel admin
SetOverwrite on
 
InstallDir "$PROGRAMFILES64\${PRODUCT_NAME}"

####################################################################
# Pages

!define MUI_ICON "src\tray-icon.ico"
!define MUI_ABORTWARNING
!define MUI_LANGDLL_ALLLANGUAGES
!define MUI_FINISHPAGE_SHOWREADME ""
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED
!define MUI_FINISHPAGE_SHOWREADME_TEXT "Create Desktop Shortcut"
!define MUI_FINISHPAGE_SHOWREADME_FUNCTION CreateDesktopShortcut
!define MUI_FINISHPAGE_RUN "$INSTDIR\${PRODUCT_NAME}.exe"

!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

####################################################################
# Language

!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

####################################################################
# Sections

Section "Install"
  SetOutPath $INSTDIR

  # Regkeys
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayIcon" "$INSTDIR\${PRODUCT_NAME}.exe"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayName" "${PRODUCT_NAME} (x64)"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "UninstallString" '"$INSTDIR\${PRODUCT_NAME}.exe" --uninstall'
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "Publisher" "Carriez, Inc."
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "HelpLink" "https://www.rustdesk.com/"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "URLInfoAbout" "https://www.rustdesk.com/"
  WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_NAME}" "URLUpdateInfo" "https://www.rustdesk.com/"

  nsExec::Exec "taskkill /F /IM ${PRODUCT_NAME}.exe"
  Sleep 500 ; Give time for process to be completely killed
  File "${PRODUCT_NAME}.exe"

  SetShellVarContext all
  CreateShortCut "$INSTDIR\Uninstall ${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_NAME}.exe" "--uninstall" "msiexec.exe"
  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_NAME}.exe"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall ${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_NAME}.exe" "--uninstall" "msiexec.exe"
  CreateShortCut "$SMSTARTUP\${PRODUCT_NAME} Tray.lnk" "$INSTDIR\${PRODUCT_NAME}.exe" "--tray"
  
  nsExec::Exec 'sc create ${PRODUCT_NAME} start=auto DisplayName="${PRODUCT_NAME} Service" binPath= "\"$INSTDIR\${PRODUCT_NAME}.exe\" --service"'
  nsExec::Exec 'netsh advfirewall firewall add rule name="${PRODUCT_NAME} Service" dir=in action=allow program="$INSTDIR\${PRODUCT_NAME}.exe" enable=yes'
  nsExec::Exec 'sc start ${PRODUCT_NAME}'
SectionEnd

####################################################################
# Functions

Function .onInit
  # RustDesk is 64-bit only
  ${IfNot} ${RunningX64}
    MessageBox MB_ICONSTOP "${PRODUCT_NAME} is 64-bit only!"
    Quit
  ${EndIf}
  ${DisableX64FSRedirection}
  SetRegView 64

  !insertmacro MUI_LANGDLL_DISPLAY
FunctionEnd

Function CreateDesktopShortcut
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" "$INSTDIR\${PRODUCT_NAME}.exe"
FunctionEnd