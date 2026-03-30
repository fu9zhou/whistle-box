; Always show language selector even if a language was stored in registry
!define MUI_LANGDLL_ALWAYSSHOW

!macro NSIS_HOOK_PREINSTALL
  ; Note: Processes must be killed BEFORE file operations to prevent locked-file errors.
  ; The config/data retention dialogs below are independent of process state.
  ; ── Step 1: Kill WhistleBox main app and its process tree ──
  nsExec::ExecToStack 'taskkill /F /T /IM "whistle-box.exe"'
  Pop $1
  Pop $1

  ; ── Step 3: Kill node sidecar variants (build-time names) ──
  nsExec::ExecToStack 'taskkill /F /IM "node-x86_64-pc-windows-msvc.exe"'
  Pop $1
  Pop $1
  nsExec::ExecToStack 'taskkill /F /IM "node-aarch64-pc-windows-msvc.exe"'
  Pop $1
  Pop $1

  ; ── Step 4: Kill node.exe in WhistleBox path via PowerShell ──
  nsExec::ExecToStack `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Process -Name node -ErrorAction SilentlyContinue | Where-Object { $$_.Path -and $$_.Path -like '*WhistleBox*' } | Stop-Process -Force -ErrorAction SilentlyContinue"`
  Pop $1
  Pop $1

  ; ── Step 5: Kill via CIM/WMI (more reliable for some Windows versions) ──
  nsExec::ExecToStack `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-CimInstance Win32_Process -Filter 'name=''node.exe''' -ErrorAction SilentlyContinue | Where-Object { $$_.ExecutablePath -and $$_.ExecutablePath -like '*WhistleBox*' } | ForEach-Object { Stop-Process -Id $$_.ProcessId -Force -ErrorAction SilentlyContinue }"`
  Pop $1
  Pop $1

  ; ── Step 6: Fallback via wmic for older Windows ──
  nsExec::ExecToStack `wmic process where "name='node.exe' and ExecutablePath like '%WhistleBox%'" call terminate`
  Pop $1
  Pop $1

  Sleep 3000

  ; ── Step 7: Verify node sidecar is not locked via rename test ──
  IfFileExists "$INSTDIR\node-x86_64-pc-windows-msvc.exe" 0 _pre_node_ok
  ClearErrors
  Rename "$INSTDIR\node-x86_64-pc-windows-msvc.exe" "$INSTDIR\node-x86_64-pc-windows-msvc.exe.bak"
  IfErrors _pre_node_force_kill
  Rename "$INSTDIR\node-x86_64-pc-windows-msvc.exe.bak" "$INSTDIR\node-x86_64-pc-windows-msvc.exe"
  Goto _pre_node_ok

  _pre_node_force_kill:
  ; Retry path-scoped kill for WhistleBox node processes only
  nsExec::ExecToStack `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Process -Name node -ErrorAction SilentlyContinue | Where-Object { $$_.Path -and $$_.Path -like '*WhistleBox*' } | Stop-Process -Force -ErrorAction SilentlyContinue"`
  Pop $1
  Pop $1
  Sleep 3000
  Delete "$INSTDIR\node-x86_64-pc-windows-msvc.exe.bak"

  _pre_node_ok:

  ; Ask separately: 1) app config  2) whistle rules/data
  IfFileExists "$APPDATA\WhistleBox\config.json" _pre_ask_config _pre_check_rules

  _pre_ask_config:
  StrCmp $LANGUAGE 2052 0 _pre_cfg_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "检测到 WhistleBox 用户配置：$\r$\n$APPDATA\WhistleBox$\r$\n$\r$\n是否保留？" \
      /SD IDYES IDYES _pre_check_rules
    Goto _pre_del_config
  _pre_cfg_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Existing WhistleBox config found:$\r$\n$APPDATA\WhistleBox$\r$\n$\r$\nKeep it?" \
      /SD IDYES IDYES _pre_check_rules
  _pre_del_config:
  RMDir /r "$APPDATA\WhistleBox"

  _pre_check_rules:
  ; Check both possible Whistle data directories
  IfFileExists "$PROFILE\.WhistleBoxData\*.*" _pre_ask_rules 0
  IfFileExists "$PROFILE\.WhistleBoxData\" _pre_ask_rules 0
  IfFileExists "$PROFILE\.whistle\*.*" _pre_ask_rules_default _pre_end

  _pre_ask_rules:
  StrCmp $LANGUAGE 2052 0 _pre_rules_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "检测到 Whistle 规则：$\r$\n$PROFILE\.WhistleBoxData$\r$\n$\r$\n是否保留？" \
      /SD IDYES IDYES _pre_check_default_whistle
    Goto _pre_del_rules
  _pre_rules_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Whistle rules found:$\r$\n$PROFILE\.WhistleBoxData$\r$\n$\r$\nKeep them?" \
      /SD IDYES IDYES _pre_check_default_whistle
  _pre_del_rules:
  RMDir /r "$PROFILE\.WhistleBoxData"

  _pre_check_default_whistle:
  IfFileExists "$PROFILE\.whistle\*.*" _pre_ask_rules_default _pre_end

  _pre_ask_rules_default:
  StrCmp $LANGUAGE 2052 0 _pre_rules_default_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "检测到 Whistle 规则：$\r$\n$PROFILE\.whistle$\r$\n$\r$\n是否保留？" \
      /SD IDYES IDYES _pre_end
    Goto _pre_del_rules_default
  _pre_rules_default_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Whistle rules found:$\r$\n$PROFILE\.whistle$\r$\n$\r$\nKeep them?" \
      /SD IDYES IDYES _pre_end
  _pre_del_rules_default:
  RMDir /r "$PROFILE\.whistle"

  _pre_end:
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; ── Kill processes ──
  nsExec::ExecToStack 'taskkill /F /T /IM "whistle-box.exe"'
  Pop $1
  Pop $1
  nsExec::ExecToStack 'taskkill /F /IM "node-x86_64-pc-windows-msvc.exe"'
  Pop $1
  Pop $1
  nsExec::ExecToStack 'taskkill /F /IM "node-aarch64-pc-windows-msvc.exe"'
  Pop $1
  Pop $1
  nsExec::ExecToStack `powershell -NoProfile -ExecutionPolicy Bypass -Command "Get-Process -Name node -ErrorAction SilentlyContinue | Where-Object { $$_.Path -and $$_.Path -like '*WhistleBox*' } | Stop-Process -Force -ErrorAction SilentlyContinue"`
  Pop $1
  Pop $1
  nsExec::ExecToStack `wmic process where "name='node.exe' and ExecutablePath like '%WhistleBox%'" call terminate`
  Pop $1
  Pop $1

  Sleep 1000

  ; ── Data cleanup: ask separately for config and rules ──
  IfFileExists "$APPDATA\WhistleBox\config.json" _post_ask_config _post_check_rules

  _post_ask_config:
  StrCmp $LANGUAGE 2052 0 _post_cfg_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "是否保留 WhistleBox 用户配置？$\r$\n$APPDATA\WhistleBox" \
      /SD IDYES IDYES _post_check_rules
    Goto _post_del_config
  _post_cfg_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Keep WhistleBox config?$\r$\n$APPDATA\WhistleBox" \
      /SD IDYES IDYES _post_check_rules
  _post_del_config:
  RMDir /r "$APPDATA\WhistleBox"

  _post_check_rules:
  IfFileExists "$PROFILE\.WhistleBoxData\*.*" _post_ask_rules 0
  IfFileExists "$PROFILE\.WhistleBoxData\" _post_ask_rules 0
  IfFileExists "$PROFILE\.whistle\*.*" _post_ask_rules_default _post_end

  _post_ask_rules:
  StrCmp $LANGUAGE 2052 0 _post_rules_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "检测到 Whistle 规则：$\r$\n$PROFILE\.WhistleBoxData$\r$\n$\r$\n是否保留？" \
      /SD IDYES IDYES _post_check_default_whistle
    Goto _post_del_rules
  _post_rules_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Whistle rules found:$\r$\n$PROFILE\.WhistleBoxData$\r$\n$\r$\nKeep them?" \
      /SD IDYES IDYES _post_check_default_whistle
  _post_del_rules:
  RMDir /r "$PROFILE\.WhistleBoxData"

  _post_check_default_whistle:
  IfFileExists "$PROFILE\.whistle\*.*" _post_ask_rules_default _post_end

  _post_ask_rules_default:
  StrCmp $LANGUAGE 2052 0 _post_rules_default_en
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "检测到 Whistle 规则：$\r$\n$PROFILE\.whistle$\r$\n$\r$\n是否保留？" \
      /SD IDYES IDYES _post_end
    Goto _post_del_rules_default
  _post_rules_default_en:
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "Whistle rules found:$\r$\n$PROFILE\.whistle$\r$\n$\r$\nKeep them?" \
      /SD IDYES IDYES _post_end
  _post_del_rules_default:
  RMDir /r "$PROFILE\.whistle"

  _post_end:
  SetErrorLevel 0
!macroend
