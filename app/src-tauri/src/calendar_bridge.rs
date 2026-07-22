//! Wires `buddy-calendar`'s [`SettingsLookup`] to the SQLite settings store.

use std::sync::Arc;

use buddy_calendar::SettingsLookup;
use buddy_database::Database;

pub struct DbSettings {
    pub db: Arc<Database>,
}

impl SettingsLookup for DbSettings {
    fn get(&self, key: &str) -> Option<String> {
        self.db.get_setting(key).ok().flatten()
    }
}
