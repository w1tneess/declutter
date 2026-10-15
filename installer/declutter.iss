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

[Files]
Source: "..\dist\Declutter\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion

[Icons]
Name: "{autoprograms}\Declutter"; Filename: "{app}\Declutter.exe"
Name: "{autodesktop}\Declutter"; Filename: "{app}\Declutter.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\Declutter.exe"; Description: "Launch Declutter"; Flags: nowait postinstall skipifsilent
