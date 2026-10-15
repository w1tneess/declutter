; Build with: iscc /DAppVersion=1.0.0 installer\declutter.iss   (after PyInstaller has produced dist\Declutter)
#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif

[Setup]
AppId={{16B67213-637B-4025-A904-6E1686D7C140}
AppName=Declutter
AppVersion={#AppVersion}
AppPublisher=w1tneess
DefaultDirName={autopf}\Declutter
DisableProgramGroupPage=yes
; Per-user install: no admin rights and no UAC prompt.
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=Declutter-Setup
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\Declutter.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop icon"; Flags: unchecked
Name: "contextmenu"; Description: "Add ""Tidy with Declutter"" to the folder right-click menu"; Flags: unchecked

[Registry]
; Per-user (HKCU), so no admin rights; removed again on uninstall.
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Declutter"; ValueType: string; ValueName: ""; ValueData: "Tidy with Declutter"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Declutter"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\Declutter.exe"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Declutter\command"; ValueType: string; ValueName: ""; ValueData: """{app}\Declutter.exe"" ""%1"""; Tasks: contextmenu

[Files]
Source: "..\dist\Declutter\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{autoprograms}\Declutter"; Filename: "{app}\Declutter.exe"
Name: "{autodesktop}\Declutter"; Filename: "{app}\Declutter.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Declutter.exe"; Description: "Launch Declutter"; Flags: nowait postinstall skipifsilent
