pub fn install(webview: &tauri::Webview) {
    #[cfg(windows)]
    if webview.label() == "main" {
        let _ = webview.eval(include_str!("windows-ime-focus.js"));
    }
    #[cfg(not(windows))]
    let _ = webview;
}

#[tauri::command]
pub async fn repair_windows_ime_focus(
    webview: tauri::Webview,
    requested_at: u64,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        use webview2_com::Microsoft::Web::WebView2::Win32::COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC;
        use windows::Win32::UI::{
            Input::{
                Ime::{GCS_COMPSTR, ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext},
                KeyboardAndMouse::{GetFocus, SetFocus},
            },
            WindowsAndMessaging::GetForegroundWindow,
        };

        if webview.label() != "main" {
            return Ok(());
        }
        let parent = webview.window().hwnd().map_err(|e| e.to_string())?.0 as usize;
        let (sender, receiver) = tokio::sync::oneshot::channel();
        webview
            .with_webview(move |platform| {
                let result = (|| -> Result<(), String> {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|e| e.to_string())?
                        .as_millis();
                    // 遅れて届いた操作や、別アプリの操作中には入力先を変更しない。
                    if now.saturating_sub(u128::from(requested_at)) > 100
                        || unsafe { GetForegroundWindow().0 as usize } != parent
                    {
                        return Ok(());
                    }
                    // Windows側のフォーカスを再通知する。DOMの文字・選択位置は変更しない。
                    unsafe {
                        let focused = GetFocus();
                        if focused.0.is_null() {
                            return Ok(());
                        }
                        let context = ImmGetContext(focused);
                        if !context.0.is_null() {
                            let composing = ImmGetCompositionStringW(context, GCS_COMPSTR, None, 0);
                            let _ = ImmReleaseContext(focused, context);
                            if composing > 0 {
                                return Ok(());
                            }
                        }
                        SetFocus(None).map_err(|e| e.to_string())?;
                        if let Err(error) = platform
                            .controller()
                            .MoveFocus(COREWEBVIEW2_MOVE_FOCUS_REASON_PROGRAMMATIC)
                        {
                            // 再接続に失敗しても、元の入力先へ戻す。
                            let _ = SetFocus(Some(focused));
                            return Err(error.to_string());
                        }
                    }
                    #[cfg(debug_assertions)]
                    eprintln!("IME確認: Windows側の入力先を再接続しました");
                    Ok(())
                })();
                let _ = sender.send(result);
            })
            .map_err(|e| e.to_string())?;
        receiver.await.map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = (webview, requested_at);
        Ok(())
    }
}
