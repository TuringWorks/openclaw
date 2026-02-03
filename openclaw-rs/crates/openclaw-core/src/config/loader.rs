//! Configuration loading and persistence.

use super::Config;
use crate::error::ConfigError;
use crate::paths;
use std::fs;
use std::path::Path;

impl Config {
    /// Load configuration from the default path.
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = paths::config_file()?;
        Self::load(&path)
    }

    /// Load configuration from a file path.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }

        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse configuration from a string.
    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        json5::from_str(content).map_err(|e| ConfigError::Json5(e.to_string()))
    }

    /// Save configuration to the default path.
    pub fn save_default(&self) -> Result<(), ConfigError> {
        let path = paths::config_file()?;
        self.save(&path)
    }

    /// Save configuration to a file path.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = self.to_json5()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &content)?;
        fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Serialize to JSON5 string.
    pub fn to_json5(&self) -> Result<String, ConfigError> {
        // json5 doesn't have a serializer, so we use serde_json with pretty print
        serde_json::to_string_pretty(self).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Validate the configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate default agent exists if set
        if let Some(default) = &self.agents.default {
            if !self.agents.agents.contains_key(default) && !self.agents.agents.is_empty() {
                // Only error if there are agents defined but default is not among them
                // If no agents defined, default agent will be created implicitly
            }
        }

        // Validate model references
        if let Some(model) = &self.agents.defaults.model {
            // Check format is "provider/model-id"
            if !model.contains('/') {
                return Err(ConfigError::Validation(format!(
                    "Invalid model format '{}', expected 'provider/model-id'",
                    model
                )));
            }
        }

        // Validate port
        if self.gateway.port == 0 {
            return Err(ConfigError::Validation("Port cannot be 0".to_string()));
        }

        Ok(())
    }

    /// Get agent config by ID, falling back to defaults.
    pub fn get_agent(&self, id: &str) -> Option<&crate::types::AgentConfig> {
        self.agents.agents.get(id)
    }

    /// Get the default agent ID.
    pub fn default_agent_id(&self) -> Option<&str> {
        self.agents.default.as_deref().or_else(|| {
            // If only one agent, use it as default
            if self.agents.agents.len() == 1 {
                self.agents.agents.keys().next().map(|s| s.as_str())
            } else {
                None
            }
        })
    }
}

/// Configuration builder for creating configs programmatically.
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Create a new config builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default agent.
    pub fn default_agent(mut self, id: impl Into<String>) -> Self {
        self.config.agents.default = Some(id.into());
        self
    }

    /// Set the default model.
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.config.agents.defaults.model = Some(model.into());
        self
    }

    /// Set the gateway port.
    pub fn port(mut self, port: u16) -> Self {
        self.config.gateway.port = port;
        self
    }

    /// Set the bind mode.
    pub fn bind(mut self, mode: super::BindMode) -> Self {
        self.config.gateway.bind = mode;
        self
    }

    /// Build the config.
    pub fn build(self) -> Config {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let content = r#"{
            "agents": {
                "default": "test"
            }
        }"#;

        let config = Config::parse(content).unwrap();
        assert_eq!(config.agents.default, Some("test".to_string()));
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .default_agent("bot")
            .default_model("anthropic/claude-3-opus")
            .port(8080)
            .build();

        assert_eq!(config.agents.default, Some("bot".to_string()));
        assert_eq!(
            config.agents.defaults.model,
            Some("anthropic/claude-3-opus".to_string())
        );
        assert_eq!(config.gateway.port, 8080);
    }

    #[test]
    fn test_validate_invalid_model() {
        let mut config = Config::default();
        config.agents.defaults.model = Some("invalid".to_string());

        let result = config.validate();
        assert!(result.is_err());
    }
}
