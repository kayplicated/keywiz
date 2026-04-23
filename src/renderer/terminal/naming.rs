//! Human-readable names for physical key ids (terminal use).
//!
//! Translates opaque ids like `mods_shift_left`, `fn_f3`, `nav_up`
//! into UI-friendly labels. Unknown ids fall back verbatim.

pub fn human_name(id: &str) -> String {
    if let Some(rest) = id.strip_prefix("mods_") {
        return mods_name(rest);
    }
    if let Some(rest) = id.strip_prefix("fn_f") {
        return format!("F{rest}");
    }
    if let Some(rest) = id.strip_prefix("nav_") {
        return title_case(rest);
    }
    if let Some(rest) = id.strip_prefix("num_pad_") {
        return format!("Num {}", title_case(rest));
    }
    if let Some(k_idx) = id.rfind("_k")
        && id[k_idx + 2..].chars().all(|c| c.is_ascii_digit())
    {
        return id[k_idx + 1..].to_string();
    }
    id.to_string()
}

fn mods_name(name: &str) -> String {
    match name {
        "escape" => "Esc".into(),
        "tab" => "Tab".into(),
        "capslock" => "Caps".into(),
        "shift_left" => "L-Shift".into(),
        "shift_right" => "R-Shift".into(),
        "ctrl_left" => "L-Ctrl".into(),
        "ctrl_right" => "R-Ctrl".into(),
        "alt_left" => "L-Alt".into(),
        "alt_right" => "R-Alt".into(),
        "meta_left" => "L-Meta".into(),
        "meta_right" => "R-Meta".into(),
        "space" => "Space".into(),
        "enter" => "Enter".into(),
        "backspace" => "Bksp".into(),
        "delete" => "Del".into(),
        "menu" => "Menu".into(),
        "fn" => "Fn".into(),
        "shift" => "Shift".into(),
        "ctrl" => "Ctrl".into(),
        "alt" => "Alt".into(),
        "meta" => "Meta".into(),
        other => title_case(other),
    }
}

fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut cap = true;
    for ch in s.chars() {
        if ch == '_' {
            out.push(' ');
            cap = true;
        } else if cap {
            out.extend(ch.to_uppercase());
            cap = false;
        } else {
            out.push(ch);
        }
    }
    out
}
