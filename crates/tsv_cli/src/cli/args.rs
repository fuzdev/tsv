/// Hand-rolled argument parser with builder pattern
#[derive(Debug)]
pub struct Args {
    args: Vec<String>,
    pos: usize, // Current position for positional args
}

impl Args {
    pub fn new(args: Vec<String>) -> Self {
        Self { args, pos: 0 }
    }

    /// Check if a flag is present (e.g., "--pretty")
    pub fn flag(&self, name: &str) -> bool {
        let flag = format!("--{}", name);
        self.args.iter().any(|arg| arg == &flag)
    }

    /// Get value for an option (e.g., "--parser svelte" returns Some("svelte"))
    pub fn option(&self, name: &str) -> Option<String> {
        let flag = format!("--{}", name);
        self.args
            .iter()
            .position(|arg| arg == &flag)
            .and_then(|pos| self.args.get(pos + 1).cloned())
    }

    /// Get required option or return error
    pub fn required_option(&self, name: &str) -> Result<String, String> {
        self.option(name)
            .ok_or_else(|| format!("Missing required option: --{}", name))
    }

    /// Get next positional argument
    pub fn positional(&mut self) -> Option<String> {
        // Skip flags and their values
        while self.pos < self.args.len() {
            let arg = &self.args[self.pos];
            if arg.starts_with("--") {
                self.pos += 1;
                // Skip the value if this is an option (not a standalone flag)
                if self.pos < self.args.len() && !self.args[self.pos].starts_with("--") {
                    self.pos += 1;
                }
            } else {
                let result = arg.clone();
                self.pos += 1;
                return Some(result);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flag() {
        let args = Args::new(vec!["parse".into(), "--pretty".into()]);
        assert!(args.flag("pretty"));
        assert!(!args.flag("verbose"));
    }

    #[test]
    fn test_option() {
        let args = Args::new(vec!["format".into(), "--parser".into(), "svelte".into()]);
        assert_eq!(args.option("parser"), Some("svelte".into()));
        assert_eq!(args.option("missing"), None);
    }

    #[test]
    fn test_positional() {
        let mut args = Args::new(vec!["parse".into(), "file.ts".into(), "--pretty".into()]);
        assert_eq!(args.positional(), Some("parse".into()));
        assert_eq!(args.positional(), Some("file.ts".into()));
        assert_eq!(args.positional(), None);
    }

    #[test]
    fn test_mixed() {
        let mut args = Args::new(vec![
            "format".into(),
            "input.ts".into(),
            "--parser".into(),
            "typescript".into(),
            "--verbose".into(),
        ]);
        assert_eq!(args.positional(), Some("format".into()));
        assert_eq!(args.positional(), Some("input.ts".into()));
        assert_eq!(args.option("parser"), Some("typescript".into()));
        assert!(args.flag("verbose"));
    }
}
