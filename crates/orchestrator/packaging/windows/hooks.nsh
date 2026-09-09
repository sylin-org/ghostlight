; SPDX-License-Identifier: Apache-2.0 OR MIT
; Tauri NSIS lifecycle hooks for the ownership-checked package seam.

!include "Win\RestartManager.nsh"

Var GhostlightOwnsDeployLock
Var GhostlightRestartSession
Var GhostlightPhysicalDirectory
Var GhostlightRegisteredProcesses

; Tauri's default check stops by image name and misses both connectors. Our hooks
; quiesce the exact installed sibling set before either replacement or removal.
!macroundef CheckIfAppIsRunning
!macro CheckIfAppIsRunning executableName productName
!macroend

!macro GhostlightPackageFunctions prefix
Function ${prefix}GhostlightReleaseDeployLock
  ${If} $GhostlightOwnsDeployLock == 1
    Delete "$INSTDIR\deploy.lock"
    StrCpy $GhostlightOwnsDeployLock 0
  ${EndIf}
FunctionEnd

Function ${prefix}GhostlightPhysicalPath
  Exch $1
  Push $2
  Push $3
  Push $4
  StrCpy $0 ""
  ${If} ${FileExists} "$1"
    ; Resolve the consumer's physical path, including packaged-caller AppData
    ; redirection. Restart Manager must not receive a caller-relative alias.
    System::Call 'kernel32::CreateFileW(w r1, i0, i7, p0, i3, i0, p0)p.r2'
    ${If} $2 != -1
      System::Call 'kernel32::GetFinalPathNameByHandleW(p r2, w .r3, i${NSIS_MAX_STRLEN}, i0)i.r4'
      System::Call 'kernel32::CloseHandle(p r2)'
      ${If} $4 > 0
      ${AndIf} $4 < ${NSIS_MAX_STRLEN}
        StrCpy $0 $3
      ${EndIf}
    ${EndIf}
  ${EndIf}
  Pop $4
  Pop $3
  Pop $2
  Pop $1
FunctionEnd

Function ${prefix}GhostlightRegisterInstalledProcesses
  Push $1
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7
  Push $8
  Push $9
  StrCpy $GhostlightRegisteredProcesses 0
  ; NSIS is a 32-bit process: PROCESSENTRY32W is 556 bytes, with PID at offset 8.
  ; Register process identities, not files: a different app merely reading an
  ; installed executable must not become a shutdown target.
  System::Alloc 556
  Pop $1
  System::Call '*$1(i556)'
  System::Call 'kernel32::CreateToolhelp32Snapshot(i2, i0)p.r2 ?e'
  Pop $0
  ${If} $2 == -1
    Goto snapshot_done
  ${EndIf}
  System::Call 'kernel32::Process32FirstW(p r2, p r1)i.r3 ?e'
  Pop $0
  ${DoWhile} $3 != 0
    IntOp $4 $1 + 8
    System::Call '*$4(i.r4)'
    System::Call 'kernel32::OpenProcess(i0x1000, i0, i r4)p.r5'
    ${If} $5 != 0
      StrCpy $6 ${NSIS_MAX_STRLEN}
      System::Call 'kernel32::QueryFullProcessImageNameW(p r5, i0, w .r7, *i r6)i.r8'
      ${If} $8 != 0
        Push $7
        Call ${prefix}GhostlightPhysicalPath
        ${If} $0 == "$GhostlightPhysicalDirectory\ghostlight.exe"
        ${OrIf} $0 == "$GhostlightPhysicalDirectory\ghostlight-mcp-connector.exe"
        ${OrIf} $0 == "$GhostlightPhysicalDirectory\ghostlight-browser-connector.exe"
          ; RM_UNIQUE_PROCESS binds both PID and creation time, preventing PID reuse
          ; from directing shutdown at a replacement process.
          System::Alloc ${SYSSIZEOF_RM_UNIQUE_PROCESS}
          Pop $9
          System::Call '*$9(i r4)'
          IntOp $6 $9 + 4
          System::Call 'kernel32::GetProcessTimes(p r5, p r6, *l, *l, *l)i.r8 ?e'
          Pop $0
          ${If} $8 != 0
            System::Call 'RSTRTMGR::RmRegisterResources(i$GhostlightRestartSession, i0, p0, i1, p r9, i0, p0)i.r0'
          ${EndIf}
          System::Free $9
          ${If} $0 != 0
            System::Call 'kernel32::CloseHandle(p r5)'
            Goto processes_done
          ${EndIf}
          IntOp $GhostlightRegisteredProcesses $GhostlightRegisteredProcesses + 1
        ${EndIf}
      ${EndIf}
      System::Call 'kernel32::CloseHandle(p r5)'
    ${EndIf}
    System::Call 'kernel32::Process32NextW(p r2, p r1)i.r3 ?e'
    Pop $0
  ${Loop}
  ${If} $0 == 18 ; ERROR_NO_MORE_FILES
    StrCpy $0 0
  ${EndIf}
  processes_done:
  System::Call 'kernel32::CloseHandle(p r2)'
  snapshot_done:
  System::Free $1
  Pop $9
  Pop $8
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
  Pop $1
