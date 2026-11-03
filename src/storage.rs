use crate::theme::ThemeChoice;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    future::Future,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

pub const MIN_SOUND_BUTTON_SCALE: u16 = 50;
pub const MAX_SOUND_BUTTON_SCALE: u16 = 200;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub theme: ThemeChoice,
    pub remote_volume: u8,
    pub local_volume: u8,
    pub volumes_linked: bool,
    pub sound_button_scale: u16,
    pub minimize_to_tray: bool,
    pub recordings: crate::recording::RecordingSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::default(),
            remote_volume: 100,
            local_volume: 100,
            volumes_linked: false,
            sound_button_scale: 100,
            minimize_to_tray: false,
            recordings: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Button {
    pub id: String,
    pub name: Option<String>,
    pub full_path: Option<PathBuf>,
    pub start_ms: Option<u64>,
    pub end_ms: Option<u64>,
    pub color: Option<String>,
    pub relative_volume: Option<f32>,
    #[serde(default)]
    pub overlap: bool,
    pub hotkey: Option<String>,
    pub sorting: u64,
}

impl Default for Button {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: None,
            full_path: None,
            start_ms: None,
            end_ms: None,
            color: None,
            relative_volume: None,
            overlap: false,
            hotkey: None,
            sorting: 1,
        }
    }
}

impl Button {
    pub fn display_name(&self) -> &str {
        self.name
            .as_deref()
            .filter(|name| !name.trim().is_empty())
            .or_else(|| self.full_path.as_ref()?.file_stem()?.to_str())
            .unwrap_or("Choose sound")
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Category {
    pub id: String,
    pub category_name: Option<String>,
    pub entries: Vec<Button>,
}

impl Default for Category {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            category_name: None,
            entries: Vec::new(),
        }
    }
}

impl Category {
    fn filename(&self) -> String {
        if self.category_name.is_none() {
            "default.json".into()
        } else {
            format!("{}.json", self.id)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub settings: Settings,
    pub categories: Vec<Category>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            settings: Settings::default(),
            categories: vec![Category {
                entries: vec![Button::default()],
                ..Category::default()
            }],
        }
    }
}

