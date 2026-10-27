; Installer hooks for Tauri's NSIS template (see installer.nsi for the start-at-sign-in checkbox).

!macro NSIS_HOOK_POSTUNINSTALL
  ; Updates uninstall the old version first; keep the startup entry for those.
  ${If} $UpdateMode <> 1
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run" "${PRODUCTNAME}"
    DeleteRegValue HKCU "${MANUPRODUCTKEY}" "AutostartAsked"
    DeleteRegKey /ifempty HKCU "${MANUPRODUCTKEY}"
  ${EndIf}
!macroend
