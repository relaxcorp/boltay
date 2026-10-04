!macro NSIS_HOOK_POSTINSTALL
  ; Explorer keeps showing the icon of the previous version on the taskbar and in Start
  ; until it hears that icons changed.
  System::Call 'shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)'
  ; A pinned button keeps its picture by the path of the exe, which did not change: ask for
  ; that one image again.
  System::Call 'shell32::Shell_GetCachedImageIndexW(w "$INSTDIR\${MAINBINARYNAME}.exe", i 0, i 0) i .r0'
  ${If} $0 >= 0
    System::Call 'shell32::SHUpdateImageW(w "$INSTDIR\${MAINBINARYNAME}.exe", i 0, i 0, i r0)'
  ${EndIf}
!macroend
