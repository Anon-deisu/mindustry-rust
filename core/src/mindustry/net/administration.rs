use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct Administration {
    pub banned_ips: Vec<String>,
    pub whitelist: Vec<String>,
    pub whitelist_enabled: bool,
    pub chat_filters: Vec<ChatFilter>,
    pub action_filters: Vec<ActionFilter>,
    pub subnet_bans: Vec<String>,
    pub dos_blacklist: HashSet<String>,
    pub kicked_ips: HashMap<String, i64>,
    pub banned_names: Vec<String>,
    pub player_info: HashMap<String, PlayerInfo>,
    pub configs: Vec<Config>,
}

impl Default for Administration {
    fn default() -> Self {
        Self {
            banned_ips: Vec::new(),
            whitelist: Vec::new(),
            whitelist_enabled: false,
            chat_filters: Vec::new(),
            action_filters: Vec::new(),
            subnet_bans: Vec::new(),
            dos_blacklist: HashSet::new(),
            kicked_ips: HashMap::new(),
            banned_names: Vec::new(),
            player_info: HashMap::new(),
            configs: Config::defaults(),
        }
    }
}

impl Administration {
    pub fn blacklist_dos(&mut self, address: impl Into<String>) {
        self.dos_blacklist.insert(address.into());
    }

    pub fn unblacklist_dos(&mut self, address: &str) {
        self.dos_blacklist.remove(address);
    }

    pub fn is_dos_blacklisted(&self, address: &str) -> bool {
        self.dos_blacklist.contains(address)
    }

    pub fn add_chat_filter<F>(&mut self, filter: F)
    where
        F: Fn(Option<&str>, &str) -> Option<String> + Send + Sync + 'static,
    {
        self.chat_filters.push(Arc::new(filter));
    }

    pub fn filter_message(&self, player: Option<&str>, message: &str) -> Option<String> {
        let mut current = message.to_string();
        for filter in &self.chat_filters {
            current = filter(player, &current)?;
        }
        Some(current)
    }

    pub fn add_action_filter<F>(&mut self, filter: F)
    where
        F: Fn(&PlayerAction) -> bool + Send + Sync + 'static,
    {
        self.action_filters.push(Arc::new(filter));
    }

    pub fn allow_action(&self, action: &PlayerAction) -> bool {
        if action.player.is_none() {
            return true;
        }

        self.action_filters.iter().all(|filter| filter(action))
    }

    pub fn add_subnet_ban(&mut self, ip_prefix: impl Into<String>) {
        self.subnet_bans.push(ip_prefix.into());
    }

    pub fn is_subnet_banned(&self, ip: &str) -> bool {
        self.subnet_bans.iter().any(|prefix| ip.starts_with(prefix))
    }

    pub fn handle_kicked(
        &mut self,
        uuid: impl Into<String>,
        ip: impl Into<String>,
        until_millis: i64,
    ) {
        let uuid = uuid.into();
        let ip = ip.into();
        self.kicked_ips
            .entry(ip)
            .and_modify(|existing| *existing = (*existing).max(until_millis))
            .or_insert(until_millis);
        let info = self.get_or_create_info(uuid);
        info.times_kicked += 1;
        info.last_kicked = info.last_kicked.max(until_millis);
    }

    pub fn get_kick_time(&self, uuid: &str, ip: &str) -> i64 {
        let info_kick = self
            .player_info
            .get(uuid)
            .map(|info| info.last_kicked)
            .unwrap_or_default();
        let ip_kick = self.kicked_ips.get(ip).copied().unwrap_or_default();
        info_kick.max(ip_kick)
    }

    pub fn is_recently_kicked(&self, uuid: &str, ip: &str, now_millis: i64) -> bool {
        self.get_kick_time(uuid, ip) > now_millis
    }

    pub fn update_player_joined(
        &mut self,
        id: impl Into<String>,
        ip: impl Into<String>,
        name: impl Into<String>,
    ) {
        let id = id.into();
        let ip = ip.into();
        let name = name.into();
        let info = self.get_or_create_info(id);
        info.last_name = name.clone();
        info.last_ip = ip.clone();
        info.times_joined += 1;
        push_unique(&mut info.names, name);
        push_unique(&mut info.ips, ip);
    }

