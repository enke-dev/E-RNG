; Installing for the current user is the default, also for admins (NSIS would preselect all users):
; it needs no admin rights, and v0.8.0 installed per user without leaving the marker Tauri reads to
; keep the mode, so its updates would otherwise move to Program Files.
!define MULTIUSER_INSTALLMODE_DEFAULT_CURRENTUSER

; With the current user as default, NSIS takes an install for all users as one by this HKLM value;
; Tauri only writes a value named after the mode, so updates of such an install would go per user.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $MultiUser.InstallMode == "AllUsers"
    WriteRegStr HKLM "${UNINSTKEY}" "CurrentUser" "0"
  ${EndIf}
!macroend

; Uninstalling never deletes the data of the app: the database and the kept invoices and offers
; must be kept for years. Tauri's uninstaller offers to delete `%APPDATA%\<identifier>` with a
; checkbox on its confirm page; whatever was ticked there, it is cleared before anything is removed.
!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $DeleteAppDataCheckboxState 0
!macroend
