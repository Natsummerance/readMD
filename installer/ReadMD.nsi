; ReadMD Windows NSIS Installer Script
; Generates ReadMDSetup-windows-x64.exe

!include "MUI2.nsh"
!include "FileFunc.nsh"

Name "ReadMD"
OutFile "dist\ReadMDSetup-windows-x64.exe"
InstallDir "$LOCALAPPDATA\ReadMD"
InstallDirRegKey HKCU "Software\ReadMD" "Install_Dir"
RequestExecutionLevel user

!define MUI_ABORTWARNING
!define MUI_ICON "assets\readmd.ico"
!define MUI_UNICON "assets\readmd.ico"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; Languages
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "ReadMD Core" SEC01
  SetOutPath "$INSTDIR"
  SetOverwrite on
  
  File "dist\ReadMD-windows-x64\ReadMD.exe"
  File /r "dist\ReadMD-windows-x64\assets"
  File "dist\ReadMD-windows-x64\LICENSE"
  File "dist\ReadMD-windows-x64\README.md"
  
  ; Write install dir to registry
  WriteRegStr HKCU "Software\ReadMD" "Install_Dir" "$INSTDIR"
  
  ; Shortcuts
  CreateDirectory "$SMPROGRAMS\ReadMD"
  CreateShortcut "$SMPROGRAMS\ReadMD\ReadMD.lnk" "$INSTDIR\ReadMD.exe" "" "$INSTDIR\ReadMD.exe" 0
  CreateShortcut "$SMPROGRAMS\ReadMD\Uninstall ReadMD.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\Uninstall.exe" 0
  CreateShortcut "$DESKTOP\ReadMD.lnk" "$INSTDIR\ReadMD.exe" "" "$INSTDIR\ReadMD.exe" 0
  
  ; File Associations for .md and .markdown
  WriteRegStr HKCU "Software\Classes\.md" "" "ReadMD.Markdown"
  WriteRegStr HKCU "Software\Classes\.markdown" "" "ReadMD.Markdown"
  WriteRegStr HKCU "Software\Classes\ReadMD.Markdown" "" "Markdown Document"
  WriteRegStr HKCU "Software\Classes\ReadMD.Markdown\DefaultIcon" "" "$INSTDIR\ReadMD.exe,0"
  WriteRegStr HKCU "Software\Classes\ReadMD.Markdown\shell\open\command" "" '"$INSTDIR\ReadMD.exe" "%1"'
  
  ; Uninstaller & Control Panel Add/Remove entry
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "DisplayName" "ReadMD - Markdown Editor & Reader"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "DisplayIcon" "$INSTDIR\ReadMD.exe,0"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "DisplayVersion" "0.0.1"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "Publisher" "Natsummerance"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "URLInfoAbout" "https://rust.readmd.asia"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$DESKTOP\ReadMD.lnk"
  Delete "$SMPROGRAMS\ReadMD\ReadMD.lnk"
  Delete "$SMPROGRAMS\ReadMD\Uninstall ReadMD.lnk"
  RMDir "$SMPROGRAMS\ReadMD"
  
  RMDir /r "$INSTDIR\assets"
  Delete "$INSTDIR\ReadMD.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\README.md"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  
  DeleteRegKey HKCU "Software\Classes\ReadMD.Markdown"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ReadMD"
  DeleteRegKey HKCU "Software\ReadMD"
SectionEnd
