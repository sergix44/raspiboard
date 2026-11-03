# Saved categories and the background writer

`src/storage.rs` owns saved data, validation, and file operations.
`src/ui/persistence.rs` connects them to the interface. `Soundboard::boot` restores
saved data before the main window opens; `Soundboard::default` creates in-memory
state for tests.

## Data records

Serde serializes global settings as TOML and categories as JSON:

| File | Record | Data |
| --- | --- | --- |
| `settings.toml` | `Settings` | Optional theme, remote/local percentages, linking, sound button scale. |
| `categories/default.json` | `Category` | UUIDv7 ID, null category name, sound entries. |
| `categories/<category-id>.json` | `Category` | UUIDv7 ID matching the filename, category name, sound entries. |

Each `Button` entry stores `id`, `name`, `full_path`, `start_ms`, `end_ms`,
`color`, `relative_volume`, `overlap`, `hotkey`, and `sorting`. Category and entry
IDs must be canonical UUIDv7 strings. Entry IDs must be unique across categories.
The installed `uuid` crate generates IDs with `Uuid::now_v7`.

`name` and the nullable metadata fields may be null; omitted nullable fields load
as null, and omitted `overlap` loads as false. `sorting` is a required unsigned
integer. It orders entries numerically within each category, with stable array
order for ties. New UI entries use the default category's largest sorting value
plus one (saturating at the unsigned integer limit). `State` owns categories;
`State::ordered_buttons` projects them into the flat grid, with the default first
and named categories in filename order. `grid.json` is no longer needed.

`full_path` is null for an unassigned button or an absolute reference to the
original audio file. Loading metadata does not copy or open audio. Missing audio is
reported when playback or waveform analysis is attempted. `Button::display_name` uses a custom
nonblank name, then the filename stem, then `Choose sound`.

Trim times define the playback range in milliseconds; relative volume is a finite
gain from −15 to +15 dB; color is an RGB hex string or null for the theme default.
The Options editor updates these fields. Its Overlap Mode checkbox saves `overlap`
through the same button writer. False replaces current playback; true adds an
independent instance. The flag defaults to false for existing entries that omit it.
`hotkey` stores the desktop-confirmed trigger description; null disables that
sound's shortcut. The desktop owns the actual key registration, so changing this
text manually does not reassign keys. Enabled sounds are rebound by ID on launch;
desktop changes and Clear save through the same button writer. Invalid option
values are reported without rewriting files.

Settings use a typed application theme choice serialized as Deck or Console.
Missing fields use defaults, including Deck for an absent theme. Built-in Iced
theme names and arbitrary names are rejected while preserving invalid files.
Volume percentages must be 0–100 and equal when linked. Sound button scale must
be 50–200, defaulting to 100. Unknown fields are rejected in every record.

## Startup

`Store::in_home` resolves the user's home directory and appends `.raspiboard`.
`load_or_create` reads settings and every JSON category, validating all existing
records before creating missing defaults. It checks IDs, category filenames,
global entry uniqueness, absolute audio paths, and settings bounds. Errors
include the affected path, and existing files remain untouched.

On a fresh store it creates settings and a default category containing one empty
button, both with generated UUIDv7 IDs. Existing empty categories stay empty.
If named categories exist without a default, startup creates an empty default
category. Non-JSON files in the categories directory are ignored.

The previous `buttons/` directory and `grid.json` are neither read nor rewritten;
there is no automatic import or compatibility path.

`Soundboard::from_saved` restores settings, projects ordered entries, and keeps
the default category's entry count so new entries are inserted before named
categories in the live grid. Playback stays idle, Settings stays closed, and
application discovery chooses the first available recording stream. Load failure
shows an error with in-memory defaults and autosave disabled for that launch.

## Saves

Accepted assignments queue `Change::Button`; additions queue
`Change::AddButton`. `Store::updated_category` validates and loads categories,
finds the entry's owner, and replaces that entry while preserving its category
and siblings. New entries append to the default category. Additions reject
existing entry IDs instead of replacing them. The UI changes only `full_path`
when reassigning a sound, preserving that entry's metadata and sorting.

One worker processes jobs in queue order. Each update reads the latest category
contents, so queued changes to neighboring entries do not overwrite each other.
`queue` sends synchronously and returns a detached future for an iced `Task`.
Canceled or stale pickers and temporary visual/playback state produce no saves.

The writer validates before changing a destination. A `NamedTempFile` in the same
directory is written, synced, and persisted atomically. Initialization uses
no-clobber persistence. Normal saves replace only their category or settings
file. Dropping `Writer` drains pending work and joins its worker thread.

Reports track failures by settings or entry ID, retaining each failed edit until
that same entry saves successfully. A successful sibling edit cannot hide it,
even when both entries share a category file. Reports include an increasing
revision. The UI ignores older reports,
so a delayed success cannot hide a newer failure. Storage and audio errors are
displayed separately. Coordination between multiple running app instances or
external edits is not implemented; manually edit files with the app closed.

## Tests

`src/storage/tests.rs` uses real files in unique temporary directories beneath
the ignored `test-results/` directory. It checks complete category JSON, UUIDv7,
ordering, metadata round trips, named category ownership, duplicate rejection,
corruption preservation, write isolation, queue draining, and error recovery.

`src/ui/persistence/tests.rs` checks startup and update handlers through real
storage, including generated IDs, metadata preservation during reassignment,
adding to the default category with named categories present, canceled/stale
pickers, and stale save reports. Tests never modify `HOME`, use the user's real
configuration, or start the GUI or audio workers.


## Saving pad order

`Change::ReorderButtons` carries a category ID and that category's complete ordered
entry-ID list. The writer rereads and validates every category, verifies exact
unique membership, then changes only sorting ranks and atomically rewrites the
affected category once. Fresh names, paths and sound options are preserved; no
source audio or neighboring category files are changed. Already-correct orders
and empty categories avoid unnecessary writes. The writer error key is tied to
the category, allowing a successful retry to clear its previous ordering error.
