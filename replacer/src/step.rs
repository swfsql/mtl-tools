use yew::prelude::*;

pub enum VirtualSort {
    None,
    CharLength,
    CharLengthRev,
}

pub struct StepProps {
    /// The step title.
    pub title: String,
    /// Whether the step is enabled during a replacement run.
    pub enabled: bool,
    /// Whether it is selected for edit.
    pub selected: bool,
    /// Whether any regex match triggers a return to the first regex.
    pub restart_on_match: bool,
    /// In which regex ordering should replacement run on.
    pub virtual_sort: VirtualSort,
}

pub struct RegexInfo {
    pub title: String,
    pub r#match: Result<regex::Regex, String>,
    pub match_parse_error: Option<regex::Error>,
    pub replace: String,
}

impl Default for RegexInfo {
    fn default() -> Self {
        Self {
            title: Default::default(),
            r#match: Err("".into()),
            match_parse_error: Default::default(),
            replace: Default::default(),
        }
    }
}

impl Default for StepProps {
    fn default() -> Self {
        Self {
            title: "".into(),
            enabled: true,
            selected: false,
            restart_on_match: true,
            virtual_sort: VirtualSort::None,
        }
    }
}

#[derive(Default)]
pub struct Step {
    // TODO: refactor out
    pub props: StepProps,
    pub regexes: Vec<RegexInfo>,
}

impl Step {
    pub fn render(&self) -> Html {
        html! {
            // <MatListItem>{&self.props.title}</MatListItem>
        }
    }
}
