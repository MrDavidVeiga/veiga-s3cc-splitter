use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemAppearance {
    pub mode: Option<String>,
    pub accent: Option<String>,
    pub accent_light_1: Option<String>,
    pub accent_light_2: Option<String>,
    pub accent_light_3: Option<String>,
    pub accent_dark_1: Option<String>,
    pub accent_dark_2: Option<String>,
    pub accent_dark_3: Option<String>,
    pub source: String,
}

impl SystemAppearance {
    fn fallback(source: &str) -> Self {
        Self {
            mode: None, accent: None,
            accent_light_1: None, accent_light_2: None, accent_light_3: None,
            accent_dark_1: None, accent_dark_2: None, accent_dark_3: None,
            source: source.to_string(),
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_appearance() -> Option<SystemAppearance> {
    use windows::UI::Color;
    use windows::UI::ViewManagement::{UIColorType, UISettings};

    fn hex(color: Color) -> String { format!("#{:02X}{:02X}{:02X}", color.R, color.G, color.B) }
    fn luminance(color: Color) -> f32 {
        0.2126 * color.R as f32 / 255.0 + 0.7152 * color.G as f32 / 255.0 + 0.0722 * color.B as f32 / 255.0
    }

    let settings=UISettings::new().ok()?;
    let background=settings.GetColorValue(UIColorType::Background).ok()?;
    let mode=if luminance(background)>0.5 { "light" } else { "dark" };
    let read=|kind| settings.GetColorValue(kind).ok().map(hex);

    Some(SystemAppearance {
        mode:Some(mode.to_string()),
        accent:read(UIColorType::Accent),
        accent_light_1:read(UIColorType::AccentLight1),
        accent_light_2:read(UIColorType::AccentLight2),
        accent_light_3:read(UIColorType::AccentLight3),
        accent_dark_1:read(UIColorType::AccentDark1),
        accent_dark_2:read(UIColorType::AccentDark2),
        accent_dark_3:read(UIColorType::AccentDark3),
        source:"windows-ui-settings".to_string(),
    })
}

#[cfg(target_os = "macos")]
fn macos_appearance() -> SystemAppearance {
    use std::process::Command;
    fn read_default(key:&str)->Option<String> {
        let output=Command::new("defaults").args(["read","-g",key]).output().ok()?;
        if !output.status.success() { return None; }
        let value=String::from_utf8(output.stdout).ok()?.trim().to_string();
        if value.is_empty() { None } else { Some(value) }
    }

    let mode=match read_default("AppleInterfaceStyle") {
        Some(value) if value.eq_ignore_ascii_case("dark")=>Some("dark".to_string()),
        _=>Some("light".to_string()),
    };
    let accent=match read_default("AppleAccentColor").as_deref() {
        Some("-1")=>"#8E8E93", Some("0")=>"#FF3B30", Some("1")=>"#FF9500",
        Some("2")=>"#FFCC00", Some("3")=>"#34C759", Some("4")=>"#007AFF",
        Some("5")=>"#AF52DE", Some("6")=>"#FF2D55", _=>"#007AFF",
    };

    SystemAppearance {
        mode, accent:Some(accent.to_string()),
        accent_light_1:None, accent_light_2:None, accent_light_3:None,
        accent_dark_1:None, accent_dark_2:None, accent_dark_3:None,
        source:"macos-system-preferences".to_string(),
    }
}

#[cfg(target_os = "linux")]
fn linux_appearance() -> SystemAppearance {
    use std::process::Command;
    fn portal_read(key:&str)->Option<String> {
        let output=Command::new("gdbus").args([
            "call","--session","--dest","org.freedesktop.portal.Desktop",
            "--object-path","/org/freedesktop/portal/desktop",
            "--method","org.freedesktop.portal.Settings.Read",
            "org.freedesktop.appearance",key,
        ]).output().ok()?;
        if !output.status.success() { return None; }
        String::from_utf8(output.stdout).ok()
    }

    let mode=portal_read("color-scheme").and_then(|text| {
        let pos=text.find("uint32")?;
        let digits:String=text[pos+6..].chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
        match digits.as_str() { "1"=>Some("dark".to_string()), "2"=>Some("light".to_string()), _=>None }
    });

    let accent=portal_read("accent-color").and_then(|text| {
        let mut values=Vec::new();
        let mut token=String::new();
        for ch in text.chars() {
            if ch.is_ascii_digit() || ch=='.' || ch=='-' { token.push(ch); }
            else if !token.is_empty() {
                if let Ok(value)=token.parse::<f64>() { if (0.0..=1.0).contains(&value) { values.push(value); } }
                token.clear();
            }
        }
        if !token.is_empty() {
            if let Ok(value)=token.parse::<f64>() { if (0.0..=1.0).contains(&value) { values.push(value); } }
        }
        if values.len()<3 { return None; }
        let rgb=&values[values.len()-3..];
        Some(format!("#{:02X}{:02X}{:02X}",
            (rgb[0]*255.0).round() as u8,
            (rgb[1]*255.0).round() as u8,
            (rgb[2]*255.0).round() as u8))
    });

    SystemAppearance {
        mode, accent,
        accent_light_1:None, accent_light_2:None, accent_light_3:None,
        accent_dark_1:None, accent_dark_2:None, accent_dark_3:None,
        source:"xdg-desktop-portal".to_string(),
    }
}

#[tauri::command]
pub fn get_system_appearance() -> SystemAppearance {
    #[cfg(target_os = "windows")]
    { return windows_appearance().unwrap_or_else(|| SystemAppearance::fallback("windows-fallback")); }
    #[cfg(target_os = "macos")]
    { return macos_appearance(); }
    #[cfg(target_os = "linux")]
    { return linux_appearance(); }
    #[allow(unreachable_code)]
    SystemAppearance::fallback("unsupported-platform")
}