    pub fn ban_player_ip(&mut self, ip: impl Into<String>) -> bool {
        let ip = ip.into();
        if self.banned_ips.contains(&ip) {
            return false;
        }
        for info in self.player_info.values_mut() {
            if info.ips.contains(&ip) {
                info.banned = true;
            }
        }
        self.banned_ips.push(ip);
        true
    }

    pub fn ban_player_id(&mut self, id: impl Into<String>) -> bool {
        let info = self.get_or_create_info(id.into());
        if info.banned {
            return false;
        }
        info.banned = true;
        true
    }

    pub fn unban_player_id(&mut self, id: impl Into<String>) -> bool {
        let info = self.get_or_create_info(id.into());
        if !info.banned {
            return false;
        }
        info.banned = false;
        let ips = info.ips.clone();
        self.banned_ips.retain(|ip| !ips.contains(ip));
        true
    }

    pub fn is_ip_banned(&self, ip: &str) -> bool {
        self.banned_ips.iter().any(|banned| banned == ip) || self.is_subnet_banned(ip)
    }

    pub fn is_id_banned(&self, id: &str) -> bool {
        self.player_info
            .get(id)
            .map(|info| info.banned)
            .unwrap_or(false)
    }

    pub fn ban_name_pattern(&mut self, pattern: impl Into<String>) -> bool {
        let pattern = pattern.into();
        if self
            .banned_names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&pattern))
        {
            return false;
        }
        self.banned_names.push(pattern);
        true
    }

    pub fn is_name_banned(&self, name: &str) -> bool {
        if self.banned_names.is_empty() {
            return false;
        }

        let name = name.to_ascii_lowercase();
        self.banned_names
            .iter()
            .any(|pattern| name.contains(&pattern.to_ascii_lowercase()))
    }

    pub fn get_admins(&self) -> Vec<PlayerInfo> {
        self.player_info
            .values()
            .filter(|info| info.admin)
            .cloned()
            .collect()
    }

    pub fn get_banned(&self) -> Vec<PlayerInfo> {
        self.player_info
            .values()
            .filter(|info| info.banned)
            .cloned()
            .collect()
    }

    pub fn get_banned_ips(&self) -> &[String] {
        &self.banned_ips
    }

    pub fn admin_player(&mut self, id: impl Into<String>, usid: impl Into<String>) -> bool {
        let info = self.get_or_create_info(id.into());
        let was_admin = info.admin;
        info.admin_usid = Some(usid.into());
        info.admin = true;
        was_admin
    }

    pub fn unadmin_player(&mut self, id: impl Into<String>) -> bool {
        let info = self.get_or_create_info(id.into());
        if !info.admin {
            return false;
        }

        info.admin = false;
        true
    }

    pub fn is_admin(&self, id: &str, usid: &str) -> bool {
        self.player_info.get(id).is_some_and(|info| {
            info.admin
                && info
                    .admin_usid
                    .as_deref()
                    .is_some_and(|admin_usid| admin_usid == usid)
        })
    }

    pub fn set_whitelist_enabled(&mut self, enabled: bool) {
        self.whitelist_enabled = enabled;
    }

    pub fn is_whitelist_enabled(&self) -> bool {
        self.whitelist_enabled
    }

    pub fn is_whitelisted(&self, id: &str, usid: &str) -> bool {
        !self.is_whitelist_enabled() || self.whitelist.contains(&Self::whitelist_key(id, usid))
    }

    pub fn whitelist(&mut self, id: impl Into<String>) -> bool {
        let id = id.into();
        let key = {
            let info = self.get_or_create_info(id.clone());
            Self::whitelist_key(&id, info.admin_usid.as_deref().unwrap_or("null"))
        };
        if self.whitelist.contains(&key) {
            return false;
        }

        self.whitelist.push(key);
        true
    }

    pub fn unwhitelist(&mut self, id: impl Into<String>) -> bool {
        let id = id.into();
        let key = {
            let info = self.get_or_create_info(id.clone());
            Self::whitelist_key(&id, info.admin_usid.as_deref().unwrap_or("null"))
        };
        if let Some(index) = self.whitelist.iter().position(|existing| existing == &key) {
            self.whitelist.remove(index);
            true
        } else {
            false
        }
    }

    pub fn get_whitelisted(&self) -> Vec<PlayerInfo> {
        self.player_info
            .values()
            .filter(|info| {
                self.is_whitelisted(&info.id, info.admin_usid.as_deref().unwrap_or("null"))
            })
            .cloned()
            .collect()
    }

    pub fn find_by_name(&self, name: &str) -> Vec<PlayerInfo> {
        self.player_info
            .values()
            .filter(|info| {
                info.last_name.eq_ignore_ascii_case(name)
                    || info.names.iter().any(|candidate| candidate == name)
                    || strip_colors(&strip_colors(&info.last_name)) == name
                    || info.ips.iter().any(|ip| ip == name)
                    || info.id == name
            })
            .cloned()
            .collect()
    }

    pub fn search_names(&self, name: &str) -> Vec<PlayerInfo> {
        let lower_name = name.to_ascii_lowercase();
        self.player_info
            .values()
            .filter(|info| {
                info.names.iter().any(|candidate| {
                    candidate.to_ascii_lowercase().contains(&lower_name)
                        || strip_colors(candidate)
                            .trim()
                            .to_ascii_lowercase()
                            .contains(&lower_name)
                })
            })
            .cloned()
            .collect()
    }

    pub fn find_by_ips(&self, ip: &str) -> Vec<PlayerInfo> {
        self.player_info
            .values()
            .filter(|info| info.ips.iter().any(|candidate| candidate == ip))
            .cloned()
            .collect()
    }

    pub fn find_by_ip(&self, ip: &str) -> Option<PlayerInfo> {
        self.player_info
            .values()
            .find(|info| info.ips.iter().any(|candidate| candidate == ip))
            .cloned()
    }

    pub fn get_info(&mut self, id: impl Into<String>) -> &mut PlayerInfo {
        self.get_or_create_info(id.into())
    }

    pub fn get_info_optional(&self, id: &str) -> Option<&PlayerInfo> {
        self.player_info.get(id)
    }

    pub fn get_or_create_info(&mut self, id: String) -> &mut PlayerInfo {
        self.player_info
            .entry(id.clone())
            .or_insert_with(|| PlayerInfo::new(id))
    }

    fn whitelist_key(id: &str, usid: &str) -> String {
        format!("{usid}{id}")
    }
}

