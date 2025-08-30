use strum_macros::{Display, EnumString};

#[derive(Debug, Display, PartialEq, EnumString)]
pub enum Methods {
    #[strum(serialize = "tlock_ping")]
    TlockPing,
    #[strum(serialize = "plugin_version")]
    PluginVersion,
    #[strum(serialize = "plugin_name")]
    PluginName,
}
