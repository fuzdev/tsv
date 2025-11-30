/// Hand-rolled argument parser with builder pattern
#[derive(Debug)]
pub struct Args {
    args: Vec<String>,
    pos: usize, // Current position for positional args
    boolean_flags: std::collections::HashSet<String>, // Track boolean flags
}

impl Args {
    pub fn new(args: Vec<String>) -> Self {
        Self {
            args,
            pos: 0,
            boolean_flags: std::collections::HashSet::new(),
        }
    }

    /// Check if a flag is present (e.g., "--pretty")
    pub fn flag(&mut self, name: &str) -> bool {
        let flag = format!("--{name}");
        let is_present = self.args.iter().any(|arg| arg == &flag);
        if is_present {
            // Track this as a boolean flag
            self.boolean_flags.insert(flag);
        }
        is_present
    }

    /// Get value for an option (e.g., "--parser svelte" returns Some("svelte"))
    pub fn option(&self, name: &str) -> Option<String> {
        let flag = format!("--{name}");
        self.args
            .iter()
            .position(|arg| arg == &flag)
            .and_then(|pos| self.args.get(pos + 1).cloned())
    }

    /// Get required option or return error
    pub fn required_option(&self, name: &str) -> Result<String, String> {
        self.option(name)
            .ok_or_else(|| format!("Missing required option: --{name}"))
    }

    /// Get next positional argument
    pub fn positional(&mut self) -> Option<String> {
        // Skip flags and their values
        while self.pos < self.args.len() {
            let arg = &self.args[self.pos];
            if arg.starts_with("--") {
                // This is a flag - skip it
                let is_boolean = self.boolean_flags.contains(arg);
                self.pos += 1;

                // Skip the value if this is an option (not a boolean flag)
                if !is_boolean
                    && self.pos < self.args.len()
                    && !self.args[self.pos].starts_with("--")
                {
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
        let mut args = Args::new(vec!["parse".into(), "--pretty".into()]);
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
            "file.ts".into(),
            "--parser".into(),
            "typescript".into(),
            "--verbose".into(),
        ]);
        // Check flags first (this is important for the new behavior)
        assert!(args.flag("verbose"));
        assert_eq!(args.option("parser"), Some("typescript".into()));
        // Then collect positionals
        assert_eq!(args.positional(), Some("format".into()));
        assert_eq!(args.positional(), Some("file.ts".into()));
    }

    #[test]
    fn test_boolean_flag_with_positionals() {
        let mut args = Args::new(vec!["--list".into(), "unicode_6_digits".into()]);
        // Check the boolean flag first
        assert!(args.flag("list"));
        // Then collect positionals - should get the filter, not skip it as a flag value
        assert_eq!(args.positional(), Some("unicode_6_digits".into()));
        assert_eq!(args.positional(), None);
    }
}
