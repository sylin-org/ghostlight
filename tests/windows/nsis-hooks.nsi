; SPDX-License-Identifier: Apache-2.0 OR MIT
; Execute the production install hooks only in a dedicated fixture directory.
Unicode true
Name "Ghostlight package hook contract"
OutFile "${GHOSTLIGHT_TEST_EXE}"
InstallDir "${GHOSTLIGHT_TEST_ROOT}"
RequestExecutionLevel user
SilentInstall silent
!include "LogicLib.nsh"
!include "FileFunc.nsh"

; The production hook replaces this generated Tauri macro.
!macro CheckIfAppIsRunning executableName productName
!macroend
!include "${GHOSTLIGHT_HOOKS}"

Section
  Call GhostlightQuiescePackage
  ${GetParameters} $1
  ClearErrors
  ${GetOptions} $1 "/FAIL_AFTER_ACQUIRE" $2
  ${IfNot} ${Errors}
    SetErrorLevel 23
    Abort "Intentional failure after acquiring the fixture deployment lock."
  ${EndIf}
  FileOpen $1 "$INSTDIR\prepared.txt" w
  FileWrite $1 "prepared"
  FileClose $1
  Call GhostlightReleaseDeployLock
SectionEnd

; Compile the production uninstall functions, but never write or run an uninstaller.
Section Uninstall
  Abort "This fixture cannot uninstall a product."
SectionEnd
