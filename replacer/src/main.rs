#![feature(stmt_expr_attributes)]

pub mod step;
pub mod text_project;

use indexmap::IndexSet;
use regex::Regex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use step::{RegexInfo, Step};
use text_project::CancelMotive;
use text_project::{OutputStatus, TextProject};
use yew::prelude::*;

pub type StepIndex = usize;
pub type RegexIndex = usize;
pub type ProjectIndex = usize;
pub type Confirmed = bool;

pub enum MoveDirection {
    Up,
    Down,
}

pub enum Msg {
    // Step
    AddStep,
    SelectStep(StepIndex),
    SetStepEnabled(StepIndex, bool),
    UpdateStepTitle(StepIndex, String),
    AddRegex(StepIndex),
    UpdateRegexTitle(StepIndex, RegexIndex, String),
    UpdateRegexSearch(StepIndex, RegexIndex, String),
    UpdateRegexReplacement(StepIndex, RegexIndex, String),
    DeleteRegex(StepIndex, RegexIndex, Confirmed),
    MoveRegex(StepIndex, RegexIndex, MoveDirection),

    // Text Project
    AddTextProject,
    SelectTextProject(ProjectIndex),
    UpdateTextProjectTitle(ProjectIndex, String),
    StartReplacingText(Option<ProjectIndex>),
    CancelReplacingText(),
    FinishReplacingText(ProjectIndex, String),
    CancelledReplacingText(ProjectIndex, CancelMotive, String),

    // Input/Output
    InputUpdated(ProjectIndex, String),
    OutputUpdated(ProjectIndex, String),
}

pub struct Model {
    // text projects
    pub text_projects: Vec<TextProject>,
    pub active_text_project: Option<usize>,
    pub replacement_in_progress: bool,
    pub replacement_cancel_signal: Arc<AtomicBool>,

    // steps
    pub steps: Vec<Step>,
    pub steps_edit: IndexSet<usize>,

    // regexes
    pub active_regex_index: Option<usize>,
}

pub async fn replace_text(
    original: String,
    steps_regexes: Vec<Vec<(regex::Regex, String)>>,
    cancel_signal: Arc<AtomicBool>,
) -> Result<String, (CancelMotive, String)> {
    use crc32fast::Hasher;
    use std::collections::{HashMap, HashSet};

    let ms = std::time::Duration::from_millis(1);
    let original_len = original.len();
    let mut content = original;
    let mut group_count = 0;
    for step_regexes in &steps_regexes {
        let mut hash_maps = HashMap::<usize, Option<HashSet<_>>>::new();
        let mut ever_changed = false;
        loop {
            // check for replacement cycles
            //
            // first check the content length

            match hash_maps.get_mut(&content.len()) {
                // if it's a new length, then don't even calculate a hash from it
                None => {
                    hash_maps.insert(content.len(), None);
                }

                // if we hit the same length again, then we start tracking it's hash
                //
                // if it's a cycle, it should be eventually detected
                Some(hash_sets) => {
                    log::info!("checksum");
                    let mut hasher = Hasher::new();
                    hasher.update(content.as_bytes());
                    let hash = hasher.finalize();
                    if let Some(hashes) = hash_sets {
                        let is_new_insertion = hashes.insert(hash);
                        if !is_new_insertion {
                            log::warn!("Replacement cycle detected. Cancelling automatically.");
                            return Err((CancelMotive::CycleDetected, content));
                        }
                    } else {
                        let mut hashes = HashSet::new();
                        hashes.insert(hash);
                        let _ = hash_sets.insert(hashes);
                    }
                }
            }

            if cancel_signal.load(Ordering::SeqCst) {
                log::info!("Replacement cancelled.");
                return Err((CancelMotive::ManuallyCancelled, content));
            }
            if content.len() > 4 * original_len && content.len() > 1000 {
                log::warn!("Resulting text is growing too much from the replacement and thus has been automatically cancelled.");
                return Err((CancelMotive::HighGrowth, content));
            }
            gloo_timers::future::sleep(ms).await;
            let mut just_replaced = false;
            for (re, replacement) in step_regexes {
                if re.is_match(&content) {
                    // apply the highest priority substitution
                    content = re.replace_all(&content, replacement).into_owned();

                    just_replaced = true;
                    group_count += 1;

                    // allow to restart the step regexes
                    // (allowing higher priorities substitutions)
                    break;
                }
            }
            if just_replaced {
                ever_changed = true;
                // restart the step regexes
                // (allowing higher priorities substitutions)
                continue;
            } else {
                if ever_changed {}
                // finished the current step regexes
                break;
            }
        }
        // continue to the next step regexes
    }
    Ok(content)
}

