pub struct TextProjectProps {
    pub title: String,
    pub commentary: Option<String>,
}

impl Default for TextProjectProps {
    fn default() -> Self {
        Self {
            title: "".into(),
            commentary: None,
        }
    }
}

#[derive(Default)]
pub struct TextProject {
    pub props: TextProjectProps,
    pub input: String,
    pub output: String,
    pub output_status: OutputStatus,
}

#[derive(Debug)]
pub enum OutputStatus {
    Outdated,
    InProgress,
    Done,
    Cancelled(CancelMotive),
}

#[derive(Debug)]
pub enum CancelMotive {
    ManuallyCancelled,
    CycleDetected,
    HighGrowth,
}

impl Default for OutputStatus {
    fn default() -> Self {
        OutputStatus::Done
    }
}
