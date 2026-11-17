; ShareXrs's Windows installer, ported from ShareX.Setup/InnoSetup/ShareX-setup.iss
; (REFERENCE_COMMIT). Same install folder rules, shortcuts, tasks, update switches
; (/UPDATE, /NORUN) and uninstall, under ShareXrs's own names, AppId and registry
; keys, so it never touches an installed ShareX (docs/RELEASING.md).
;
; Built by scripts/package-windows.ps1:
;   ISCC /DPlatform=x64|arm64 /DMyAppVersion=<ver> /DMyAppVersionNumeric=<x.y.z>
;        /DSourceDir=<staged folder> /DOutputDir=<out> /DRootDir=<repo> ShareXrs-setup.iss

#define MyAppName "ShareXrs"
#ifndef Platform
  #define Platform "x64"
#endif
#if Platform == "arm64"
  #define MyArchitecturesAllowed "arm64"
#else
  #define MyArchitecturesAllowed "x64compatible"
#endif
#ifndef RootDir
  #define RootDir "..\.."
#endif
#ifndef SourceDir
  #error SourceDir (the staged ShareXrs folder) must be defined
#endif
#ifndef OutputDir
  #define OutputDir RootDir + "\dist"
#endif
#ifndef MyAppVersion
  #error MyAppVersion must be defined
#endif
#ifndef MyAppVersionNumeric
  #define MyAppVersionNumeric MyAppVersion
#endif
#define MyAppFileName "sharexrs.exe"
#define MyAppPublisher "ShareXrs"
#define MyAppURL "https://github.com/CubityFirst/sharexrs"
; ShareXrs's own: ShareX's is 82E6AC09-0FEF-4390-AD9F-0DD3F5561EFC. Never change it,
; or updates install beside the old copy instead of over it.
#define MyAppId "F7492E77-AA16-4212-9FD6-53D97873DD92"