impl Component for Model {
    type Message = Msg;
    type Properties = ();

    fn create(_ctx: &Context<Self>) -> Self {
        let mut steps_edit = IndexSet::new();
        steps_edit.insert(0);
        let mut steps = vec![Step::default()];
        for i in &steps_edit {
            steps[*i].props.selected = true;
        }
        Self {
            text_projects: vec![TextProject::default()],
            active_text_project: Some(0),
            replacement_in_progress: false,
            replacement_cancel_signal: Arc::new(AtomicBool::new(false)),
            steps,
            steps_edit,
            active_regex_index: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::AddStep => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Added step won't affect the replacement that is already in progress."
                    );
                }
                self.steps.push(Step::default());
                true
            }
            Msg::SelectStep(index) => {
                if self.steps_edit.contains(&index) {
                    self.steps_edit.remove(&index);
                    self.steps[index].props.selected = false;
                    true
                } else {
                    self.steps_edit.insert(index);
                    self.steps[index].props.selected = true;
                    true
                }
            }
            Msg::SetStepEnabled(index, value) => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Modified step won't affect the replacement that is already in progress."
                    );
                }
                self.steps[index].props.enabled = value;
                true
            }
            Msg::UpdateStepTitle(step_index, title) => {
                self.steps[step_index].props.title = title;
                true
            }
            Msg::AddRegex(step_index) => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Added regex won't affect the replacement that is already in progress."
                    );
                }
                self.steps[step_index].regexes.push(RegexInfo::default());
                true
            }
            Msg::UpdateRegexTitle(step_index, regex_index, title) => {
                self.steps[step_index].regexes[regex_index].title = title;
                true
            }
            Msg::UpdateRegexSearch(step_index, regex_index, search) => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Changed regex won't affect the replacement that is already in progress."
                    );
                }
                let r = &mut self.steps[step_index].regexes[regex_index];
                match Regex::new(&search) {
                    Ok(re) => {
                        r.r#match = Ok(re);
                        r.match_parse_error = None;
                        true
                    }
                    Err(err) => {
                        r.r#match = Err(search);
                        r.match_parse_error = Some(err);
                        true
                    }
                }
            }
            Msg::UpdateRegexReplacement(step_index, regex_index, replacement) => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Changed regex won't affect the replacement that is already in progress."
                    );
                }
                self.steps[step_index].regexes[regex_index].replace = replacement;
                true
            }
            Msg::DeleteRegex(step_index, regex_index, confirmed) => {
                if self.replacement_in_progress {
                    log::warn!(
                        "Removed regex won't affect the replacement that is already in progress."
                    );
                }
                if confirmed {
                    self.steps[step_index].regexes.remove(regex_index);
                    true
                } else {
                    true
                }
            }
            Msg::MoveRegex(step_index, regex_index, direction) => {
                if self.replacement_in_progress {
                    log::warn!("Re-ordered regexes won't affect the replacement that is already in progress.");
                }
                let regexes = &mut self.steps[step_index].regexes;
                let len = regexes.len();

                match direction {
                    MoveDirection::Up => {
                        if regex_index >= 1 {
                            regexes.swap(regex_index - 1, regex_index);
                            true
                        } else {
                            false
                        }
                    }
                    MoveDirection::Down => {
                        if regex_index + 1 < len {
                            regexes.swap(regex_index, regex_index + 1);
                            true
                        } else {
                            false
                        }
                    }
                }
            }
            Msg::InputUpdated(project_index, value) => {
                if self.replacement_in_progress {
                    log::error!("A replacement is already in progress.");
                    return false;
                }
                let project = &mut self.text_projects[project_index];
                project.input = value;
                project.output_status = OutputStatus::Outdated;
                // project.output = value;
                true
            }
            Msg::OutputUpdated(_project_index, _discarded_value) => {
                log::error!("This should never be triggered.");
                false
            }
            Msg::AddTextProject => {
                let next_text_project = TextProject::default();
                self.text_projects.push(next_text_project);
                self.active_text_project = Some(self.text_projects.len() - 1);
                true
            }
            Msg::SelectTextProject(index) => {
                if Some(index) == self.active_text_project {
                    false
                } else {
                    self.active_text_project = Some(index);
                    true
                }
            }
            Msg::UpdateTextProjectTitle(index, title) => {
                let project = &mut self.text_projects[index];
                project.props.title = title;
                true
            }
            Msg::StartReplacingText(project_index) => {
                if let Some(project_index) = project_index {
                    if self.replacement_in_progress {
                        log::error!("Replacement already in progress");
                        return false;
                    }

                    self.replacement_in_progress = true;
                    let project = &mut self.text_projects[project_index];
                    project.output_status = OutputStatus::InProgress;

                    let mut regexes = vec![];

                    for (i, step) in self.steps.iter().enumerate() {
                        let mut regexes_i = vec![];

                        for re in step.regexes.iter() {
                            let r#match = match &re.r#match {
                                Ok(r) => r,
                                Err(s) if s.is_empty() => {
                                    continue;
                                }
                                Err(s) => {
                                    log::error!("The regex {} had a parse error", s);
                                    return true;
                                }
                            };
                            let repl = &re.replace;
                            regexes_i.push((r#match.clone(), repl.clone()));
                        }
                        regexes.push(regexes_i);
                    }

                    let mut content = project.input.clone();

                    self.replacement_cancel_signal
                        .store(false, Ordering::SeqCst);
                    let cancel_signal = self.replacement_cancel_signal.clone();
                    ctx.link().send_future(async move {
                        content = match replace_text(content, regexes, cancel_signal).await {
                            Ok(content) => content,
                            Err((motive, content)) => {
                                return Msg::CancelledReplacingText(project_index, motive, content);
                            }
                        };

                        Msg::FinishReplacingText(project_index, content)
                    });

                    true
                } else {
                    false
                }
            }
            Msg::CancelReplacingText() => {
                if self.replacement_in_progress {
                    self.replacement_cancel_signal.store(true, Ordering::SeqCst);
                    false
                } else {
                    log::error!("No replacement not in progress");
                    false
                }
            }
            Msg::FinishReplacingText(project_index, content) => {
                self.replacement_in_progress = false;
                let project = &mut self.text_projects[project_index];
                project.output = content;
                project.output_status = OutputStatus::Done;
                self.replacement_cancel_signal
                    .store(false, Ordering::SeqCst);

                true
            }
            Msg::CancelledReplacingText(project_index, cancel_motive, latest_content) => {
                self.replacement_in_progress = false;
                let project = &mut self.text_projects[project_index];
                project.output = latest_content;
                project.output_status = OutputStatus::Cancelled(cancel_motive);
                self.replacement_cancel_signal
                    .store(false, Ordering::SeqCst);

                true
            }
        }
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        use ybc::InputType;
        use ybc::NavbarFixed::Top;
        use ybc::NavbarItemTag::{Div, A};
        use ybc::TileCtx::{Ancestor, Child, Parent};
        use ybc::TileSize::*;

        let link = ctx.link();

        let new_step = link.callback(|_| Msg::AddStep);

        let active_text_project_index = self.active_text_project;

        let navbar = {
            let navbrand = html_nested! {<div />};
            let navstart = html_nested! {<div />};
            let nav_steps = {
                let navlink = html! {"Steps"};
                html! {<>
                    <ybc::NavbarDropdown
                        {navlink}
                        hoverable=true
                        right=true
                    >
                    <ybc::NavbarItem>
                        <div onclick={new_step}><ybc::Button>
                            {"Add Step"}
                        </ybc::Button></div>
                    </ybc::NavbarItem>

                    { for self.steps.iter().enumerate().map(|(i, step)| {
                        let set_enabled = link.callback(move |value| Msg::SetStepEnabled(i, value));
                        let onclick = link.callback(move |_| Msg::SelectStep(i));
                        html_nested!{
                            <ybc::NavbarItem
                                tag={A}
                                classes={classes!(
                                    if step.props.selected {
                                        "is-highlighted"
                                    } else {
                                        ""
                                    }
                                )}
                                href={"#"}
                            >
                                <ybc::Checkbox
                                    name={format!("step-{}-enabled", i)}
                                    checked={step.props.enabled}
                                    update={set_enabled}
                                />
                                <span
                                    {onclick}
                                    class={"ml-1"}
                                >
                                    {format!(" {} - ", i + 1)}
                                    {if step.props.title.trim().is_empty() {
                                        "New Step"
                                    } else {
                                        &step.props.title
                                    }}
                                </span>
                            </ybc::NavbarItem>
                        }
                    }) }
                    </ybc::NavbarDropdown>
                </>}
            };

            let navend = nav_steps;

            html_nested! {
                <ybc::Navbar
                    {navbrand}
                    {navstart}
                    {navend}
                    fixed={Top}
                    transparent=false
                    navburger=true
                    spaced=true
                    classes={classes!(
                        "has-background-info-light",
                    )}
                >
                </ybc::Navbar>
            }
        };

        let edit_steps = {
            html_nested! {
                { for self.steps_edit.iter().map(|i| {
                    let i = *i;
                    let step = self.steps.get(i).unwrap();
                    let total_steps = self.steps.len();
                    let total_regexes = step.regexes.len();
                    let update_step_title = link.callback(move |t| Msg::UpdateStepTitle(i, t));
                    let add_regex = link.callback(move |_| Msg::AddRegex(i));
                    let close_step = link.callback(move |_| Msg::SelectStep(i));
                    html_nested!{

                        <ybc::Columns
                            classes={classes!("is-centered")}
                        >
                        <ybc::Column
                            classes={classes!("is-half")}
                        >
                        <ybc::Message
                            classes={classes!("is-info")}
                        >
                            <ybc::MessageHeader
                            >

                                {format!("Step {}/{}", i + 1, total_steps)}
                                <ybc::Delete
                                    tag={"button"}
                                    onclick={close_step}
                                />

                            </ybc::MessageHeader>
                            <ybc::MessageBody
                            >

                                <ybc::Field
                                    label={"Step Name"}
                                >
                                <ybc::Control
                                    tag={"div"}
                                    classes={classes!("has-icons-left")}
                                >
                                <ybc::Input
                                    name={format!("step-{}-title", i)}
                                    value={step.props.title.clone()}
                                    update={update_step_title}
                                    placeholder={r#"Optionally add a step name. Defaults to "New Step"."#}
                                />
                                <span class="icon is-small is-left">
                                    <i class="fas fa-info" />
                                </span>
                                </ybc::Control>
                                </ybc::Field>




                                <p>{"(add option to delete the step)"}</p>




                        <ybc::Tile ctx={Parent} vertical=true>
                        { for step.regexes.iter().enumerate().map(|(j, r)| {
                            use ybc::Size::Small;
                            let update_regex_title = link.callback(move |t| Msg::UpdateRegexTitle(i, j, t));
                            let update_regex_match = link.callback(move |s| Msg::UpdateRegexSearch(i, j, s));
                            let update_regex_replace = link.callback(move |s| Msg::UpdateRegexReplacement(i, j, s));
                            let delete_regex = link.callback(move |_| Msg::DeleteRegex(i, j, true));
                            let move_regex_up = link.callback(move |_| Msg::MoveRegex(i, j, MoveDirection::Up));
                            let move_regex_down = link.callback(move |_| Msg::MoveRegex(i, j, MoveDirection::Down));
                            let (re_text, re_error) = match &r.r#match {
                                Ok(re) => (re.to_string(), None),
                                Err(re) => (re.clone(), r.match_parse_error.clone())
                            };
                            html_nested! {
                                <ybc::Tile ctx={Child} classes={classes!("box")}>
                                    <ybc::Subtitle
                                        size={ybc::HeaderSize::Is6}
                                    >
                                        {format!("Regex {}/{}", j + 1, total_regexes)}
                                    </ybc::Subtitle>

                                    <ybc::Field grouped=true>
                                        <a onclick={move_regex_up}><ybc::Button
                                            classes={classes!("is-small")}
                                            disabled={j == 0}
                                        >
                                            <span class="icon is-small">
                                                <i class="fas fa-arrow-up"></i>
                                            </span>
                                        </ybc::Button></a>
                                        <a onclick={move_regex_down}><ybc::Button
                                            classes={classes!("is-small")}
                                            disabled={j + 1 == total_regexes}
                                        >
                                            <span class="icon is-small">
                                                <i class="fas fa-arrow-down"></i>
                                            </span>
                                        </ybc::Button></a>
                                        <a onclick={delete_regex}><ybc::Button classes={classes!("is-small")}>
                                            <span class="icon is-small">
                                                <i class="fas fa-trash"></i>
                                            </span>
                                        </ybc::Button></a>
                                    </ybc::Field>

                                    <ybc::Field
                                        label={"Regex Description"}
                                        label_classes={classes!("is-small")}
                                    >
                                    <ybc::Control
                                        tag={"div"}
                                        classes={classes!("has-icons-left")}
                                    >
                                    <ybc::Input
                                        name={format!("step-{}-regex-{}-title", i, j)}
                                        value={r.title.clone()}
                                        update={update_regex_title}
                                        placeholder={r#"Optionally add a regex description."#}
                                        size={Small}
                                    />
                                    <span class="icon is-small is-left">
                                        <i class="fas fa-info" />
                                    </span>
                                    </ybc::Control>
                                    </ybc::Field>
                                    <ybc::Field
                                        label={"Regex Match"}
                                        label_classes={classes!("is-small")}
                                        help={
                                            if let Some(err) = &re_error {
                                                let err = err.to_string();
                                                if err.trim().is_empty() {
                                                    "unknown error".to_string()
                                                } else {
                                                    err
                                                }
                                            } else if !re_text.is_empty() {
                                                "".to_string()
                                            } else {
                                                "No match defined. This regex will be ignored.".to_string()
                                            }
                                        }
                                    >
                                    <ybc::Control
                                        tag={"div"}
                                        classes={classes!("has-icons-left")}
                                    >
                                    <ybc::Input
                                        name={format!("step-{}-regex-{}-match", i, j)}
                                        value={re_text}
                                        update={update_regex_match}
                                        placeholder={r#"What to try to match. Eg. "ABC"."#}
                                        classes={classes!(
                                            if re_error.is_some() {
                                                "is-danger"
                                            } else if !re_text.is_empty() {
                                                "is-success"
                                            } else {
                                                ""
                                            }
                                        )}
                                        size={Small}
                                    />
                                    <span class="icon is-small is-left">
                                        <i class="fas fa-search" />
                                    </span>
                                    </ybc::Control>
                                    </ybc::Field>
                                    <ybc::Field
                                        label={"Regex Replacement"}
                                        label_classes={classes!("is-small")}
                                        help={
                                            if r.replace.is_empty() {
                                                "The replacement is empty. This will erase the matched content."
                                            } else {
                                                ""
                                            }
                                        }
                                    >
                                    <ybc::Control
                                        tag={"div"}
                                        classes={classes!("has-icons-left")}
                                    >
                                    <ybc::Input
                                        name={format!("step-{}-regex-{}-replacement", i, j)}
                                        value={r.replace.clone()}
                                        update={update_regex_replace}
                                        placeholder={r#"What the matches will be replaced with. Eg. "XYZ"."#}
                                        size={Small}
                                        classes={classes!(
                                            if r.replace.is_empty() {
                                                "is-warning"
                                            } else {
                                                ""
                                            }
                                        )}
                                    />
                                    <span class="icon is-small is-left">
                                        <i class="fas fa-paste" />
                                    </span>
                                    </ybc::Control>
                                    </ybc::Field>
                                    <p>{"(add option to delete the regex)"}</p>
                                    <p>{"(add option to move up/down the regex)"}</p>
                                </ybc::Tile>
                            }
                        })}
                        </ybc::Tile>

                                <a onclick={add_regex}><ybc::Button>
                                    <span class="icon is-small">
                                        <i class="fas fa-hand-point-up"></i>
                                    </span>
                                    <span>
                                        {"Add Regex"}
                                    </span>
                                </ybc::Button></a>

                        </ybc::MessageBody>
                    </ybc::Message>
                    </ybc::Column>
                    </ybc::Columns>
                    }
                })}
            }
        };

        let tabs = if let Some(active_text_project) = active_text_project_index {
            html_nested! {
                <ybc::Tabs boxed=true>
                    {for self.text_projects.iter().enumerate().map(|(i, t)| {
                        let active = i == active_text_project;
                        let title = &t.props.title;
                        let title = if title.trim().is_empty() {
                            "New Project"
                        } else {
                            title
                        };

                        html_nested!{
                            <li class={classes!(
                                if active {"is-active"} else {""},
                                if i == 0 {"ml-6"} else {""}
                            )}>
                                <a onclick={
                                    link.callback(move |_| Msg::SelectTextProject(i))
                                }>{&title}</a>
                            </li>
                        }
                    })}
                    <li>
                        <a onclick={link.callback(|_| Msg::AddTextProject)}>
                            {"+"}
                        </a>
                    </li>
                </ybc::Tabs>
            }
        } else {
            html_nested! {
                <ybc::Tabs boxed=true>
                    <li>
                        <a onclick={link.callback(|_| Msg::AddTextProject)}>
                            {"+"}
                        </a>
                    </li>
                </ybc::Tabs>
            }
        };

        let edit_project_title = if let Some(active_text_project_index) = active_text_project_index
        {
            let active_text_project = &self.text_projects[active_text_project_index];
            let update_project_title =
                link.callback(move |t| Msg::UpdateTextProjectTitle(active_text_project_index, t));
            html_nested! {
                <ybc::Tile ctx={Child}><ybc::Field
                    label={"Text Project Title"}
                ><ybc::Input
                    name={format!("project-title-{}", active_text_project_index)}
                    value={active_text_project.props.title.clone()}
                    update={update_project_title}
                    placeholder={r#"The project title. Eg. "Ch015 Google Translate". Defaults to "New Project"."#}
                /></ybc::Field></ybc::Tile>
            }
        } else {
            html_nested! {<ybc::Tile ctx={Child}></ybc::Tile>}
        };

        let input = if let Some(active_text_project_index) = active_text_project_index {
            let active_text_project = &self.text_projects[active_text_project_index];
            html_nested! {
                <ybc::Tile ctx={Child}><ybc::Field
                    label={"Original Text"}
                    help={"Help message"}
                ><ybc::TextArea
                    name={"original-text"}
                    value={active_text_project.input.clone()}
                    update={link.callback(move |value: String| Msg::InputUpdated(active_text_project_index, value.clone()))}
                    placeholder={"Add the original text here.."}
                    rows=6
                /></ybc::Field></ybc::Tile>
            }
        } else {
            html_nested! {<ybc::Tile ctx={Child}></ybc::Tile>}
        };

        let output = if let Some(active_text_project_index) = active_text_project_index {
            let active_text_project = &self.text_projects[active_text_project_index];
            let status = &active_text_project.output_status;

            let help = match status {
                OutputStatus::Outdated => "This contains an outdated result.",
                OutputStatus::InProgress => {
                    "This contains an outdated result. A new result is being produced.."
                }
                OutputStatus::Done => "This contains the latest replacement.",
                OutputStatus::Cancelled(CancelMotive::ManuallyCancelled) => {
                    "This result is incomplete. The replacement was manually cancelled."
                }
                OutputStatus::Cancelled(CancelMotive::CycleDetected) => {
                    "This result is incomplete. Due to a replacement cycle, the replacement was cancelled."
                }
                OutputStatus::Cancelled(CancelMotive::HighGrowth) => {
                    "This result is incomplete. The replacement was cancelled because it was growing too much."
                }
            };

            html_nested! {
                <ybc::Tile ctx={Child}><ybc::Field
                    label={"Result"}
                    {help}
                ><ybc::Control
                    tag={"div"}
                    classes={classes!(
                        match status {
                            OutputStatus::Outdated | OutputStatus::Done | OutputStatus::Cancelled(_) => {"has-icons-right"},
                            OutputStatus::InProgress => {"is-loading"}
                        }
                    )}
                ><ybc::TextArea
                    name={"replaced-text"}
                    value={active_text_project.output.clone()}
                    update={link.callback(move |value| Msg::OutputUpdated(active_text_project_index, value))}
                    placeholder={"The replaced text will be shown here.."}
                    readonly=true
                    rows=6
                    classes={classes!(
                        match status {
                            OutputStatus::Outdated => {"is-warning"},
                            OutputStatus::InProgress => {""}
                            OutputStatus::Done => {"is-success"},
                            OutputStatus::Cancelled(_) => {"is-danger"}
                        }
                    )}
                />
                if matches!(status, OutputStatus::Outdated | OutputStatus::Done) {
                    <span class="icon is-small is-right">
                        if matches!(status, OutputStatus::Done) {
                            <i class="fas fa-check"></i>
                        } else if matches!(status, OutputStatus::Outdated) {
                            <i class="fas fa-exclamation-triangle"></i>
                        }
                    </span>
                }
                </ybc::Control></ybc::Field></ybc::Tile>
            }
        } else {
            html_nested! {<ybc::Tile ctx={Child}></ybc::Tile>}
        };

        let toggle_replace_text = if self.replacement_in_progress {
            link.callback(move |_| Msg::CancelReplacingText())
        } else {
            link.callback(move |_| Msg::StartReplacingText(active_text_project_index))
        };
        let toggle_replacement = html_nested! {
            <ybc::Tile ctx={Child}><a onclick={toggle_replace_text}><ybc::Button>
                {
                    if self.replacement_in_progress {
                        "Cancel Replacing Text"
                    } else {
                        "Start Replacing Text"
                    }
                }
            </ybc::Button></a></ybc::Tile>

        };

        let body = html_nested! {
            <ybc::Tile ctx={Ancestor}>
                <ybc::Tile vertical=true>
                    <ybc::Tile
                        vertical=true
                        classes={classes!(
                            "px-6",
                            "py-3",
                            "has-background-info-light"
                        )}
                    >
                        {edit_steps}
                    </ybc::Tile>
                    <ybc::Tile vertical=true>
                        <ybc::Tile
                            ctx={Parent}
                            vertical=true
                            classes={classes!(
                                "pt-0"
                            )}
                        >
                            <ybc::Tile
                                ctx={Child}
                                classes={classes!(
                                    "has-background-info-light",
                                )}
                            >
                                {tabs}
                            </ybc::Tile>
                        </ybc::Tile>
                        <ybc::Tile
                            ctx={Parent}
                            vertical=true
                            classes={classes!(
                                "mx-6"
                            )}
                        >
                            {edit_project_title}
                            {input}
                            {toggle_replacement}
                            {output}
                        </ybc::Tile>
                    </ybc::Tile>
                </ybc::Tile>
            </ybc::Tile>
        };

        html! {<>
        {navbar}
        {body}
        </>
        }
    }
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    yew::start_app_with_props::<Model>(());
}
