#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#ifndef MyArch
  #define MyArch "x64"
#endif

#ifndef MyTarget
  #define MyTarget "x86_64-pc-windows-msvc"
#endif

#ifndef MyExeName
  #define MyExeName "termy.exe"
#endif

#if MyArch == "x64"
  #define MyArchAllowed "x64compatible"
  #define MyArchInstallMode "x64compatible"
#elif MyArch == "arm64"
  #define MyArchAllowed "arm64"
  #define MyArchInstallMode "arm64"
#else
  #error Unsupported MyArch value. Use x64 or arm64.
#endif

[Setup]
AppName=Termy
AppVersion={#MyAppVersion}
AppPublisher=Termy
DefaultDirName={autopf}\Termy
DefaultGroupName=Termy
OutputDir=..\target\dist
OutputBaseFilename=Termy-{#MyAppVersion}-{#MyArch}-Setup
SetupIconFile=..\assets\termy.ico
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#MyArchAllowed}
ArchitecturesInstallIn64BitMode={#MyArchInstallMode}
UninstallDisplayIcon={app}\{#MyExeName}

[Files]
Source: "..\target\{#MyTarget}\release\{#MyExeName}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Termy"; Filename: "{app}\{#MyExeName}"
Name: "{autodesktop}\Termy"; Filename: "{app}\{#MyExeName}"

[Run]
Filename: "{app}\{#MyExeName}"; Description: "Launch Termy"; Flags: nowait postinstall skipifsilent
