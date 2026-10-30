; Uninstalling never deletes the data of the app: the database and the kept invoices and offers
; must be kept for years. Tauri's uninstaller offers to delete `%APPDATA%\<identifier>` with a
; checkbox on its confirm page; whatever was ticked there, it is cleared before anything is removed.
!macro NSIS_HOOK_PREUNINSTALL
  StrCpy $DeleteAppDataCheckboxState 0
!macroend