impl State {
    pub fn ordered_buttons(&self) -> Vec<Button> {
        self.categories
            .iter()
            .flat_map(|category| {
                let mut entries = category.entries.clone();
                entries.sort_by_key(|entry| entry.sorting);
                entries
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn in_home() -> Result<Self, String> {
        std::env::home_dir()
            .map(|home| Self::new(home.join(".raspiboard")))
            .ok_or_else(|| "Cannot resolve the user's home directory".into())
    }

    pub fn recordings_directory(&self) -> PathBuf {
        self.root.join("recordings")
    }

    pub fn logs_directory(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// Keep the returned file open for the application's lifetime; never remove it.
    pub fn try_lock(&self) -> Result<Option<fs::File>, String> {
        fs::create_dir_all(&self.root)
            .map_err(|error| format!("{}: {error}", self.root.display()))?;
        let path = self.root.join("instance.lock");
        let file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(file)),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(error)) => Err(format!("{}: {error}", path.display())),
        }
    }

    pub fn load_or_create(&self) -> Result<State, String> {
        let settings_path = self.root.join("settings.toml");
        let settings_text = read_optional(&settings_path)?;
        let settings = match settings_text.as_deref() {
            Some(text) => toml::from_str::<Settings>(text)
                .map_err(|error| format!("{}: {error}", settings_path.display()))?,
            None => Settings::default(),
        };
        validate_settings(&settings)
            .map_err(|error| format!("{}: {error}", settings_path.display()))?;

        let mut categories = self.load_categories()?;
        let default = if categories
            .iter()
            .any(|category| category.category_name.is_none())
        {
            None
        } else {
            let mut category = Category::default();
            if categories.is_empty() {
                category.entries.push(Button::default());
            }
            categories.insert(0, category.clone());
            Some(category)
        };

        // Validate every existing category before creating any missing defaults.
        let categories_path = self.root.join("categories");
        fs::create_dir_all(&categories_path)
            .map_err(|error| format!("{}: {error}", categories_path.display()))?;
        if settings_text.is_none() {
            self.write_settings(&settings, true)?;
        }
        if let Some(category) = default {
            self.write_category(&category, true)?;
        }
        Ok(State {
            settings,
            categories,
        })
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<(), String> {
        self.write_settings(settings, false)
    }

    pub fn save_button(&self, button: &Button) -> Result<(), String> {
        self.write_button(button, false)
    }

    pub fn delete_button(&self, id: &str) -> Result<(), String> {
        validate_id(id)?;
        for mut category in self.load_categories()? {
            let count = category.entries.len();
            category.entries.retain(|button| button.id != id);
            if category.entries.len() != count {
                return self.write_category(&category, false);
            }
        }
        Ok(())
    }

    pub fn reorder_buttons(&self, category_id: &str, ids: &[String]) -> Result<(), String> {
        validate_id(category_id)?;
        let mut ranks = BTreeMap::new();
        for (index, id) in ids.iter().enumerate() {
            validate_id(id)?;
            if ranks.insert(id.as_str(), index as u64 + 1).is_some() {
                return Err(format!("duplicate entry ID {id:?} in button order"));
            }
        }
        let mut category = self
            .load_categories()?
            .into_iter()
            .find(|category| category.id == category_id)
            .ok_or_else(|| format!("category {category_id:?} does not exist"))?;
        if category.entries.len() != ids.len()
            || category
                .entries
                .iter()
                .any(|button| !ranks.contains_key(button.id.as_str()))
        {
            return Err(format!(
                "button order must contain every entry in category {category_id:?} exactly once"
            ));
        }
        let mut changed = false;
        for button in &mut category.entries {
            let sorting = ranks[button.id.as_str()];
            changed |= button.sorting != sorting;
            button.sorting = sorting;
        }
        if changed {
            self.write_category(&category, false)?;
        }
        Ok(())
    }

    fn write_button(&self, button: &Button, insert_only: bool) -> Result<(), String> {
        let category = self.updated_category(button, insert_only)?;
        self.write_category(&category, false)
    }

    fn write_settings(&self, settings: &Settings, create_only: bool) -> Result<(), String> {
        let path = self.root.join("settings.toml");
        validate_settings(settings).map_err(|error| format!("{}: {error}", path.display()))?;
        let text = toml::to_string_pretty(settings)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        write_atomic(&path, text.as_bytes(), create_only)
    }

    fn write_category(&self, category: &Category, create_only: bool) -> Result<(), String> {
        write_json(
            &self.root.join("categories").join(category.filename()),
            category,
            create_only,
        )
    }

    fn updated_category(&self, button: &Button, insert_only: bool) -> Result<Category, String> {
        validate_button(button)?;
        let mut categories = self.load_categories()?;
        for category in &mut categories {
            if let Some(existing) = category
                .entries
                .iter_mut()
                .find(|entry| entry.id == button.id)
            {
                if insert_only {
                    return Err(format!(
                        "entry {:?} already exists in {}",
                        button.id,
                        category.filename()
                    ));
                }
                *existing = button.clone();
                return Ok(category.clone());
            }
        }
        let mut category = categories
            .into_iter()
            .find(|category| category.category_name.is_none())
            .unwrap_or_default();
        category.entries.push(button.clone());
        Ok(category)
    }

    fn load_categories(&self) -> Result<Vec<Category>, String> {
        let directory = self.root.join("categories");
        let mut paths = match fs::read_dir(&directory) {
            Ok(entries) => entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| format!("{}: {error}", directory.display()))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(format!("{}: {error}", directory.display())),
        };
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        paths.sort_by_key(|path| {
            (
                path.file_name().is_none_or(|name| name != "default.json"),
                path.clone(),
            )
        });
        let mut category_ids = HashSet::new();
        let mut entry_ids = HashSet::new();
        paths
            .into_iter()
            .map(|path| {
                let text = fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let category = parse_json::<Category>(&path, &text)?;
                let mut validate = || -> Result<(), String> {
                    validate_id(&category.id)?;
                    if path.file_name().and_then(|name| name.to_str())
                        != Some(category.filename().as_str())
                    {
                        return Err(format!(
                            "category must be stored in {}",
                            category.filename()
                        ));
                    }
                    if !category_ids.insert(category.id.clone()) {
                        return Err(format!("duplicate category ID {:?}", category.id));
                    }
                    for entry in &category.entries {
                        validate_button(entry)?;
                        if !entry_ids.insert(entry.id.clone()) {
                            return Err(format!("duplicate entry ID {:?}", entry.id));
                        }
                    }
                    Ok(())
                };
                validate().map_err(|error| format!("{}: {error}", path.display()))?;
                Ok(category)
            })
            .collect()
    }
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if settings
        .recordings
        .directory
        .as_ref()
        .is_some_and(|directory| !directory.is_absolute())
    {
        return Err("recordings directory must be an absolute path".into());
    }
    if !(MIN_SOUND_BUTTON_SCALE..=MAX_SOUND_BUTTON_SCALE).contains(&settings.sound_button_scale) {
        return Err("sound button scale must be between 50 and 200 percent".into());
    }
    if settings.remote_volume > 100 || settings.local_volume > 100 {
        return Err("volume percentages must be between 0 and 100".into());
    }
    if settings.volumes_linked && settings.remote_volume != settings.local_volume {
        return Err("linked volume percentages must be equal".into());
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<(), String> {
    let valid = uuid::Uuid::parse_str(id).is_ok_and(|uuid| {
        uuid.get_version_num() == 7
            && uuid.get_variant() == uuid::Variant::RFC4122
            && uuid.to_string() == id
    });
    if !valid {
        return Err(format!("invalid ID {id:?}: expected a canonical UUIDv7"));
    }
    Ok(())
}

pub fn parse_button_color(value: &str) -> Option<iced::Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    value.parse().ok()
}

fn validate_button(button: &Button) -> Result<(), String> {
    validate_id(&button.id)?;
    if button
        .full_path
        .as_ref()
        .is_some_and(|path| !path.is_absolute())
    {
        return Err(format!("entry {:?}: full_path must be absolute", button.id));
    }
    crate::audio::PlaybackOptions {
        start_ms: button.start_ms,
        end_ms: button.end_ms,
        relative_volume: button.relative_volume,
    }
    .validate(None)
    .map_err(|error| format!("entry {:?}: {error}", button.id))?;
    if button
        .color
        .as_deref()
        .is_some_and(|color| parse_button_color(color).is_none())
    {
        return Err(format!(
            "entry {:?}: color must be an RGB hex value such as #AABBCC",
            button.id
        ));
    }
    Ok(())
}

fn read_optional(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

fn parse_json<T: DeserializeOwned>(path: &Path, text: &str) -> Result<T, String> {
    serde_json::from_str(text).map_err(|error| format!("{}: {error}", path.display()))
}

fn write_json(path: &Path, value: &impl Serialize, create_only: bool) -> Result<(), String> {
    let text =
        serde_json::to_vec_pretty(value).map_err(|error| format!("{}: {error}", path.display()))?;
    write_atomic(path, &text, create_only)
}

fn write_atomic(path: &Path, contents: &[u8], create_only: bool) -> Result<(), String> {
    let write = || -> io::Result<()> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("destination has no parent directory"))?;
        fs::create_dir_all(parent)?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
        temporary.write_all(contents)?;
        temporary.as_file().sync_all()?;
        if create_only {
            temporary
                .persist_noclobber(path)
                .map_err(|error| error.error)?;
        } else {
            temporary.persist(path).map_err(|error| error.error)?;
        }
        Ok(())
    };
    write().map_err(|error| format!("{}: {error}", path.display()))
}

#[derive(Debug, Clone)]
pub enum Change {
    Settings(Settings),
    Button(Button),
    AddButton(Button),
    DeleteButton(String),
    ReorderButtons {
        category_id: String,
        ids: Vec<String>,
    },
}

#[derive(Debug, Clone)]
pub struct Report {
    pub revision: u64,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct Writer {
    sender: Option<mpsc::Sender<WriteJob>>,
    worker: Option<thread::JoinHandle<()>>,
    revision: u64,
}

#[derive(Debug)]
struct WriteJob {
    revision: u64,
    change: Change,
    report: async_channel::Sender<Report>,
}

impl Writer {
    pub fn start(store: Store) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel::<WriteJob>();
        let worker = thread::Builder::new()
            .name("raspiboard-storage".into())
            .spawn(move || {
                let mut errors = BTreeMap::new();
                for job in receiver {
                    let (target, result) = match job.change {
                        Change::Settings(settings) => {
                            ("settings.toml".into(), store.save_settings(&settings))
                        }
                        Change::Button(button) => {
                            (format!("entry:{}", button.id), store.save_button(&button))
                        }
                        Change::DeleteButton(id) => {
                            (format!("entry:{id}"), store.delete_button(&id))
                        }
                        Change::AddButton(button) => (
                            format!("entry:{}", button.id),
                            store.write_button(&button, true),
                        ),
                        Change::ReorderButtons { category_id, ids } => (
                            format!("order:{category_id}"),
                            store.reorder_buttons(&category_id, &ids),
                        ),
                    };
                    match result {
                        Ok(()) => {
                            errors.remove(&target);
                        }
                        Err(error) => {
                            log::error!("Cannot save {target}: {error}");
                            errors.insert(target, error);
                        }
                    }
                    let _ = job.report.try_send(Report {
                        revision: job.revision,
                        errors: errors.values().cloned().collect(),
                    });
                }
            })
            .map_err(|error| format!("Cannot start the storage writer: {error}"))?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            revision: 0,
        })
    }

    pub fn queue(
        &mut self,
        change: Change,
    ) -> impl Future<Output = Report> + Send + 'static + use<> {
        self.revision += 1;
        let revision = self.revision;
        let (report, receiver) = async_channel::unbounded();
        let queued = self.sender.as_ref().is_some_and(|sender| {
            sender
                .send(WriteJob {
                    revision,
                    change,
                    report,
                })
                .is_ok()
        });
        async move {
            if queued {
                receiver.recv().await.unwrap_or_else(|_| Report {
                    revision,
                    errors: vec!["The storage writer stopped before confirming this save".into()],
                })
            } else {
                Report {
                    revision,
                    errors: vec!["Cannot save: the storage writer is unavailable".into()],
                }
            }
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            log::error!("The storage writer thread failed while draining queued saves");
        }
    }
}

#[cfg(test)]
mod tests;
