use serde::Deserialize;
use std::fmt;

#[derive(Deserialize, Clone)]
pub struct HostConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

impl fmt::Debug for HostConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HostConfig")
            .field("name", &self.name)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_host_config_debug_then_password_should_be_redacted() {
        let host = HostConfig {
            name: "test".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 58846,
            username: "user".to_owned(),
            password: "secret".to_owned(),
        };

        let debug_output = format!("{host:?}");

        assert!(!debug_output.contains("secret"));
        assert!(debug_output.contains("<redacted>"));
    }
}
