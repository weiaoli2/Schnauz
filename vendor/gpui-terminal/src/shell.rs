//! OSC 7 working-directory and OSC 133 prompt boundaries, including split reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellEvent {
    Focused,
    SshIntegration(String),
    WorkingDirectory(String),
    PromptReady,
    CommandStarted,
    DirectoryChangeFailed,
}

#[derive(Default)]
pub struct ShellParser {
    state: u8,
    payload: Vec<u8>,
}

impl ShellParser {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<ShellEvent> {
        let mut events = Vec::new();
        for &byte in bytes {
            match (self.state, byte) {
                (0, 0x1b) => self.state = 1,
                (1, b']') => {
                    self.state = 2;
                    self.payload.clear();
                }
                (1, _) => self.state = 0,
                (2, 7) | (3, b'\\') => {
                    if let Ok(payload) = std::str::from_utf8(&self.payload) {
                        if let Some(event) = payload.strip_prefix("777;schnauz-ssh;") {
                            events.push(ShellEvent::SshIntegration(event.to_owned()));
                        } else if let Some(url) = payload.strip_prefix("7;") {
                            events.push(ShellEvent::WorkingDirectory(url.to_owned()));
                        } else if payload == "133;A" {
                            events.push(ShellEvent::PromptReady);
                        } else if payload == "133;D;1" {
                            events.push(ShellEvent::DirectoryChangeFailed);
                        } else if payload == "133;C" {
                            events.push(ShellEvent::CommandStarted);
                        }
                    }
                    self.state = 0;
                    self.payload.clear();
                }
                (2, 0x1b) => self.state = 3,
                (2, _) if self.payload.len() < 16384 => self.payload.push(byte),
                (3, _) => {
                    self.state = 0;
                    self.payload.clear();
                }
                (2, _) => {
                    self.state = 0;
                    self.payload.clear();
                }
                _ => {}
            }
        }
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragmented_shell_events_survive_every_byte_boundary() {
        let bytes = b"prompt\x1b]7;file://localhost/tmp/My%20Folder\x1b\\\x1b]133;A\x07command\x1b]133;C\x07\x1b]777;schnauz-ssh;ready;schnauz-ssh.ABC;2f746d70\x07";
        let expected = vec![
            ShellEvent::WorkingDirectory("file://localhost/tmp/My%20Folder".into()),
            ShellEvent::PromptReady,
            ShellEvent::CommandStarted,
            ShellEvent::SshIntegration("ready;schnauz-ssh.ABC;2f746d70".into()),
        ];
        for split in 0..bytes.len() {
            let mut parser = ShellParser::default();
            let mut events = parser.feed(&bytes[..split]);
            events.extend(parser.feed(&bytes[split..]));
            assert_eq!(events, expected);
        }
    }
}
