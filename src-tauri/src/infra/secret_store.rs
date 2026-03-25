use keyring::{Entry, Error as KeyringError};

use crate::app::errors::{AppError, AppResult};

#[derive(Debug, Clone)]
pub struct SecretStore {
    service_name: String,
}

impl SecretStore {
    pub fn new(service_name: String) -> Self {
        Self { service_name }
    }

    pub fn get(&self, alias: &str) -> AppResult<Option<String>> {
        let entry = Entry::new(&self.service_name, alias)
            .map_err(|error| AppError::Secrets(error.to_string()))?;

        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(error) => Err(AppError::Secrets(error.to_string())),
        }
    }

    pub fn set(&self, alias: &str, value: &str) -> AppResult<()> {
        let entry = Entry::new(&self.service_name, alias)
            .map_err(|error| AppError::Secrets(error.to_string()))?;

        entry.set_password(value)
            .map_err(|error| AppError::Secrets(error.to_string()))
    }

    pub fn delete(&self, alias: &str) -> AppResult<()> {
        let entry = Entry::new(&self.service_name, alias)
            .map_err(|error| AppError::Secrets(error.to_string()))?;

        match entry.delete_credential() {
            Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
            Err(error) => Err(AppError::Secrets(error.to_string())),
        }
    }
}
