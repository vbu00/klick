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
    ; Права задаём только самой папке, а всё внутри просто наследует их.
    ; Раньше здесь стоял /T: на каждом файле снималось наследование, а
    ; флаги (OI)(CI) к файлу не применяются — файлы оставались вовсе без
    ; разрешений, и ни klick.exe, ни uninstall.exe не запускались.
    nsExec::ExecToLog 'icacls "$INSTDIR" /inheritance:r /grant:r "*S-1-5-32-544:(OI)(CI)F" "*S-1-5-18:(OI)(CI)F" "*S-1-5-32-545:(OI)(CI)RX" /C /Q'
    Pop $R0
    nsExec::ExecToLog 'icacls "$INSTDIR\*" /reset /T /C /Q'
    Pop $R0
  klick_acl_done:

  ; Иконка поменялась (0.2 — логотип-клавиша), а путь к exe прежний:
  ; Windows показывает на панели задач и в «Пуске» картинку из своего кэша.
  ; Сообщаем оболочке, что значки устарели, и перестраиваем кэш значков.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)'
  nsExec::ExecToLog '"$SYSDIR\ie4uinit.exe" -show'
  Pop $R0
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
