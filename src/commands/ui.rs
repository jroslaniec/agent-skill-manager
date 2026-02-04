use inquire::ui::{Color, RenderConfig, StyleSheet, Styled};

pub fn render_config() -> RenderConfig<'static> {
    RenderConfig::default_colored()
        .with_prompt_prefix(Styled::new(""))
        .with_answered_prompt_prefix(Styled::new(""))
        .with_highlighted_option_prefix(Styled::new("▸"))
        .with_selected_checkbox(Styled::new("●").with_fg(Color::LightGreen))
        .with_unselected_checkbox(Styled::new("○"))
        .with_selected_option(None)
        .with_option(StyleSheet::new())
}
