#[cfg(windows)]
pub struct Tray {
    _icon: tray_icon::TrayIcon,
    pub show_id: tray_icon::menu::MenuId,
    pub quit_id: tray_icon::menu::MenuId,
}

#[cfg(windows)]
pub type TraySlot = Option<Tray>;
#[cfg(not(windows))]
pub type TraySlot = ();

pub fn make_tray(enabled: bool) -> TraySlot {
    #[cfg(windows)]
    {
        if enabled { create_tray() } else { None }
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        ()
    }
}

#[cfg(windows)]
fn create_tray() -> Option<Tray> {
    use tray_icon::menu::{Menu, MenuItem, PredefinedMenuItem};

    let show = MenuItem::new("Show / Hide", true, None);
    let quit = MenuItem::new("Quit", true, None);
    let menu = Menu::new();
    menu.append(&show).ok()?;
    menu.append(&PredefinedMenuItem::separator()).ok()?;
    menu.append(&quit).ok()?;

    let icon = tray_icon::Icon::from_rgba(crate::icon::rgba(32), 32, 32).ok()?;
    let icon = tray_icon::TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_tooltip("Status Panel")
        .with_icon(icon)
        .build()
        .ok()?;

    Some(Tray {
        _icon: icon,
        show_id: show.id().clone(),
        quit_id: quit.id().clone(),
    })
}

