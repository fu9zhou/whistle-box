!define WHISTLEBOX_HOOK_DIR "${__FILEDIR__}"

!macro WHISTLEBOX_REQUEST_EXIT SCRIPT
  nsExec::ExecToStack 'powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "${SCRIPT}" -Executable "$INSTDIR\whistle-box.exe"'
  Pop $0
  Pop $1
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "请从托盘退出 WhistleBox 后重试。$\r$\n$1"
    Abort
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  InitPluginsDir
  File "/oname=$PLUGINSDIR\whistlebox-maintenance.ps1" "${WHISTLEBOX_HOOK_DIR}\resources\maintenance.ps1"
  !insertmacro WHISTLEBOX_REQUEST_EXIT "$PLUGINSDIR\whistlebox-maintenance.ps1"
  ; Use this release's cleanup code, including when upgrading an older app.
  File "/oname=$PLUGINSDIR\whistlebox-cleanup.exe" "${MAINBINARYSRCPATH}"
  ExecWait '"$PLUGINSDIR\whistlebox-cleanup.exe" --cleanup-only' $0
  ${If} $0 != 0
    Abort "系统代理恢复失败，未替换应用文件。"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro WHISTLEBOX_REQUEST_EXIT "$INSTDIR\resources\maintenance.ps1"
  ExecWait '"$INSTDIR\whistle-box.exe" --cleanup-only' $0
  ${If} $0 != 0
    Abort "系统代理恢复失败，未删除应用文件。"
  ${EndIf}
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "WhistleBox"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; Preserve rules, CA ownership and recovery journals; only preferences are optional.
  IfFileExists "$APPDATA\WhistleBox\config.json" 0 +3
  MessageBox MB_YESNO|MB_ICONQUESTION "保留 WhistleBox 设置？" /SD IDYES IDYES +2
  Delete "$APPDATA\WhistleBox\config.json"
!macroend