[Setup]
AppCopyright=ShareXrs, a port of ShareX (Copyright (c) 2007-2026 ShareX Team)
AppId={#MyAppId}
; ShareX also sets AppMutex to its AppId, a mutex ShareX.exe holds. ShareXrs holds
; none yet, so Setup closes a running copy through the Restart Manager instead
; (CloseApplications, on by default) and the uninstaller looks for the process.
AppName={#MyAppName}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppVerName={#MyAppName} {#MyAppVersion}
AppVersion={#MyAppVersion}
ArchitecturesAllowed={#MyArchitecturesAllowed}
ArchitecturesInstallIn64BitMode={#MyArchitecturesAllowed}
DefaultDirName={commonpf}\{#MyAppName}
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
LicenseFile={#RootDir}\LICENSE
MinVersion=10.0.14393
OutputBaseFilename={#MyAppName}-{#MyAppVersion}-setup-{#Platform}
OutputDir={#OutputDir}
PrivilegesRequired=none
SolidCompression=yes
UninstallDisplayIcon={app}\{#MyAppFileName}
UninstallDisplayName={#MyAppName}
VersionInfoCompany={#MyAppPublisher}
VersionInfoProductName={#MyAppName}
VersionInfoTextVersion={#MyAppVersion}
VersionInfoVersion={#MyAppVersionNumeric}

[Tasks]
Name: "CreateDesktopIcon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"; Check: not IsUpdating and not DesktopIconExists
; Checked in ShareX. Unchecked here until ShareXrs acts on a file passed on its command
; line (ShareX's ApplicationCommandLine isn't ported): until then they'd only start
; ShareXrs again (docs/RELEASING.md).
Name: "CreateContextMenuButton"; Description: "Show ""Upload with ShareXrs"" button in Windows Explorer context menu"; GroupDescription: "Additional shortcuts:"; Flags: unchecked; Check: not IsUpdating
Name: "CreateSendToIcon"; Description: "Create a send to shortcut"; GroupDescription: "Additional shortcuts:"; Flags: unchecked; Check: not IsUpdating
Name: "CreateStartupIcon"; Description: "Run ShareXrs when Windows starts"; GroupDescription: "Other tasks:"; Check: not IsUpdating
; ShareX's "Enable browser extension support" registers its native messaging host
; (ShareX.NativeMessagingHost), which ShareXrs doesn't have: left out.
; Only offered when Print Screen still opens the Snipping Tool, so the value is only
; ever removed on uninstall by the installer that set it (ShareX's may have).
Name: "DisablePrintScreenKeyForSnippingTool"; Description: "Disable Print Screen key for Snipping Tool"; GroupDescription: "Other tasks:"; Check: not IsUpdating and PrintScreenOpensSnippingTool

; ShareX's [InstallDelete] removes language DLLs ShareX 18 put beside ShareX.exe:
; ShareXrs never did (its translations are in sharexrs.exe).

[Files]
Source: "{#SourceDir}\{#MyAppFileName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\ffmpeg.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\onnxruntime.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\Models\*"; DestDir: "{app}\Models"; Flags: ignoreversion recursesubdirs
Source: "{#SourceDir}\Licenses\*.txt"; DestDir: "{app}\Licenses"; Flags: ignoreversion

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppFileName}"; WorkingDir: "{app}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"; WorkingDir: "{app}"
Name: "{userdesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppFileName}"; WorkingDir: "{app}"; Tasks: CreateDesktopIcon
Name: "{usersendto}\{#MyAppName}"; Filename: "{app}\{#MyAppFileName}"; WorkingDir: "{app}"; Tasks: CreateSendToIcon
; The same ShareXrs.lnk Application settings > Run ShareXrs when Windows starts makes
; (crates/sharexrs/src/startup.rs).
Name: "{userstartup}\{#MyAppName}"; Filename: "{app}\{#MyAppFileName}"; WorkingDir: "{app}"; Parameters: "-silent"; Tasks: CreateStartupIcon

[Run]
Filename: "{app}\{#MyAppFileName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; Flags: nowait postinstall; Check: not IsNoRun

[Registry]
; ShareX's IntegrationHelpers keys, under ShareXrs's names.
Root: "HKCU"; Subkey: "Software\Classes\*\shell\{#MyAppName}"; ValueType: string; ValueData: "Upload with {#MyAppName}"; Tasks: CreateContextMenuButton
Root: "HKCU"; Subkey: "Software\Classes\*\shell\{#MyAppName}"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppFileName}"",0"; Tasks: CreateContextMenuButton
Root: "HKCU"; Subkey: "Software\Classes\*\shell\{#MyAppName}\command"; ValueType: string; ValueData: """{app}\{#MyAppFileName}"" ""%1"""; Tasks: CreateContextMenuButton
Root: "HKCU"; Subkey: "Software\Classes\Directory\shell\{#MyAppName}"; ValueType: string; ValueData: "Upload with {#MyAppName}"; Tasks: CreateContextMenuButton
Root: "HKCU"; Subkey: "Software\Classes\Directory\shell\{#MyAppName}"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\{#MyAppFileName}"",0"; Tasks: CreateContextMenuButton
Root: "HKCU"; Subkey: "Software\Classes\Directory\shell\{#MyAppName}\command"; ValueType: string; ValueData: """{app}\{#MyAppFileName}"" ""%1"""; Tasks: CreateContextMenuButton
; Removed on uninstall, whoever made them (ShareX: the app registers .sxcu/.sxie and
; the image editor's menu at run time, the uninstaller removes them). ShareXrs's
; ProgIDs are ShareXrs.sxcu/ShareXrs.sxie; the shared .sxcu/.sxie keys are cleaned
; in CurUninstallStepChanged, only where they point at ShareXrs.
Root: "HKCU"; Subkey: "Software\Classes\*\shell\{#MyAppName}"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Software\Classes\Directory\shell\{#MyAppName}"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Software\Classes\{#MyAppName}.sxcu"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Software\Classes\{#MyAppName}.sxie"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Software\Classes\SystemFileAssociations\image\shell\{#MyAppName}ImageEditor"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Software\Classes\SystemFileAssociations\image\shell\{#MyAppName}PhotoEditor"; Flags: dontcreatekey uninsdeletekey
Root: "HKCU"; Subkey: "Control Panel\Keyboard"; ValueType: dword; ValueName: "PrintScreenKeyForSnippingEnabled"; ValueData: "0"; Flags: uninsdeletevalue; Tasks: DisablePrintScreenKeyForSnippingTool

[Code]
// Without admin rights, the per-user Programs folder. ShareX does this even over a
// /DIR= on the command line; ShareXrs keeps the folder asked for.
procedure InitializeWizard;
begin
  if not IsAdmin and (ExpandConstant('{param:DIR|}') = '') then
  begin
    WizardForm.DirEdit.Text := ExpandConstant('{userpf}\{#MyAppName}');
  end;
end;

function IsAppRunning(): Boolean;
var
  ResultCode: Integer;
begin
  Result := Exec(ExpandConstant('{cmd}'), '/C tasklist /FI "IMAGENAME eq {#MyAppFileName}" /NH | find /I "{#MyAppFileName}" > nul',
    '', SW_HIDE, ewWaitUntilTerminated, ResultCode) and (ResultCode = 0);
end;

function InitializeUninstall(): Boolean;
var
  ErrorCode: Integer;
begin
  if IsAppRunning() then
  begin
    if SuppressibleMsgBox('Uninstall has detected that {#MyAppName} is currently running.' + #13#10#13#10 + 'Would you like to close it?', mbError, MB_YESNO, IDYES) = IDYES then
    begin
      Exec('taskkill.exe', '/f /im {#MyAppFileName}', '', SW_HIDE, ewWaitUntilTerminated, ErrorCode);
    end
    else
    begin
      Result := False;
      Exit;
    end;
  end;

  Result := True;
end;

// .sxcu and .sxie are ShareX's extensions too: take ShareXrs out of them without
// touching what ShareX (or the user) set.
procedure UnregisterExtension(const Extension, ProgId: string);
var
  Key, Current: string;
begin
  Key := 'Software\Classes\' + Extension;
  RegDeleteValue(HKEY_CURRENT_USER, Key + '\OpenWithProgids', ProgId);
  if RegQueryStringValue(HKEY_CURRENT_USER, Key, '', Current) and (CompareText(Current, ProgId) = 0) then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, Key, '');
  end;
  RegDeleteKeyIfEmpty(HKEY_CURRENT_USER, Key + '\OpenWithProgids');
  RegDeleteKeyIfEmpty(HKEY_CURRENT_USER, Key);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    UnregisterExtension('.sxcu', '{#MyAppName}.sxcu');
    UnregisterExtension('.sxie', '{#MyAppName}.sxie');
  end;
end;

function CmdLineParamExists(const value: string): Boolean;
var
  i: Integer;
begin
  Result := False;
  for i := 1 to ParamCount do
    if CompareText(ParamStr(i), value) = 0 then
    begin
      Result := True;
      Exit;
    end;
end;

function IsUpdating(): Boolean;
begin
  Result := CmdLineParamExists('/UPDATE');
end;

function IsNoRun(): Boolean;
begin
  Result := CmdLineParamExists('/NORUN');
end;

function DesktopIconExists(): Boolean;
begin
  Result := FileExists(ExpandConstant('{userdesktop}\{#MyAppName}.lnk'));
end;

function PrintScreenOpensSnippingTool(): Boolean;
var
  Value: Cardinal;
begin
  Result := not (RegQueryDWordValue(HKEY_CURRENT_USER, 'Control Panel\Keyboard', 'PrintScreenKeyForSnippingEnabled', Value) and (Value = 0));
end;
