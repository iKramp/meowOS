use std::{string::String, vec::Vec};

#[derive(Debug)]
pub struct CmdArgs {
    pub init_commands: Vec<String>,
}

impl CmdArgs {
    pub fn new(arg_str: &str) -> Self {
        let splitter = CmdArgsSplitter::new(arg_str);
        let mut init_commands = Vec::new();
        for (key, value) in splitter {
            if key == "init" {
                init_commands.push(value.replace("\\ ", " ").replace("\\\\", "\\"));
            }
        }
        Self { init_commands }
    }
}

struct CmdArgsSplitter<'a> {
    remaining: &'a str,
}

impl<'a> CmdArgsSplitter<'a> {
    pub fn new(arg_str: &'a str) -> Self {
        Self { remaining: arg_str }
    }
}

impl<'a> Iterator for CmdArgsSplitter<'a> {
    type Item = (&'a str, &'a str); // (key, value)

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }

        let next_equal_pos = self.remaining.find('=')?;
        let key = &self.remaining[..next_equal_pos];
        let mut search_start = next_equal_pos + 1;
        let initial_search_start = search_start;
        loop {
            let next_space_pos = self.remaining[search_start..]
                .find(' ')
                .unwrap_or(self.remaining.len() - search_start)
                + search_start;
            let prev_char = if next_space_pos > 0 {
                self.remaining.as_bytes()[next_space_pos - 1] as char
            } else {
                ' '
            };
            if prev_char != '\\' {
                let value = &self.remaining[initial_search_start..next_space_pos];
                self.remaining = if next_space_pos < self.remaining.len() {
                    &self.remaining[next_space_pos + 1..]
                } else {
                    ""
                };
                return Some((key, value));
            } else {
                search_start = next_space_pos + 1;
            }
        }
    }
}
