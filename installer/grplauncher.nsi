; GRP Launcher per-user NSIS installer.
; Installs into %LOCALAPPDATA%\GRPLauncher (no admin rights required, so in-place self-updates keep working),
; installs Microsoft Edge WebView2 Runtime if missing, and keeps the ~4 GB game folder untouched on uninstall.
;
; Build with:
;   makensis -NOCD -INPUTCHARSET UTF8 -DAPP_VERSION=0.20.0 -DDIST_DIR=dist -DICON_FILE=src-tauri/icons/icon.ico \
;            -DWEBVIEW2_BOOTSTRAPPER=MicrosoftEdgeWebview2Setup.exe \
;            -DOUT_FILE=dist/GRPLauncher-Setup.exe installer/grplauncher.nsi

Unicode True
ManifestDPIAware true
RequestExecutionLevel user
SetCompressor /SOLID lzma

!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "LogicLib.nsh"

!ifndef APP_VERSION
  !define APP_VERSION "0.20.0"
!endif
!ifndef DIST_DIR
  !define DIST_DIR "..\dist"
!endif
!ifndef ICON_FILE
  !define ICON_FILE "..\src-tauri\icons\icon.ico"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "${DIST_DIR}\GRPLauncher-Setup.exe"
!endif

!define APP_NAME "GRP Launcher"
!define APP_EXE "GRPLauncher.exe"
!define PUBLISHER "Aquilabot"
!define APP_URL "https://github.com/Aquilabot/GRPLauncher"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\GRPLauncher"
!define WV2_CLIENT_KEY "Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

Name "${APP_NAME} ${APP_VERSION}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\GRPLauncher"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
BrandingText "${APP_NAME} ${APP_VERSION}"

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "CompanyName" "${PUBLISHER}"
VIAddVersionKey "LegalCopyright" "Copyright (c) 2026 ${PUBLISHER}. MPL-2.0"
VIAddVersionKey "FileDescription" "${APP_NAME} Installer"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"

; Modern UI 2 configuration
!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "$(STR_LAUNCH_APP)"
!define MUI_UNCONFIRMPAGE_TEXT_TOP "$(STR_UNINST_GAME_KEPT)"

; Installer pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

; Uninstaller pages
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages (Spanish primary, English secondary; NSIS selects based on Windows UI language)
!insertmacro MUI_LANGUAGE "Spanish"
!insertmacro MUI_LANGUAGE "English"

LangString STR_SEC_CORE ${LANG_SPANISH} "GRP Launcher (requerido)"
LangString STR_SEC_CORE ${LANG_ENGLISH} "GRP Launcher (required)"

LangString STR_SEC_DESKTOP ${LANG_SPANISH} "Acceso directo en el escritorio"
LangString STR_SEC_DESKTOP ${LANG_ENGLISH} "Desktop shortcut"

LangString STR_DESC_CORE ${LANG_SPANISH} "Instala GRP Launcher en tu carpeta de usuario y, si falta, Microsoft Edge WebView2 Runtime."
LangString STR_DESC_CORE ${LANG_ENGLISH} "Installs GRP Launcher in your user folder and, if missing, Microsoft Edge WebView2 Runtime."

LangString STR_DESC_DESKTOP ${LANG_SPANISH} "Crea un acceso directo a GRP Launcher en el escritorio."
LangString STR_DESC_DESKTOP ${LANG_ENGLISH} "Creates a GRP Launcher shortcut on the desktop."

LangString STR_SHORTCUT_UNINSTALL ${LANG_SPANISH} "Desinstalar GRP Launcher"
LangString STR_SHORTCUT_UNINSTALL ${LANG_ENGLISH} "Uninstall GRP Launcher"

LangString STR_LAUNCH_APP ${LANG_SPANISH} "Abrir GRP Launcher"
LangString STR_LAUNCH_APP ${LANG_ENGLISH} "Launch GRP Launcher"

LangString STR_APP_RUNNING ${LANG_SPANISH} "GRP Launcher está abierto. Ciérralo antes de continuar."
LangString STR_APP_RUNNING ${LANG_ENGLISH} "GRP Launcher is currently running. Close it before continuing."

LangString STR_WV2_CHECKING ${LANG_SPANISH} "Comprobando Microsoft Edge WebView2 Runtime..."
LangString STR_WV2_CHECKING ${LANG_ENGLISH} "Checking Microsoft Edge WebView2 Runtime..."

LangString STR_WV2_FOUND ${LANG_SPANISH} "Microsoft Edge WebView2 Runtime ya está instalado."
LangString STR_WV2_FOUND ${LANG_ENGLISH} "Microsoft Edge WebView2 Runtime is already installed."

LangString STR_WV2_INSTALLING ${LANG_SPANISH} "Instalando Microsoft Edge WebView2 Runtime..."
LangString STR_WV2_INSTALLING ${LANG_ENGLISH} "Installing Microsoft Edge WebView2 Runtime..."

LangString STR_WV2_FAILED ${LANG_SPANISH} "No se pudo instalar Microsoft Edge WebView2 Runtime automáticamente. El launcher se instaló igual; si no abre la ventana, instálalo desde Microsoft o vuelve a ejecutar este instalador con conexión a internet."
LangString STR_WV2_FAILED ${LANG_ENGLISH} "Could not install Microsoft Edge WebView2 Runtime automatically. The launcher was still installed; if its window does not open, install WebView2 from Microsoft or re-run this installer while online."

LangString STR_UNINST_GAME_KEPT ${LANG_SPANISH} "El desinstalador elimina solo GRP Launcher. La carpeta del juego (~4 GB) y tus ajustes se conservan."
LangString STR_UNINST_GAME_KEPT ${LANG_ENGLISH} "The uninstaller removes only GRP Launcher. The game folder (~4 GB) and your settings are kept."

