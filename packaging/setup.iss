; Inno Setup 6 — real per-user install (no admin).
; Installs to %LOCALAPPDATA%\Programs\Grok Desktop\
; Always creates Start Menu + Desktop shortcuts + Add/Remove Programs entry.
; Built on CI: iscc packaging\setup.iss /DMyAppVersion=0.1.5

#ifndef MyAppVersion
  #define MyAppVersion "0.1.5"
#endif

#define MyAppName "Grok Desktop"
#define MyAppPublisher "QingChen Cloud"
#define MyAppURL "https://github.com/qingchencloud/grok-app"
#define MyAppExeName "GrokDesktop.exe"

[Setup]
AppId={{A8C3E2F1-9B4D-4E7A-8C2F-1D0E6B5A4932}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}/releases
DefaultDirName={localappdata}\Programs\Grok Desktop
DefaultGroupName={#MyAppName}
; Show install folder so users see it is a real install (not portable).
DisableDirPage=no
DisableProgramGroupPage=yes
; Per-user install — no UAC / admin
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
OutputDir=..\dist
OutputBaseFilename=GrokDesktop-Setup-{#MyAppVersion}-windows-x64
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
AppCopyright=Copyright (C) {#MyAppPublisher}
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany={#MyAppPublisher}
VersionInfoDescription={#MyAppName} Setup
VersionInfoProductName={#MyAppName}
; The app normally turns WM_CLOSE into "hide to tray". Force is the final
; Restart Manager fallback if the explicit process-tree shutdown below fails.
CloseApplications=force
RestartApplications=no
LicenseFile=..\LICENSE
; Allow reinstall / upgrade over the same AppId
UsePreviousAppDir=yes
UsePreviousGroup=yes
; Create uninstall registry under HKCU (per-user)
CreateUninstallRegKey=yes
; Always show finished page with Launch checkbox
DisableFinishedPage=no

[Languages]
; English only in the wizard (app itself is EN/ZH). Avoids missing ISL on CI.
Name: "english"; MessagesFile: "compiler:Default.isl"

; Desktop shortcut is always created (not an optional unchecked task).
; Users can still delete the .lnk; Start Menu entry remains.

[Files]
; Stage folder filled by CI / build-setup.ps1 before iscc
Source: "stage\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "stage\LICENSE.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "stage\README.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "stage\Uninstall.ps1"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "stage\VERSION.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist

[Icons]
; Start Menu
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Comment: "{#MyAppName}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
; Desktop — always (real install, not portable)
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; Comment: "{#MyAppName}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[UninstallDelete]
; Clean leftover files in install dir if any
Type: filesandordirs; Name: "{app}"

[Code]
procedure StopRunningGrokDesktop;
var
  ResultCode: Integer;
begin
  { Only do this for an upgrade/reinstall of an existing per-user install. }
  if FileExists(ExpandConstant('{app}\{#MyAppExeName}')) then
  begin
    { /T also stops the Grok CLI agent child; /F bypasses close-to-tray. }
    Exec(
      ExpandConstant('{sys}\taskkill.exe'),
      '/F /T /IM "{#MyAppExeName}"',
      '',
      SW_HIDE,
      ewWaitUntilTerminated,
      ResultCode
    );
    Sleep(500);
  end;
end;

function InitializeSetup(): Boolean;
begin
  Result := True;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  { Runs before Inno Setup checks locked [Files] resources. }
  StopRunningGrokDesktop;
  Result := '';
end;
