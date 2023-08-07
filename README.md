<div align="center">
  <h1 align="center">euphony</h1>
  <h6 align="center">a ♯ personal music library transcode manager</h6>
</div>

---


**Important: `euphony` *does not organise* your original audio files** - it transcodes your already-organised library/libraries into an aggregated (usually lossy) library, leaving your source libraries alone. If music library organisation is what you're after, you might want to look into tools like [MusicBrainz Picard](https://picard.musicbrainz.org/) or the more advanced [Beets](https://beets.readthedocs.io/en/stable/) CLI.


<div align="center">
  <img src="https://raw.githubusercontent.com/DefaultSimon/euphony/master/assets/euphony-v2.0.0-demo.gif" width="100%" height="auto">
  <div><code>transcode</code> command demo</div>
</div>

---

## Table of contents
* [1. Why and how](#1-why-and-how)
* [2. Library structure](#2-library-structure)
* [3. Installation](#3-installation)
* [4. Setup](#4-setup)
* [5. Usage](#5-usage)
* [5. Advanced topics](#5-advanced-topics)
* [6. Implementation details](#6-implementation-details)

## Other resources
* [Changelog (master)](https://github.com/DefaultSimon/euphony/blob/master/CHANGELOG.md)

---

# 1. Why and how
<details>

<summary>📇 Why I made euphony</summary>

> Over the years, I've been collecting an offline music library that has been growing in size, but simultaneously getting harder to maintain.
> Considering you're here, you might have encountered the same :). Here's a quick outline of why and how.
>
> Let's say most of your music library is lossless, but a significant chunk of it is lossy.
> In this case, you could:
> - have both lossless and lossy files in the same folder (e.g., organized by artist, then by album, mixing the qualities), or,
> - separate lossless and lossy folders (each one again organized by artist, then by album, etc.).
>
> If you only listen on one device, such as your computer, neither of those approaches is likely to pose a problem. 
> However, for multi-device users, large libraries quickly become both a storage and a deduplication nightmare.
> Ideally, you'd want to maintain the original library (or libraries) as they were,
> but still have a separate - *transcoded* or *aggregated*, if you will - version of your entire collection containing files from all the
> libraries transcoded down to a more manageable size, ready for on-the-go listening.
>
> This is the problem `euphony` was written to solve.

</details>


Euphony's workflow acknowledges that you might have multiple libraries: one for lossless, one for lossy audio, one for a specific collection, etc. 
It *does not force you to have multiple libraries*, it works just as well with a single one, or even a mixed library of lossless and lossy audio files.

Euphony becomes useful when you want to take the music library with you on the go (e.g. having a copy of your music library on a phone). 
In those cases, you might not want (or be able) to copy the large lossless files due to storage limitations.

The obvious solution is to **transcode your library** down to something like MP3 V0 and copy those transcoded files to your other devices. 
Still, doing this manually or even with a simple script is a tedious process, prone to forgetfulness and occasional human errors.

---

Here's how `euphony` solves this with an automated transcoding process:
- *You register a list of libraries* in the configuration file.  
  Note that euphony currently supports only the following library structure: artist directory containing albums (see example below).
- Then you may opt to *validate the library for any unusual files and collisions* - see the `validate` command.  
  This way, if you have "multiple libraries" (e.g. one for lossy and one for lossless), euphony will inform you of any potential collisions (e.g. same album in both libraries) so you don't accidentally store two copies of the same album in two places 
  (it would also be unclear as to which version of the album euphony should transcode).
- When you wish to transcode your entire music library into a smaller single-folder transcoded copy, you run the `transcode` command.   
  This takes all of your registered source libraries and transcodes everything in them into MP3 V0 (by default), putting the resulting files from all your source libraries into a single *transcoded library* - this is the directory that you take with you on the go.  
  That directory will contain all the transcoded versions of albums from all the artists of all the registered libraries together in one place. Euphony will also copy album art and any other data files (as configured).

## 1.1 Diffing
If you run the `transcode` command two times without modifying any of your source libraries, you'll notice that euphony won't re-transcode anything. 
This is because euphony tracks your source files' size and modification date in order to avoid processing albums that haven't changed.

This is done by storing three types of files:
- Minimal metadata about each album's tracked files is stored in a file called `.album.source-state.euphony` (in the source album directory) 
  and `.album.transcode-state.euphony` (in the transcoded album directory).
- To detect album and artist removal, euphony also stores the `.library.state.euphony` file at the root of each registered source library.

Implementation details of this change detection algorithm are available below.

## 1.2 MP3 V0
Audio files are transcoded into MP3 V0 in the process by default. I've chosen MP3 V0 for now due to a 
good tradeoff between space on disk and quality (V0 is pretty much transparent anyway and should be more than enough for on-the-go listening, and you *still* have the original files).

> Don't like MP3 V0? No problem, modify your configuration file to have ffmpeg transcode your audio into something else.

---


# 2. Library structure
Having the library structure be configurable would quickly become very complex, 
so at the moment `euphony` expects the user to have the following structure for each library:

```markdown
<library's base directory>
|
|-- <artist directory>
|   |
|   |- [possibly some album-related text files, logs, etc.]
|   |  (settings for other files apply here (see "other files" section below))
|   |
|   |-- <album directory>
|   |   |
|   |   | ... [audio files]
|   |   |     (any audio types you allow inside each library - see 
|   |   |     `allowed_audio_file_extensions` in the configuration file)
|   |   |
|   |   | ... [cover art]
|   |   |
|   |   | ... [some album-related text files, logs, etc.]
|   |   |     (settings for other files apply here (see "other files" section below))
|   |   |
|   |   | ... <potentially other directories that you don't want transcoded or copied>
|   |   |     (album subdirectories are ignored by default, see `depth` in per-album configuration)
|
|-- <any other ignored directory>
|   (it is sometimes useful to have additional directories inside your library that are
|    not artist directories, but instead contain some other stuff (e.g. temporary files) 
|    you don't want to transcode - these directories can be ignored for each individual 
|    library using `ignored_directories_in_base_directory`)
|
| ... [other files]
|     (of whatever type or name you allow in the configuration, see
|      `allowed_other_file_extensions` and `allowed_other_files_by_name` - these settings
|      also apply to artist and album directories above)
```  

<details>

<summary>✍️ <b>Example of a library</b> and its corresponding configuration</summary>

Look at the following directory structure:
```markdown
  LosslessLibrary
  |
  |- LOSSLESS_README.txt
  |
  |-- Aindulmedir
  |   |-- The Lunar Lexicon
  |   |   | 01 Aindulmedir - Wind-Bitten.flac
  |   |   | 02 Aindulmedir - Book of Towers.flac
  |   |   | 03 Aindulmedir - The Librarian.flac
  |   |   | 04 Aindulmedir - Winter and Slumber.flac
  |   |   | 05 Aindulmedir - The Lunar Lexicon.flac
  |   |   | 06 Aindulmedir - Snow Above Blue Fire.flac
  |   |   | 07 Aindulmedir - Sleep-Form.flac
  |   |   | cover.jpg
  |   |   | Aindulmedir - The Lunar Lexicon.log
  |
  |-- Dakota
  |   |-- Leda
  |   |   | 01 Dakota - Automatic.mp3
  |   |   | 02 Dakota - Icon.mp3
  |   |   | 03 Dakota - Easier.mp3
  |   |   | 04 Dakota - Leave Me Out.mp3
  |   |   | 05 Dakota - Bare Hands.mp3
  |   |   | 06 Dakota - Tension.mp3
  |   |   | cover.jpg
  |
  |
  |-- _other
  |   | some_other_metadata_or_something.db
  |   | ... other files we don't want to validate or transcode
```

In this example there exists a lossless library in a directory named `LosslessLibrary`. We'll call it `Lossless`. We want to transcode both `mp3` and `flac` files and include any `jpg` and `log` files in our transcoded library. We also don't want euphony to touch the `_other` directory.

We get the following:

```toml
[libraries.lossless]
name = "Losless"
path = "/some/absolute/path/to/LosslessLibrary"
ignored_directories_in_base_directory = ["_other"]

[libraries.lossless.validation]
allowed_audio_file_extensions = ["mp3", "flac"]
allowed_other_file_extensions = ["jpg", "log"]
allowed_other_files_by_name = []

[libraries.lossless.transcoding]
audio_file_extensions = ["mp3", "flac"]
other_file_extensions = ["jpg", "log"]
```

</details>


---

# 3. Installation
Prerequisites for installation:
- [Rust](https://www.rust-lang.org/) (minimal supported Rust version as of `euphony v2.0.0` is `1.70.0`!),
- a [ffmpeg](https://ffmpeg.org/) binaries ([Windows builds](https://www.gyan.dev/ffmpeg/builds/)).

Clone or download this repository to your local machine, then move into the directory of the project and do the following:
- Windows: run the convenient `./install-euphony.ps1` PowerShell script to compile the project and copy the required files into the `bin` directory and add that to your `PATH` afterwards,
- Linux/other: run `cargo build --release` to compile the project. You'll find the binary in `./target/release/euphony.exe` - copy it to a place of your choosing along with the configuration file template.


# 4. Setup
Before running the binary you've built in the previous step, make sure you have the `configuration.TEMPLATE.toml` handy.
If you used the `install-euphony.ps1` script, it will already be prepared in the `bin` directory. 
If you're on a different platform, copy one from the `data` directory.

The `configuration.toml` file must be in `./data/configuration.toml` (relative to the binary) or wherever else you prefer with the `--config` option.
The PowerShell install script places this automatically (you just need to rename and fill out the file), other platforms will require a manual copy.

Make sure the file name is named `configuration.toml`, *carefully read* the explanations inside and fill out the contents.
If you're unfamiliar with the format, it's [TOML](https://toml.io/en/).
It is mostly about specifying where ffmpeg is, which files to track, where your libraries reside and what files you want to allow or forbid inside them.

> As an example, let's say I have two separate libraries: a lossy and a lossless one. The lossless one has its 
> `allowed_audio_file_extensions` value set to `["flac"]`, as I don't want any other file types inside. The lossy one instead
> has the value `["mp3"]`, because MP3 is my format of choice for lossy audio for now. If I were to place a non-FLAC file inside the
> lossless library, euphony would flag it for me as an error when I ran `euphony validate`.

Next, **extract the portable copy of ffmpeg** that was mentioned above. Again, unless you know how this works,
it should be just next to the binary in a folder called `tools`. Adapt the `tools.ffmpeg.binary` configuration value in the 
configuration file to a path to the ffmpeg binary.

Change any other configuration values you haven't yet, then save. **You're ready!**

---


# 5. Usage
Run `euphony` with the `--help` option to get all available commands and their short explanations:
```
Euphony is a music library transcode manager that allows the user to retain 
high quality audio files in one or more libraries and helps with transcoding 
the collection into a smaller size. That smaller version of the library can 
then be used on portable devices or similar occasions where space has a larger 
impact. For more info, see the README file in the repository.

Usage: euphony [OPTIONS] <COMMAND>

Commands:
  transcode
          Transcode all libraries into the aggregated library. 
          [aliases: transcode-collection]
  validate
          Validate all the available libraries for inconsistencies, 
          such as forbidden files, any inter-library collisions that would 
          cause problems when transcoding, etc. 
          [aliases: validate-collection] 
  show-config
          Loads, validates and prints the current configuration.
  list-libraries
          List all the registered libraries registered in the configuration.
  help
          Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>
          Optionally a path to your configuration file. Without this option, 
          euphony tries to load ./data/configuration.toml, but understandably 
          this might not always be the most convenient location.   

  -v, --verbose
          Increase the verbosity of output.

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

For more info about each command, run `euphony <command-name> --help`.

### 5.1 `transcode`
Using the `transcode` command will scan your source libraries for changes and transcode the entire music collection into a single folder called the transcoded or aggregated library (see `aggregated_library.path` in the configuration file).

This is the directory that will contain all transcoded files (and cover art).
The files will be MP3 V0 by default (changing this should be reasonably easy - see `tools.ffmpeg.to_mp3_v0_args` in the configuration file).


---

# 6. Advanced topics
> What follows are advanced features - I'd recommend getting acquainted with the rest of the functionality first.

## 6.1. `.album.override.euphony` (per-album overrides)
You can create an `.album.override.euphony` file in the root of each source album directory (same directory as the `.album.source-state.euphony` file). This file is optional. Its purpose is to influence the scanning and transcoding process for the relevant album.

At the moment, this file can contain the following options:
```toml
# This file serves as a sample of what can be done using album overrides.

[scan]
# How deep the transcoding scan should look.
# 0 means only the album directory and no subdirectories (most common, this is also the default without this file).
# 1 means only one directory level deeper, and so on.
depth = 0
```

> In case this description falls behind, an up-to-date documented version of the `.album.override.euphony` file is always available in the `data` directory.

Why is this useful? Well, let's say you have an album that has multiple discs, each of which is in a separate directory, like so:
```markdown
<album directory>
|- cover.jpg
|
|-- Disc 1
|   |- <... a lot of audio files ...>
|
|-- Disc 2
|   |- <... a lot of audio files ...>
|
|-- Disc 3
|   |- <... a lot of audio files ...>
|
|-- Disc 4
|   |- <... a lot of audio files ...>
|
|-- <...>
```

In this case you may want to create an `.album.override.euphony` file inside the album directory and set the `depth` setting to `1`.
This will make euphony scan one directory deeper, catching and transcoding your per-disc audio files.

---

# 7. Implementation details

#### 7.1 `.album.source-state.euphony` / `.album.transcode-state.euphony`
To make sure we don't have to transcode or copy all the files again when changing a single one,
euphony stores a special file in the root directory of each **album** called `.album.source-state.euphony`.

The contents of the file are in JSON, similar to the example below:
```json5
{
  "schema_version": 2,
  // All tracked files in the directory are listed here. 
  // Which files are tracked is dictated by the configuration 
  // in the file_metadata table (audio_file_extensions and other_file_extensions).
  "tracked_files": {
    "audio_files": {
      // Each file has several attributes - if any of them don't match, 
      // the file has likely changed and will be transcoded or copied again.
      // Paths are relative to the base album directory.
      "01 Amos Roddy - Aeronaut.mp3": {
        "size_bytes": 3403902,
        "time_modified": 1636881979.7336252,
        "time_created": 1669553407.7848136,
      }
      // ...
    },
    "data_files": {
      "cover.png": {
        "size_bytes": 32955,
        "time_modified": 1636881979.7336252,
        "time_created": 1669553407.7848136,
      }
    },
  }
}
```

Fields:
- `size_bytes` is the size of the entire file in bytes,
- `time_modified` is the file modification time (as reported by OS, compared to one decimal of precision),
- `time_created` is the file creation time (as reported by OS, compared to one decimal of precision).

If any of these attributes don't match for a given file, we can be pretty much certain the file has changed.
The opposite is not entirely true, but enough for most purposes.

A similar file named `.album.transcode-state.euphony` with almost the same structure is saved in the transcoded album directory.

> For more details about these files, see the `src/commands/transcode/album_state` module.