impl fmt::Debug for Administration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Administration")
            .field("banned_ips", &self.banned_ips)
            .field("whitelist", &self.whitelist)
            .field("whitelist_enabled", &self.whitelist_enabled)
            .field("chat_filters", &self.chat_filters.len())
            .field("action_filters", &self.action_filters.len())
            .field("subnet_bans", &self.subnet_bans)
            .field("dos_blacklist", &self.dos_blacklist)
            .field("kicked_ips", &self.kicked_ips)
            .field("banned_names", &self.banned_names)
            .field("player_info", &self.player_info)
            .field("configs", &self.configs)
            .finish()
    }
}

impl PartialEq for Administration {
    fn eq(&self, other: &Self) -> bool {
        self.banned_ips == other.banned_ips
            && self.whitelist == other.whitelist
            && self.whitelist_enabled == other.whitelist_enabled
            && self.chat_filters.len() == other.chat_filters.len()
            && self.action_filters.len() == other.action_filters.len()
            && self.subnet_bans == other.subnet_bans
            && self.dos_blacklist == other.dos_blacklist
            && self.kicked_ips == other.kicked_ips
            && self.banned_names == other.banned_names
            && self.player_info == other.player_info
            && self.configs == other.configs
    }
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn strip_colors(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '[' {
            for next in chars.by_ref() {
                if next == ']' {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }

    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub name: String,
    pub key: String,
    pub description: String,
    pub default_value: ConfigValue,
}

impl Config {
    pub fn new(name: &str, description: &str, default_value: ConfigValue) -> Self {
        Self {
            name: name.into(),
            key: name.into(),
            description: description.into(),
            default_value,
        }
    }

    pub fn with_key(name: &str, description: &str, default_value: ConfigValue, key: &str) -> Self {
        Self {
            name: name.into(),
            key: key.into(),
            description: description.into(),
            default_value,
        }
    }

    pub fn defaults() -> Vec<Self> {
        vec![
            Self::with_key(
                "name",
                "The server name as displayed on clients.",
                ConfigValue::String("Server".into()),
                "servername",
            ),
            Self::new(
                "desc",
                "The server description, displayed under the name. Max 100 characters.",
                ConfigValue::String("off".into()),
            ),
            Self::new(
                "port",
                "The port to host on.",
                ConfigValue::Number(crate::mindustry::vars::DEFAULT_PORT as i32),
            ),
            Self::new(
                "autoUpdate",
                "Whether to auto-update and exit when a new bleeding-edge update arrives.",
                ConfigValue::Bool(false),
            ),
            Self::new(
                "showConnectMessages",
                "Whether to display connect/disconnect messages.",
                ConfigValue::Bool(true),
            ),
            Self::new(
                "enableVotekick",
                "Whether votekick is enabled.",
                ConfigValue::Bool(true),
            ),
            Self::new(
                "startCommands",
                "Commands run at startup. This should be a comma-separated list.",
                ConfigValue::String(String::new()),
            ),
            Self::new(
                "logging",
                "Whether to log everything to files.",
                ConfigValue::Bool(true),
            ),
            Self::new(
                "strict",
                "Whether strict mode is on.",
                ConfigValue::Bool(true),
            ),
            Self::new(
                "antiSpam",
                "Whether spammers are automatically kicked and rate-limited.",
                ConfigValue::Bool(true),
            ),
            Self::with_key(
                "allowCustomClients",
                "Whether custom clients are allowed to connect.",
                ConfigValue::Bool(false),
                "allow-custom",
            ),
            Self::new(
                "whitelist",
                "Whether the whitelist is used.",
                ConfigValue::Bool(false),
            ),
            Self::new(
                "motd",
                "The message displayed to people on connection.",
                ConfigValue::String("off".into()),
            ),
            Self::new(
                "autosave",
                "Whether the periodically save the map when playing.",
                ConfigValue::Bool(false),
            ),
            Self::new(
                "snapshotInterval",
                "Client entity snapshot interval in ms.",
                ConfigValue::Number(200),
            ),
            Self::new(
                "roundExtraTime",
                "Time before loading a new map after gameover, in seconds.",
                ConfigValue::Number(12),
            ),
            Self::new(
                "logCommands",
                "Whether player commands should be logged.",
                ConfigValue::Bool(true),
            ),
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    String(String),
    Bool(bool),
    Number(i32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerInfo {
    pub id: String,
    pub last_name: String,
    pub last_ip: String,
    pub ips: Vec<String>,
    pub names: Vec<String>,
    pub admin_usid: Option<String>,
    pub times_kicked: i32,
    pub times_joined: i32,
    pub banned: bool,
    pub admin: bool,
    pub last_kicked: i64,
    pub last_message_time: i64,
    pub last_sync_time: i64,
    pub last_sent_message: Option<String>,
    pub message_infractions: i32,
}

impl PlayerInfo {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            last_name: "<unknown>".into(),
            last_ip: "<unknown>".into(),
            ips: Vec::new(),
            names: Vec::new(),
            admin_usid: None,
            times_kicked: 0,
            times_joined: 0,
            banned: false,
            admin: false,
            last_kicked: 0,
            last_message_time: 0,
            last_sync_time: 0,
            last_sent_message: None,
            message_infractions: 0,
        }
    }

    pub fn plain_last_name(&self) -> String {
        strip_colors(&self.last_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceInfo {
    pub ip: Option<String>,
    pub uuid: Option<String>,
    pub locale: Option<String>,
    pub modded: bool,
    pub mobile: bool,
    pub times_joined: i32,
    pub times_kicked: i32,
    pub ips: Vec<Option<String>>,
    pub names: Vec<Option<String>>,
}

impl TraceInfo {
    pub const MAX_HISTORY_LEN: usize = 12;

    pub fn new(
        ip: Option<String>,
        uuid: Option<String>,
        locale: Option<String>,
        modded: bool,
        mobile: bool,
        times_joined: i32,
        times_kicked: i32,
        ips: Vec<Option<String>>,
        names: Vec<Option<String>>,
    ) -> Self {
        Self {
            ip,
            uuid,
            locale,
            modded,
            mobile,
            times_joined,
            times_kicked,
            ips,
            names,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SteamAdminData {
    pub bans: HashSet<String>,
    pub admins: HashSet<String>,
}

impl SteamAdminData {
    pub fn from_lists(
        bans: impl IntoIterator<Item = impl Into<String>>,
        admins: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            bans: bans.into_iter().map(Into::into).collect(),
            admins: admins.into_iter().map(Into::into).collect(),
        }
    }

    pub fn from_json_text(text: &str) -> Result<Self, SteamAdminParseError> {
        Ok(Self::from_lists(
            parse_json_string_array(text, "bans")?,
            parse_json_string_array(text, "admins")?,
        ))
    }

    pub fn is_banned(&self, id: &str) -> bool {
        steam_id(id).is_some_and(|id| self.bans.contains(id))
    }

    pub fn is_admin(&self, id: &str) -> bool {
        steam_id(id).is_some_and(|id| self.admins.contains(id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SteamAdminParseError {
    MissingArray(String),
    UnterminatedArray(String),
    UnterminatedString(String),
}

fn steam_id(id: &str) -> Option<&str> {
    id.strip_prefix("steam:")
}

fn parse_json_string_array(text: &str, key: &str) -> Result<Vec<String>, SteamAdminParseError> {
    let quoted_key = format!("\"{key}\"");
    let Some(key_index) = text.find(&quoted_key) else {
        return Err(SteamAdminParseError::MissingArray(key.to_string()));
    };
    let Some(array_start) = text[key_index + quoted_key.len()..].find('[') else {
        return Err(SteamAdminParseError::MissingArray(key.to_string()));
    };
    let mut chars = text[key_index + quoted_key.len() + array_start + 1..]
        .chars()
        .peekable();
    let mut values = Vec::new();

    loop {
        while matches!(chars.peek(), Some(ch) if ch.is_whitespace() || *ch == ',') {
            chars.next();
        }

        match chars.peek() {
            Some(']') => {
                chars.next();
                return Ok(values);
            }
            Some('"') => {
                chars.next();
                let mut value = String::new();
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some('"') => value.push('"'),
                            Some('\\') => value.push('\\'),
                            Some('/') => value.push('/'),
                            Some('n') => value.push('\n'),
                            Some('r') => value.push('\r'),
                            Some('t') => value.push('\t'),
                            Some(other) => value.push(other),
                            None => {
                                return Err(SteamAdminParseError::UnterminatedString(
                                    key.to_string(),
                                ))
                            }
                        },
                        Some(ch) => value.push(ch),
                        None => {
                            return Err(SteamAdminParseError::UnterminatedString(key.to_string()))
                        }
                    }
                }
                values.push(value);
            }
            Some(_) => {
                chars.next();
            }
            None => return Err(SteamAdminParseError::UnterminatedArray(key.to_string())),
        }
    }
}

pub type ChatFilter = Arc<dyn Fn(Option<&str>, &str) -> Option<String> + Send + Sync>;
pub type ActionFilter = Arc<dyn Fn(&PlayerAction) -> bool + Send + Sync>;

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerAction {
    pub player: Option<String>,
    pub action_type: ActionType,
    pub tile: Option<i32>,
    pub block: Option<String>,
    pub rotation: i32,
    pub config: Option<String>,
    pub item: Option<String>,
    pub item_amount: i32,
    pub unit: Option<String>,
    pub payload: Option<String>,
    pub plans: Option<Vec<i32>>,
    pub unit_ids: Option<Vec<i32>>,
    pub building_positions: Option<Vec<i32>>,
    pub ping_text: Option<String>,
    pub ping_x: f32,
    pub ping_y: f32,
}

impl PlayerAction {
    pub fn new(player: Option<String>, action_type: ActionType) -> Self {
        Self {
            player,
            action_type,
            tile: None,
            block: None,
            rotation: 0,
            config: None,
            item: None,
            item_amount: 0,
            unit: None,
            payload: None,
            plans: None,
            unit_ids: None,
            building_positions: None,
            ping_text: None,
            ping_x: 0.0,
            ping_y: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    BreakBlock,
    PlaceBlock,
    Rotate,
    Configure,
    WithdrawItem,
    DepositItem,
    Control,
    BuildSelect,
    Command,
    RemovePlanned,
    CommandUnits,
    CommandBuilding,
    Respawn,
    PickupBlock,
    DropPayload,
    PingLocation,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ban_ip_also_marks_matching_player_info() {
        let mut admin = Administration::default();
        admin.update_player_joined("uuid", "1.2.3.4", "name");
        assert!(admin.ban_player_ip("1.2.3.4"));
        assert!(admin.is_ip_banned("1.2.3.4"));
        assert!(admin.is_id_banned("uuid"));
    }

    #[test]
    fn unban_player_id_clears_id_and_used_ips_like_java() {
        let mut admin = Administration::default();
        admin.update_player_joined("uuid", "1.2.3.4", "name");
        admin.update_player_joined("uuid", "5.6.7.8", "name");
        assert!(admin.ban_player_ip("1.2.3.4"));
        assert!(admin.ban_player_ip("5.6.7.8"));
        assert!(admin.is_id_banned("uuid"));

        assert!(admin.unban_player_id("uuid"));
        assert!(!admin.is_id_banned("uuid"));
        assert!(!admin.is_ip_banned("1.2.3.4"));
        assert!(!admin.is_ip_banned("5.6.7.8"));
        assert!(!admin.unban_player_id("uuid"));
    }

    #[test]
    fn admin_player_tracks_usid_like_java_access_control() {
        let mut admin = Administration::default();

        assert!(!admin.admin_player("uuid", "usid-a"));
        assert!(admin.is_admin("uuid", "usid-a"));
        assert!(!admin.is_admin("uuid", "usid-b"));
        assert!(admin.admin_player("uuid", "usid-b"));
        assert!(!admin.is_admin("uuid", "usid-a"));
        assert!(admin.is_admin("uuid", "usid-b"));

        assert!(admin.unadmin_player("uuid"));
        assert!(!admin.unadmin_player("uuid"));
        assert!(!admin.is_admin("uuid", "usid-b"));
    }

    #[test]
    fn kick_time_and_name_bans_feed_java_connection_guards() {
        let mut admin = Administration::default();
        admin.handle_kicked("uuid", "1.2.3.4", 2_000);

        assert_eq!(admin.get_kick_time("uuid", "1.2.3.4"), 2_000);
        assert!(admin.is_recently_kicked("uuid", "1.2.3.4", 1_999));
        assert!(!admin.is_recently_kicked("uuid", "1.2.3.4", 2_000));

        assert!(admin.ban_name_pattern("blocked"));
        assert!(!admin.ban_name_pattern("BLOCKED"));
        assert!(admin.is_name_banned("my Blocked player"));
        assert!(!admin.is_name_banned("ordinary"));
    }

    #[test]
    fn whitelist_uses_admin_usid_plus_id_keys_like_java() {
        let mut admin = Administration::default();
        admin.update_player_joined("uuid", "1.2.3.4", "name");

        assert!(!admin.is_whitelist_enabled());
        assert!(admin.is_whitelisted("uuid", "missing"));
        admin.set_whitelist_enabled(true);
        assert!(!admin.is_whitelisted("uuid", "usid"));

        admin.admin_player("uuid", "usid");
        assert!(admin.whitelist("uuid"));
        assert!(!admin.whitelist("uuid"));
        assert_eq!(admin.whitelist, vec!["usiduuid"]);
        assert!(admin.is_whitelisted("uuid", "usid"));
        assert!(!admin.is_whitelisted("uuid", "other"));

        let listed = admin.get_whitelisted();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "uuid");

        assert!(admin.unwhitelist("uuid"));
        assert!(!admin.unwhitelist("uuid"));
        assert!(!admin.is_whitelisted("uuid", "usid"));
    }

    #[test]
    fn chat_filters_chain_and_can_suppress_messages_like_java() {
        let mut admin = Administration::default();
        admin.add_chat_filter(|_player, message| Some(message.replace("foo", "bar")));
        admin.add_chat_filter(|_player, message| Some(format!("{message}!")));

        assert_eq!(
            admin.filter_message(Some("player"), "foo").as_deref(),
            Some("bar!")
        );

        admin.add_chat_filter(|_player, message| {
            if message.contains("blocked") {
                None
            } else {
                Some(message.to_string())
            }
        });
        assert_eq!(admin.filter_message(Some("player"), "blocked"), None);
    }

    #[test]
    fn action_filters_allow_server_actions_and_short_circuit_player_actions() {
        let mut admin = Administration::default();
        admin.add_action_filter(|action| action.action_type != ActionType::Configure);
        admin.add_action_filter(|action| action.block.as_deref() != Some("forbidden"));

        let mut configure = PlayerAction::new(Some("player".into()), ActionType::Configure);
        configure.block = Some("router".into());
        assert!(!admin.allow_action(&configure));

        let mut place = PlayerAction::new(Some("player".into()), ActionType::PlaceBlock);
        place.block = Some("forbidden".into());
        assert!(!admin.allow_action(&place));

        let server_action = PlayerAction::new(None, ActionType::Configure);
        assert!(admin.allow_action(&server_action));
    }

    #[test]
    fn player_info_queries_match_java_admin_ban_and_ip_helpers() {
        let mut admin = Administration::default();
        admin.update_player_joined("uuid-a", "1.2.3.4", "Alice");
        admin.update_player_joined("uuid-b", "5.6.7.8", "Bob");
        admin.admin_player("uuid-a", "usid-a");
        admin.ban_player_id("uuid-b");
        admin.ban_player_ip("9.9.9.9");

        assert_eq!(admin.get_admins().len(), 1);
        assert_eq!(admin.get_admins()[0].id, "uuid-a");
        assert_eq!(admin.get_banned().len(), 1);
        assert_eq!(admin.get_banned()[0].id, "uuid-b");
        assert_eq!(admin.get_banned_ips(), &["9.9.9.9".to_string()]);
        assert_eq!(admin.find_by_ip("5.6.7.8").unwrap().id, "uuid-b");
        assert_eq!(admin.find_by_ips("1.2.3.4")[0].id, "uuid-a");
        assert!(admin.get_info_optional("missing").is_none());
        assert_eq!(admin.get_info("created").id, "created");
    }

    #[test]
    fn player_info_name_search_matches_java_exact_and_contains_shapes() {
        let mut admin = Administration::default();
        admin.update_player_joined("uuid-a", "1.2.3.4", "[scarlet]Alice");
        admin.update_player_joined("uuid-b", "5.6.7.8", "builder");
        admin.get_info("uuid-a").names.push("[accent]Alicia".into());

        assert_eq!(admin.find_by_name("Alice")[0].id, "uuid-a");
        assert_eq!(admin.find_by_name("1.2.3.4")[0].id, "uuid-a");
        assert_eq!(admin.find_by_name("uuid-b")[0].id, "uuid-b");
        assert_eq!(admin.search_names("lic")[0].id, "uuid-a");
        assert_eq!(
            admin.get_info_optional("uuid-a").unwrap().plain_last_name(),
            "Alice"
        );
    }

    #[test]
    fn steam_admin_data_only_matches_prefixed_steam_ids() {
        let data = SteamAdminData::from_lists(["111", "222"], ["333"]);

        assert!(data.is_banned("steam:111"));
        assert!(data.is_banned("steam:222"));
        assert!(!data.is_banned("111"));
        assert!(!data.is_banned("steam:333"));
        assert!(data.is_admin("steam:333"));
        assert!(!data.is_admin("333"));
    }

    #[test]
    fn steam_admin_data_parses_mindustry_bans_json_shape() {
        let text = r#"{
            "bans": ["111", "222"],
            "admins": ["333", "escaped\"id"]
        }"#;
        let data = SteamAdminData::from_json_text(text).unwrap();

        assert!(data.is_banned("steam:111"));
        assert!(data.is_banned("steam:222"));
        assert!(data.is_admin("steam:333"));
        assert!(data.is_admin("steam:escaped\"id"));
        assert_eq!(
            SteamAdminData::from_json_text(r#"{"bans":[]}"#).unwrap_err(),
            SteamAdminParseError::MissingArray("admins".into())
        );
    }
}