FunctionEnd

Function ${prefix}GhostlightCheckFileAccess
  Exch $1
  Push $2
  StrCpy $0 0
  ${If} ${FileExists} "$1"
    ; An unrelated reader is never killed. If its sharing mode still prevents
    ; this package operation, refuse before copying or unregistering anything.
    !if "${prefix}" == "un."
      System::Call 'kernel32::CreateFileW(w r1, i0x40010000, i7, p0, i3, i0, p0)p.r2 ?e'
    !else
      System::Call 'kernel32::CreateFileW(w r1, i0x40000000, i7, p0, i3, i0, p0)p.r2 ?e'
    !endif
    Pop $0
    ${If} $2 != -1
      System::Call 'kernel32::CloseHandle(p r2)'
      StrCpy $0 0
    ${EndIf}
  ${EndIf}
  Pop $2
  Pop $1
FunctionEnd

Function ${prefix}GhostlightQuiescePackage
  StrCpy $GhostlightOwnsDeployLock 0
  CreateDirectory "$INSTDIR"
  ; CREATE_NEW preserves another deployment's lock instead of overwriting it.
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\deploy.lock", i0x40000000, i1, p0, i1, i0x80, p0)p.r0'
  ${If} $0 == -1
    SetErrorLevel 1
    Abort "Another deployment owns the Ghostlight installation, or its directory is not writable."
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
  StrCpy $GhostlightOwnsDeployLock 1
  Push "$INSTDIR\deploy.lock"
  Call ${prefix}GhostlightPhysicalPath
  ${If} $0 == ""
    Call ${prefix}GhostlightReleaseDeployLock
    SetErrorLevel 1
    Abort "Windows could not resolve the physical Ghostlight installation."
  ${EndIf}
  ${GetParent} "$0" $GhostlightPhysicalDirectory
  !insertmacro RestartManager_StartSession $GhostlightRestartSession
  ${If} $GhostlightRestartSession == ""
    Call ${prefix}GhostlightReleaseDeployLock
    SetErrorLevel 1
    Abort "Windows could not prepare the Ghostlight installation for replacement."
  ${EndIf}
  Call ${prefix}GhostlightRegisterInstalledProcesses
  ${If} $0 == 0
    ${If} $GhostlightRegisteredProcesses > 0
      DetailPrint "Closing processes that use this Ghostlight installation"
      System::Call 'RSTRTMGR::RmShutdown(i$GhostlightRestartSession, i${RmForceShutdown}, p0)i.r0'
    ${EndIf}
  ${EndIf}
  !insertmacro RestartManager_EndSession $GhostlightRestartSession
  ${If} $0 == 0
    Push "$INSTDIR\ghostlight.exe"
    Call ${prefix}GhostlightCheckFileAccess
  ${EndIf}
  ${If} $0 == 0
    Push "$INSTDIR\ghostlight-mcp-connector.exe"
    Call ${prefix}GhostlightCheckFileAccess
  ${EndIf}
  ${If} $0 == 0
    Push "$INSTDIR\ghostlight-browser-connector.exe"
    Call ${prefix}GhostlightCheckFileAccess
  ${EndIf}
  ${If} $0 != 0
    DetailPrint "Ghostlight package preparation failed (Windows error $0)"
    Call ${prefix}GhostlightReleaseDeployLock
    SetErrorLevel 1
    Abort "Close clients using this Ghostlight installation and retry. No executable has been replaced."
  ${EndIf}
FunctionEnd
!macroend

!insertmacro GhostlightPackageFunctions ""
!insertmacro GhostlightPackageFunctions "un."

Function .onInstFailed
  Call GhostlightReleaseDeployLock
FunctionEnd

Function un.onUninstFailed
  Call un.GhostlightReleaseDeployLock
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  Call GhostlightQuiescePackage
!macroend

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering the Ghostlight browser connector for this user"
  nsExec::ExecToLog '"$INSTDIR\ghostlight.exe" native-host install'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Ghostlight native-host registration needs attention (exit $0)"
    Call GhostlightReleaseDeployLock
    SetErrorLevel 1
    Abort "Ghostlight was copied, but browser registration failed. Retry the installer."
  ${EndIf}
  Call GhostlightReleaseDeployLock
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  Call un.GhostlightQuiescePackage
  DetailPrint "Removing Ghostlight-owned browser connector registrations"
  nsExec::ExecToLog '"$INSTDIR\ghostlight.exe" native-host uninstall'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Some Ghostlight browser registrations need manual attention (exit $0)"
    Call un.GhostlightReleaseDeployLock
    SetErrorLevel 1
    Abort "Ghostlight browser registration could not be removed. The installed files were preserved."
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Call un.GhostlightReleaseDeployLock
  ; The template's earlier nonrecursive removal may have encountered our lock.
  RMDir "$INSTDIR"
!macroend
