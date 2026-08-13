; SPDX-License-Identifier: Apache-2.0 OR MIT
; Tauri NSIS lifecycle hooks for the ownership-checked package seam.

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Registering the Ghostlight browser connector for this user"
  nsExec::ExecToLog '"$INSTDIR\ghostlight.exe" native-host install'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Ghostlight native-host registration needs attention (exit $0)"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Removing Ghostlight-owned browser connector registrations"
  nsExec::ExecToLog '"$INSTDIR\ghostlight.exe" native-host uninstall'
  Pop $0
  ${If} $0 != 0
    DetailPrint "Some Ghostlight browser registrations need manual attention (exit $0)"
  ${EndIf}
!macroend