; Returns "1" on the stack if Microsoft Edge WebView2 Runtime is installed (per-machine or per-user), "0" otherwise.
Function CheckWebView2
  Push $0
  Push $1
  StrCpy $1 "0"

  ; 32-bit view of HKLM (HKLM\SOFTWARE\WOW6432Node\... on 64-bit Windows, where EdgeUpdate registers)
  SetRegView 32
  ReadRegStr $0 HKLM "SOFTWARE\${WV2_CLIENT_KEY}" "pv"
  ${If} $0 != ""
  ${AndIf} $0 != "0.0.0.0"
    StrCpy $1 "1"
  ${EndIf}

  ; 64-bit view of HKLM
  ${If} $1 == "0"
    SetRegView 64
    ReadRegStr $0 HKLM "SOFTWARE\${WV2_CLIENT_KEY}" "pv"
    SetRegView 32
    ${If} $0 != ""
    ${AndIf} $0 != "0.0.0.0"
      StrCpy $1 "1"
    ${EndIf}
  ${EndIf}

  ; Per-user HKCU installation
  ${If} $1 == "0"
    ReadRegStr $0 HKCU "Software\${WV2_CLIENT_KEY}" "pv"
    ${If} $0 != ""
    ${AndIf} $0 != "0.0.0.0"
      StrCpy $1 "1"
    ${EndIf}
  ${EndIf}

  Exch $1
  Exch
  Pop $0
FunctionEnd

Function EnsureNotRunning
  ${If} ${FileExists} "$INSTDIR\${APP_EXE}"
    retry:
    ClearErrors
    FileOpen $0 "$INSTDIR\${APP_EXE}" a
    ${If} ${Errors}
      ${If} ${Silent}
        Abort
      ${Else}
        MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(STR_APP_RUNNING)" IDRETRY retry
        Abort
      ${EndIf}
    ${Else}
      FileClose $0
    ${EndIf}
  ${EndIf}
FunctionEnd

Function un.EnsureNotRunning
  ${If} ${FileExists} "$INSTDIR\${APP_EXE}"
    retry:
    ClearErrors
    FileOpen $0 "$INSTDIR\${APP_EXE}" a
    ${If} ${Errors}
      ${If} ${Silent}
        Abort
      ${Else}
        MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "$(STR_APP_RUNNING)" IDRETRY retry
        Abort
      ${EndIf}
    ${Else}
      FileClose $0
    ${EndIf}
  ${EndIf}
FunctionEnd

Function .onInit
  SetShellVarContext current
FunctionEnd

Function un.onInit
  SetShellVarContext current
FunctionEnd

Section "$(STR_SEC_CORE)" SecCore
  SectionIn RO

  Call EnsureNotRunning

  SetOutPath "$INSTDIR"
  SetOverwrite on

  ; Clean up leftover self-update files if any
  Delete "$INSTDIR\${APP_EXE}.old"
  Delete "$INSTDIR\${APP_EXE}.new"

  File "${DIST_DIR}\${APP_EXE}"
  File "${DIST_DIR}\LEEME.txt"
  File "${DIST_DIR}\LICENSE"

  ; Preserve an existing launcher.ini so custom server settings survive reinstalls
  ${IfNot} ${FileExists} "$INSTDIR\launcher.ini"
    File "${DIST_DIR}\launcher.ini"
  ${EndIf}

  ; Install Microsoft Edge WebView2 Runtime if missing
  DetailPrint "$(STR_WV2_CHECKING)"
  Call CheckWebView2
  Pop $0
  ${If} $0 == "1"
    DetailPrint "$(STR_WV2_FOUND)"
  ${Else}
    !ifdef WEBVIEW2_BOOTSTRAPPER
      DetailPrint "$(STR_WV2_INSTALLING)"
      InitPluginsDir
      SetOutPath "$PLUGINSDIR"
      File "${DIST_DIR}\${WEBVIEW2_BOOTSTRAPPER}"
      SetOutPath "$INSTDIR"
      ${If} ${Silent}
        ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /silent /install' $1
      ${Else}
        ExecWait '"$PLUGINSDIR\MicrosoftEdgeWebview2Setup.exe" /install' $1
      ${EndIf}
      Call CheckWebView2
      Pop $0
      ${If} $0 != "1"
        ${IfNot} ${Silent}
          MessageBox MB_OK|MB_ICONEXCLAMATION "$(STR_WV2_FAILED)"
        ${EndIf}
      ${EndIf}
    !endif
  ${EndIf}

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; Start Menu shortcuts
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
  CreateShortcut "$SMPROGRAMS\${APP_NAME}\$(STR_SHORTCUT_UNINSTALL).lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0

  ; Add/Remove Programs (per-user HKCU)
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "${PUBLISHER}"
  WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "${APP_URL}"
  WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE},0"
  WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1

  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKCU "${UNINST_KEY}" "EstimatedSize" "$0"
SectionEnd

Section "$(STR_SEC_DESKTOP)" SecDesktop
  SetOutPath "$INSTDIR"
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}" "" "$INSTDIR\${APP_EXE}" 0
SectionEnd

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
  !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} "$(STR_DESC_CORE)"
  !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} "$(STR_DESC_DESKTOP)"
!insertmacro MUI_FUNCTION_DESCRIPTION_END

Section "Uninstall"
  Call un.EnsureNotRunning

  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\${APP_EXE}.old"
  Delete "$INSTDIR\${APP_EXE}.new"
  Delete "$INSTDIR\launcher.ini"
  Delete "$INSTDIR\LEEME.txt"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\$(STR_SHORTCUT_UNINSTALL).lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Desinstalar GRP Launcher.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\Uninstall GRP Launcher.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"

  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey HKCU "${UNINST_KEY}"
SectionEnd
