use std::fmt::Display;
use miette::Result;
use crate::console_backends::{LogBackend, TerminalBackend};

pub struct BareConsoleBackend {}

impl BareConsoleBackend {
    pub fn new() -> Self {
        Self {
        
        }
    }
}

impl TerminalBackend for BareConsoleBackend {
    fn setup(&mut self) -> Result<()> {
        Ok(())
    }
    
    fn destroy(self) -> Result<()> {
        Ok(())
    }
}

impl LogBackend for BareConsoleBackend {
    fn log_newline(&mut self) {
        println!();
    }
    
    fn log_println<T: Display>(&mut self, content: T) {
        println!("{}", content)
    }
}
