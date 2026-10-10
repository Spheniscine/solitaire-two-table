use serde::{Deserialize, Serialize};

use crate::game::Skin;

#[derive(Clone, Serialize, Deserialize)]
pub struct SettingsState {
    pub allow_undo: bool,
    pub auto_play: bool,
    pub skin: Skin,
}