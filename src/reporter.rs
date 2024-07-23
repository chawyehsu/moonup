use indicatif::ProgressBar;

pub struct ProgressReporter {
    progress_bar: ProgressBar,
}

impl ProgressReporter {
    pub fn new(prefix: String) -> Self {
        let progress_bar = ProgressBar::new(1).with_style(download_style());
        progress_bar.set_prefix(prefix);
        Self { progress_bar }
    }
}

impl Reporter for ProgressReporter {
    fn on_start(&self, total: usize) {
        self.progress_bar.set_length(total as u64);
        self.progress_bar.set_position(0);
    }

    fn on_progress(&self, current: usize) {
        self.progress_bar.set_position(current as u64);
    }

    fn on_complete(&self) {
        self.progress_bar.finish_and_clear();
    }
}

pub trait Reporter: Send + Sync {
    fn on_start(&self, total: usize);

    fn on_progress(&self, current: usize);

    fn on_complete(&self);
}

fn download_style() -> indicatif::ProgressStyle {
    indicatif::ProgressStyle::default_bar()
        .template("{spinner:.dim} {prefix:21!} [{elapsed_precise}] [{bar:20!}] {bytes:>8}")
        .unwrap()
        .progress_chars("#> ")
}
