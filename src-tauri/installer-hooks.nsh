; Хуки установщика kl!ck (NSIS). Подключаются Tauri через
; bundle.windows.nsis.installerHooks. У kl!ck есть хвосты вне папки
; установки: дочерний mihomo, задача Планировщика, правила Kill Switch в
; брандмауэре и системный прокси. Регистры $R0–$R7 свободны: шаблон Tauri
; пользуется $0–$9.

!macro NSIS_HOOK_PREINSTALL
  ; Tauri закрывает klick.exe, но mihomo — его дочерний процесс в
  ; $INSTDIR\bin; пока он работает, файл занят и не перезаписывается.
  nsExec::ExecToLog 'taskkill /F /IM mihomo.exe'
  Pop $R0
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; klick.exe всегда поднимается до администратора. Лежи он в папке, куда
  ; может писать обычный пользователь, его можно подменить. Program Files
  ; защищён, но папку можно выбрать другую — тогда ставим права как у
  ; Program Files. Трогаем только папку с именем продукта. (Как в Klutz.)
  StrCpy $R0 $INSTDIR "" -6
  StrCmp $R0 "\kl!ck" 0 klick_acl_done
  StrLen $R1 $PROGRAMFILES64
  StrCpy $R2 $INSTDIR $R1
  StrCmp $R2 $PROGRAMFILES64 klick_acl_done
  StrLen $R1 $PROGRAMFILES
  StrCpy $R2 $INSTDIR $R1
  StrCmp $R2 $PROGRAMFILES klick_acl_done
    ; SID вместо имён: на русской Windows группы называются иначе.
    nsExec::ExecToLog 'icacls "$INSTDIR" /setowner "*S-1-5-32-544" /T /C /Q'
    Pop $R0
    nsExec::ExecToLog 'icacls "$INSTDIR" /inheritance:r /grant:r "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-545:(OI)(CI)RX" /T /C /Q'
    Pop $R0
  klick_acl_done:
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; /UPDATE — установка поверх: автозапуск и настройки переживают обновление.
  ${If} $UpdateMode <> 1

  ; Правила Kill Switch и системный прокси снимает сам kl!ck — иначе после
  ; удаления защищённые программы остались бы без интернета.
  nsExec::ExecToLog '"$INSTDIR\${MAINBINARYNAME}.exe" --cleanup'
  Pop $R0

  nsExec::ExecToLog 'schtasks /Delete /TN "klick-Autostart" /F'
  Pop $R0

  ${EndIf}

  nsExec::ExecToLog 'taskkill /F /IM mihomo.exe'
  Pop $R0
!macroend
