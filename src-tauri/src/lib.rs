#[cfg(target_os = "windows")]
use std::time::Duration;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};

#[cfg(target_os = "windows")]
mod inactivity;

fn init_sound_inactivity_monitor() {
    std::thread::Builder::new()
        .name("sound-inactive-init".into())
        .spawn(|| {
            println!("Iniciando monitoramento de inatividade sonora...");
            #[cfg(target_os = "windows")]
            if let Err(err) = inactivity::start_monitor() {
                eprintln!(
                    "[sound-inactive] falha ao iniciar monitoramento de inatividade sonora: {err}"
                );
            }
            #[cfg(not(target_os = "windows"))]
            {
                eprintln!("Funcionalidade disponivel apenas no Windows.");
            }
        })
        .expect("nao foi possivel criar a thread de inicializacao do monitoramento");
}

#[tauri::command]
fn set_sound_inactivity_timeout(minutes: Option<u64>) -> Result<(), String> {
    let minutes = minutes.unwrap_or(5);

    if minutes == 0 {
        return Err("O tempo de inatividade deve ser maior que zero.".into());
    }

    #[cfg(target_os = "windows")]
    {
        let duration = Duration::from_secs(minutes.saturating_mul(60));
        return inactivity::set_inactivity_threshold(duration);
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = minutes;
        Err("Funcionalidade disponivel apenas no Windows.".into())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            init_sound_inactivity_monitor();

            use tauri_plugin_autostart::MacosLauncher;

            let _ = app.handle().plugin(tauri_plugin_autostart::init(
                MacosLauncher::LaunchAgent,
                Some(vec!["--flag1", "--flag2"]),
            ));

            {
                let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
                let toggle =
                    MenuItem::with_id(app, "toggle", "Alternar Janela", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle, &quit])?;

                TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            let app_handle = tray.app_handle();
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    })
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "toggle" => {
                            let app_handle = app.app_handle();
                            if let Some(window) = app_handle.get_webview_window("main") {
                                if window.is_visible().unwrap_or(false) {
                                    let _ = window.hide();
                                } else {
                                    let _ = window.show();
                                    let _ = window.set_focus();
                                }
                            }
                        }
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .icon(
                        app.default_window_icon()
                            .expect("missing tray icon")
                            .clone(),
                    )
                    .build(app)?;
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .invoke_handler(tauri::generate_handler![set_sound_inactivity_timeout])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
