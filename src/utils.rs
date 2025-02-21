use directories::ProjectDirs;
use tui::style::Style;

/// Get directories for the project for config, cache, etc.
pub fn get_project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("com", "ccgauche", "ytermusic")
}

pub fn invert(style: Style) -> Style {
    Style {
        fg: style.bg,
        bg: style.fg,
        add_modifier: style.add_modifier,
        sub_modifier: style.sub_modifier,
    }
}
