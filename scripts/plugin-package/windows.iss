; SPDX-License-Identifier: GPL-3.0-or-later
; Copyright (C) 2026 Mateus Rocha de Paula and the OxiGuitar contributors
;
; The installer of one plugin repository, for one user and without administrator
; rights: it puts the plugin file in %APPDATA%\OxiGuitar\plugins, the folder
; OxiGuitar searches, and registers an uninstaller, so Settings, Apps lists
; and removes it and winget can install and upgrade it (#165). Built by
; scripts/plugin-package/windows.sh, which passes the values below with /D.
;
; The installer downloads nothing and runs nothing: it copies the plugin
; files (one for most plugins, five for oxiguitar-guitarpro).

#ifndef Name
  #error Name is not defined
#endif
#ifndef Title
  #define Title Name
#endif
#ifndef Version
  #error Version is not defined
#endif
#ifndef Arch
  #define Arch "x64"
#endif
#ifndef OutDir
  #define OutDir "."
#endif
#ifndef Homepage
  #define Homepage "https://github.com/mateusrdp/oxiguitar"
#endif

[Setup]
; One id for the plugin, kept across versions: an upgrade replaces it.
AppId=oxiguitar-plugin-{#Name}
AppName=OxiGuitar plugin: {#Title}
AppVersion={#Version}
AppPublisher=The OxiGuitar contributors
AppPublisherURL={#Homepage}
AppSupportURL={#Homepage}/issues
DefaultDirName={userappdata}\OxiGuitar\plugins
DisableDirPage=yes
DisableProgramGroupPage=yes
DisableReadyPage=no
; Per user: no administrator rights and no prompt, even for the all-users
; choice other installers offer.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=
UsePreviousAppDir=no
; The uninstaller removes only what this installer put there, and sits
; beside the others rather than among the plugins.
Uninstallable=yes
UninstallFilesDir={localappdata}\OxiGuitar\uninstall\{#Name}
CreateUninstallRegKey=yes
UninstallDisplayName=OxiGuitar plugin: {#Title}
OutputDir={#OutDir}
OutputBaseFilename=oxiguitar-{#Name}-{#Version}-windows-{#Arch}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
#if Arch == "arm64"
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

; The [Files] section, a line for each plugin file, is added by windows.sh.
