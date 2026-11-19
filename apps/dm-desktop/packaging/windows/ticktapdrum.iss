; Inno Setup script for the TickTapDrum installer. Run by package.ps1:
;   iscc /DAppVersion=0.2.0 /DSourceExe=<path to ticktapdrum.exe> /DOutputDir=<dir> ticktapdrum.iss
; It installs for the current user only, so it needs no administrator rights.

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif

[Setup]
AppId={{3F28DF71-924A-4A34-A27E-4E5DC3FE2E27}
AppName=TickTapDrum
AppVersion={#AppVersion}
AppPublisher=TickTapDrum
DefaultDirName={localappdata}\Programs\TickTapDrum
DefaultGroupName=TickTapDrum
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename=ticktapdrum-{#AppVersion}-windows-x64-setup
SetupIconFile=..\..\assets\icon.ico
UninstallDisplayIcon={app}\TickTapDrum.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "TickTapDrum.exe"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\TickTapDrum"; Filename: "{app}\TickTapDrum.exe"
Name: "{autodesktop}\TickTapDrum"; Filename: "{app}\TickTapDrum.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\TickTapDrum.exe"; Description: "Start TickTapDrum"; Flags: nowait postinstall skipifsilent
