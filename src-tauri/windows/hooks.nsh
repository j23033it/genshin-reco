; Keep productName/registry keys/install directory stable for existing installations.
; Only Windows display names and shortcuts change. The hooks are included before
; Tauri defines PRODUCTNAME/MAINBINARYNAME, so the GUI callback uses saved values.
!define BUILD_RECOMMENDER_NAME "ビルドレコメンダー"
!define BUILD_RECOMMENDER_LEGACY_NAME "原神 聖遺物レコメンダー"
Var BuildRecommenderInstalled
Var BuildRecommenderExe

; Rename only our own links; preserve AppUserModelId and unrelated user shortcuts.
!macro RenameBuildRecommenderShortcut DIRECTORY
  !insertmacro IsShortcutTarget "${DIRECTORY}\${BUILD_RECOMMENDER_LEGACY_NAME}.lnk" "$BuildRecommenderExe"
  Pop $0
  ${If} $0 = 1
    ${If} ${FileExists} "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk"
      !insertmacro IsShortcutTarget "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk" "$BuildRecommenderExe"
      Pop $0
      ${If} $0 = 1
        Delete "${DIRECTORY}\${BUILD_RECOMMENDER_LEGACY_NAME}.lnk"
      ${EndIf}
    ${Else}
      Rename "${DIRECTORY}\${BUILD_RECOMMENDER_LEGACY_NAME}.lnk" "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk"
    ${EndIf}
  ${EndIf}
!macroend

Function .onGUIEnd
  ; The normal installer's finish page can create the desktop link after POSTINSTALL.
  ; Do nothing if installation was cancelled or did not reach POSTINSTALL.
  ${If} $BuildRecommenderInstalled = 1
    !insertmacro RenameBuildRecommenderShortcut "$DESKTOP"
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${BUILD_RECOMMENDER_NAME}"
  StrCpy $BuildRecommenderExe "$INSTDIR\${MAINBINARYNAME}.exe"
  CreateShortcut "$DESKTOP\${PRODUCTNAME}.lnk" "$BuildRecommenderExe"
  !insertmacro SetLnkAppUserModelId "$DESKTOP\${PRODUCTNAME}.lnk"
  !insertmacro RenameBuildRecommenderShortcut "$DESKTOP"
  !if "${STARTMENUFOLDER}" != ""
    !insertmacro RenameBuildRecommenderShortcut "$SMPROGRAMS\$AppStartMenuFolder"
  !else
    !insertmacro RenameBuildRecommenderShortcut "$SMPROGRAMS"
  !endif
  StrCpy $BuildRecommenderInstalled 1
!macroend

!macro DeleteBuildRecommenderShortcut DIRECTORY
  !insertmacro IsShortcutTarget "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk" "$INSTDIR\${MAINBINARYNAME}.exe"
  Pop $0
  ${If} $0 = 1
    !insertmacro UnpinShortcut "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk"
    Delete "${DIRECTORY}\${BUILD_RECOMMENDER_NAME}.lnk"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  !insertmacro DeleteBuildRecommenderShortcut "$DESKTOP"
  !if "${STARTMENUFOLDER}" != ""
    !insertmacro DeleteBuildRecommenderShortcut "$SMPROGRAMS\$AppStartMenuFolder"
  !else
    !insertmacro DeleteBuildRecommenderShortcut "$SMPROGRAMS"
  !endif
!macroend
