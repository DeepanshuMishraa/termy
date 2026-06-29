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

#ifndef MyCliExeName
  #define MyCliExeName "termy-cli.exe"
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
AppId={{7D3DD34B-5F8F-4D7B-BBC9-0F54B4C89142}
AppName=Termy
AppVersion={#MyAppVersion}
AppPublisher=Termy
AppPublisherURL=https://github.com/lassejlv/termy
AppSupportURL=https://github.com/lassejlv/termy/issues
AppUpdatesURL=https://github.com/lassejlv/termy/releases
DefaultDirName={autopf}\Termy
DefaultGroupName=Termy
OutputDir=..\..\target\dist
OutputBaseFilename=Termy-{#MyAppVersion}-windows-{#MyArch}-Setup
SetupIconFile=..\..\assets\termy.ico
Compression=lzma
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed={#MyArchAllowed}
ArchitecturesInstallIn64BitMode={#MyArchInstallMode}
UninstallDisplayIcon={app}\{#MyExeName}
PrivilegesRequired=admin
CloseApplications=yes
RestartApplications=no

[Files]
Source: "..\..\target\{#MyTarget}\release\{#MyExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\{#MyTarget}\release\{#MyCliExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\target\windows-runtime\MicrosoftEdgeWebView2Setup.exe"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{group}\Termy"; Filename: "{app}\{#MyExeName}"
Name: "{autodesktop}\Termy"; Filename: "{app}\{#MyExeName}"

[Registry]
Root: HKCR; Subkey: "termy"; ValueType: string; ValueName: ""; ValueData: "URL:Termy Protocol"; Flags: uninsdeletekey
Root: HKCR; Subkey: "termy"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""
Root: HKCR; Subkey: "termy\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyExeName},0"
Root: HKCR; Subkey: "termy\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyExeName}"" ""%1"""

[Run]
Filename: "{tmp}\MicrosoftEdgeWebView2Setup.exe"; Parameters: "/silent /install"; StatusMsg: "Installing Microsoft Edge WebView2 Runtime..."; Check: NeedsWebView2Runtime; Flags: waituntilterminated runhidden
Filename: "{app}\{#MyExeName}"; Description: "Launch Termy"; Flags: nowait postinstall skipifsilent
; Silent auto-updates quit Termy before setup finishes, so relaunch after install.
Filename: "{app}\{#MyExeName}"; Flags: nowait runasoriginaluser skipifnotsilent

[Code]
function IsValidWebView2Version(Value: String): Boolean;
begin
  Result := (Value <> '') and (Value <> '0.0.0.0');
end;

function HasWebView2Runtime(): Boolean;
var
  Version: String;
begin
  Result := False;
  if RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and IsValidWebView2Version(Version) then begin
    Result := True;
    exit;
  end;
  if RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and IsValidWebView2Version(Version) then begin
    Result := True;
    exit;
  end;
  if RegQueryStringValue(HKCU, 'Software\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}', 'pv', Version) and IsValidWebView2Version(Version) then begin
    Result := True;
  end;
end;

function NeedsWebView2Runtime(): Boolean;
begin
  Result := not HasWebView2Runtime();
end;
