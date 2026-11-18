; The Windows installer of Jellyfesh, Jellyfesh-<version>-Windows-Setup.exe (docs/windows.md):
;
;   * one Setup for x64 and Arm64 PCs, which installs the build made for the PC's own processor;
;   * for the current user, without administrator rights, in %LOCALAPPDATA%\Programs\Jellyfesh;
;   * a Start menu shortcut, and an uninstaller listed in Settings > Apps;
;   * over any earlier version: the folder's old files are removed first; the settings, library
;     and caches live elsewhere (%APPDATA%\Jellyfesh, %LOCALAPPDATA%\Jellyfesh) and are kept,
;     also when Jellyfesh is uninstalled.
;
; scripts/installer.ps1 compiles it with Inno Setup 6.3 or later from the folders that
; scripts/build.ps1 publishes:
;
;   ISCC /DJfVersion=0.0.7 /DJfFileVersion=0.0.7.0 /DJfX64=<publish\win-x64\Jellyfesh>
;        /DJfArm64=<publish\win-arm64\Jellyfesh> /O<folder> /F<file name> Jellyfesh.iss
;
; with /DJfSign /Sjellyfesh=<signtool command ending in $f> to sign Setup and the uninstaller.

#if VER < EncodeVer(6, 3, 0)
  #error Inno Setup 6.3 or later is needed (the x64compatible and arm64 architectures)
#endif
#ifndef JfVersion
  #error Pass /DJfVersion, /DJfFileVersion, /DJfX64 and /DJfArm64 (scripts/installer.ps1 does)
#endif

[Setup]
; Windows and every later installer recognise Jellyfesh by this id: never change it.
AppId={{9AC75CB1-82E8-4F40-B9EB-72E1AB627A4E}
AppName=Jellyfesh
AppVersion={#JfVersion}
AppPublisher=The Jellyfesh authors
#ifdef JfWebsite
AppPublisherURL={#JfWebsite}
AppSupportURL={#JfWebsite}
#endif
VersionInfoVersion={#JfFileVersion}
VersionInfoProductName=Jellyfesh
VersionInfoDescription=Jellyfesh Setup
; Per user, never asking for administrator rights.
PrivilegesRequired=lowest
DefaultDirName={autopf}\Jellyfesh
DisableDirPage=yes
DisableProgramGroupPage=yes
; 64-bit Windows 10 version 1809 or later, on x64 or Arm64 processors.
ArchitecturesAllowed=x64compatible or arm64
ArchitecturesInstallIn64BitMode=x64compatible or arm64
MinVersion=10.0.17763
WizardStyle=modern
ShowLanguageDialog=auto
SetupIconFile=..\src\Jellyfesh.App\Assets\Icons\ice.ico
UninstallDisplayIcon={app}\Jellyfesh.exe
UninstallDisplayName=Jellyfesh
; A running Jellyfesh is closed (after asking) before its files are replaced.
CloseApplications=yes
RestartApplications=no
OutputDir=..\artifacts
OutputBaseFilename=Jellyfesh-{#JfVersion}-Windows-Setup
Compression=lzma2/max
SolidCompression=yes
#ifdef JfSign
SignTool=jellyfesh
SignedUninstaller=yes
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
; Chinese has been an official translation since Inno Setup 7.0.
#if FileExists(AddBackslash(CompilerPath) + "Languages\ChineseSimplified.isl")
Name: "chinesesimplified"; MessagesFile: "compiler:Languages\ChineseSimplified.isl"
#endif
#if FileExists(AddBackslash(CompilerPath) + "Languages\ChineseTraditional.isl")
Name: "chinesetraditional"; MessagesFile: "compiler:Languages\ChineseTraditional.isl"
#endif
Name: "japanese"; MessagesFile: "compiler:Languages\Japanese.isl"

[InstallDelete]
; The folder holds only what this installer put there (its location cannot be changed), so an
; upgrade empties it first: nothing an older version had and this one has not stays behind.
Type: filesandordirs; Name: "{app}\*"

[Files]
Source: "{#JfX64}\*"; DestDir: "{app}"; Check: not IsArm64; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#JfArm64}\*"; DestDir: "{app}"; Check: IsArm64; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Jellyfesh"; Filename: "{app}\Jellyfesh.exe"; WorkingDir: "{app}"

[Run]
Filename: "{app}\Jellyfesh.exe"; Description: "{cm:LaunchProgram,Jellyfesh}"; Flags: nowait postinstall skipifsilent
