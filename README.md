<div align="center">
  <h1 align="center">euphony</h1>
  <h6 align="center">an opinionated <sup>(read: personal)</sup> music library transcode manager</h6>
</div>

# Philosophy
> Over the years I've been collecting an offline music library that has been growing in size, but simultaneously getting harder to maintain.
> Considering you're here you might've encountered the same :). Before I describe the inner workings of euphony here's a quick outline of why and how.
>
> In my case, portable file organisation became a problem relatively quickly: let's say most of your music library is lossless, though some of it is lossy.
> In the above case, you either:
> -> have it both lossless or lossy in the same folder (e.g. organized by artist, then by album), or,
> -> separate lossless and lossy folders (each one again organized by artist, then by album or similarly) or possibly even something more complicated.
>
> If you only listen on one device, none of those approaches are likely to pose a huge problem. However, for multi-device users,
> this quickly becomes both a storage and a deduplication nightmare.
> Ideally, you'd want to maintain the original library as they were (be it one or more folders like described above), 
> but still have a separate (*aggregated*, if you will) version of the entire original library that contains all the files from all the
> sublibraries transcoded down to a more manageable size, ready for on-the-go listening.
>
> **`euphony` was written to solve this problem efficiently.**

Euphony's philosophy acknowledges that you might have split your library into smaller chunks: one directory for lossless, one for lossy audio, one for a specific
collection, etc. It does not force you to have multiple "sub"-libraries, it works just as well with a single one. 
However, as described in the preamble, this philosophy also acknowledges that you might want to take the library with you on the go, 
something that is hard to do when a part of your library contains possibly huge lossless files. Again, the obvious solution is to transcode
your library down to something like MP3 V0 and copy that version of the library to your portable devices.

Here's how euphony automates this transcoding process:
- *you register a list of "sub"-libraries* that contain the same basic folder structure (one directory per artist containing one directory per album),
- you may opt to *validate the library for any collisions* first (see the `validate` command) so you don't accidentally store two copies of the same album in two separate sub-libraries,
- when you wish to assemble your entire music library into a smaller single-folder transcoded copy, you run the `transcode` command, 
  which takes all of your sub-libraries with original files and transcodes everything into MP3 V0 (by default), putting the resulting files into the transcoded 
  library - this is the library that you take with you on the go.

As mentioned, audio files are transcoded into MP3 V0 in the process. I've chosen MP3 V0 for now due to a 
good tradeoff between space on disk and quality (V0 is pretty much transparent anyway and should be more than enough for on-the-go listening, and you *still* have the original files).
For transcoding efficiency euphony also stores minimal metadata about each album's contents in a file called `.album.euphony` (stored inside each source album's folder).
This is done to understand which files have and haven't changed, so we can skip most of the library the next time you request transcoding of your library after having added a single new album. 
Implementation details of this detection are available below.

### Note
**Importantly, euphony *does not* organise your (original) audio files** - for this job [MusicBrainz Picard](https://picard.musicbrainz.org/) 
is a good full-featured tagger (just a recommendation); it is several magnitudes better and more flexible than this project could ever achieve had it opted to include that functionality. 
You may even opt to use [Beets](https://beets.readthedocs.io/en/stable/) for most tasks regarding source library organisation.

**Regardless, `euphony`'s place in my (and maybe your) music library toolset is well-defined: 
a CLI for *validating* your library and *managing transcodes* for on-the-go listening quickly and efficiently.**  

---

<div align="center">
  <img src="https://raw.githubusercontent.com/DefaultSimon/euphony/master/assets/euphony-v1.2.0-demo.gif" width="90%" height="auto">
  <div>Short demo of the transcoding process.</div>
</div>

---

## 1. Library structure
Having the library structure be configurable would get incredibly complex very quickly, so `euphony` expects the user
to have the following exact structure in each sub-library:

```markdown
  <library directory>
  |-- <artist directory>
  |   |-- <album directory>
  |   |   |-- <... audio files, any extensions that you allow (see per-library configuration)>
  |   |   |-- <... optionally, cover art>
  |   |   |-- <... optionally, some album-related README, logs, etc.>
  |   |   |-- <... optionally, other subdirectories that don't really matter for this purpose (they are ignored by default)>
  |   |   |   [the second two are examples, euphony will allow whatever you set in the validation configuration]
  |   |-- <... possibly some artist-related README, etc. (whatever you allow in the validation configuration table)>
  | [other artist directories ...]
  | [other files (again, whichever types/names you allow in the validation configuration) ...]
```

Any other library structure will almost certainly fail with `euphony`.


## 2. Installation
Prerequisites for installation:
- [Rust](https://www.rust-lang.org/),
- a [copy of ffmpeg](https://ffmpeg.org/) binaries ([Windows builds](https://www.gyan.dev/ffmpeg/builds/)).

Clone (or download) the repository to your local machine, then move into the directory of the project and do the following:
- on Windows, run the convenient `./install-euphony.ps1` PowerShell script to compile the project and copy the required files into the `bin` directory,
- otherwise, run `cargo build --release` to compile the project, after which you'll have to get the binary 
  from `./target/release/euphony.exe` and copy it (and the configuration file) to a place of your choosing.


## 3. Preparation
Before running the binary you've built in the previous step, make sure you have the `configuration.TEMPLATE.toml` handy.
If you used the `install-euphony.ps1` script, it will already be prepared in the `bin` directory. 
If you're on a different platform, copy one from the `data` directory.

The `configuration.toml` file must be in `./data/configuration.toml` (relative to the binary) or wherever you want if you explicitly use the `--config` option to set the path.
The PowerShell install script places this automatically (you just need to rename and fill out the file), other platforms will require a manual copy.

Make sure the file name is named `configuration.toml`, *carefully read* the explanations inside and fill out the contents.
If you're unfamiliar with the format, it's [TOML](https://toml.io/en/), chosen for its readability and ease of editing.
It is mostly about specifying where ffmpeg is, which files to track, where your libraries reside and what files you want to allow or forbid inside them.

> As an example, let's say I have two separate libraries: a lossy and a lossless one. The lossless one has its 
> `allowed_audio_files_by_extension` value set to `["flac"]`, as I don't want any other file types inside. The lossy one instead
> has the value `["mp3"]`, because MP3 is my format of choice for lossy audio for now. If I were to place a non-FLAC file inside the
> lossless library, euphony would flag it for me as an error when I ran `euphony validate`.

Next, **extract the portable copy of ffmpeg** that was mentioned above. Again, unless you know how this works,
it should be just next to the binary in a folder called `tools`. Adapt the `tools.ffmpeg.binary` configuration value in the 
configuration file to a path to the ffmpeg binary.

Change any other configuration values you haven't yet, then save. **You're ready!**

### 3.1. Advanced usage: `.album.override.euphony` per-album files
> This is an advanced feature.

You may create an `.album.override.euphony` file in the root of each source album directory (same directory as the `.album.euphony` file).
This file is optional. Its purpose is to influence the scanning and transcoding process for the relevant album. In order to be
easily readable and editable by humans, the chosen format for this file is [TOML](https://toml.io/en/) (same as configuration files).

Available configuration values will likely expand in the future, but for now, the settings available are:
```toml
# This file serves as a sample of what can be done using album overrides.

[scan]
# How deep the transcoding scan should look.
# 0 means only the album directory and no subdirectories (most common, this is also the default without this file).
# 1 means only one directory level deeper, and so on.
depth = 0
```

In case this description falls behind, an up-to-date documented version of the `.album.override.euphony` file is available
in the `data` directory.

Why is this useful? Well let's say you have an album that has multiple discs and so many tracks you'd like to keep each
disc in a separate directory, like so:

```markdown
<album directory>
 |- cover.jpg
 |-- Disc 1
 |   |- <... a lot of audio files ...>
 |-- Disc 2
 |   |- <... a lot of audio files ...>
 |-- Disc 3
 |   |- <... a lot of audio files ...>
 |-- Disc 4
 |   |- <... a lot of audio files ...>
 |-- <...>
```

In this case you may create an `.album.override.euphony` file inside the album directory and set the `depth` setting to `1`.
This will make euphony scan one directory deeper, catching and transcoding your per-disc audio files.


## 4. Usage
Run `euphony` with the `--help` option to get all available commands and their short explanations:
```
Euphony is an opinionated music library transcode manager that allows the user to retain high quality audio files in multiple separate libraries while also enabling the listener to transcode their library wi
th ease into a smaller format (MP3 V0) to take with them on the go. For more info, see the README file in the repository.

Usage: euphony [OPTIONS] <COMMAND>

Commands:
  transcode
      Transcode all registered libraries into the aggregated (transcoded) library. [aliases: transcode-all]
  validate
      Validate all the available (sub)libraries for inconsistencies, such as forbidden files, any inter-library collisions that would cause problems when aggregating (transcoding), etc. [aliases: validate-all]
  validate-library
      Validate a specific library for inconsistencies, such as forbidden files.
  show-config
      Loads, validates and prints the current configuration from `./data/configuration.toml`.
  list-libraries
      List all the registered libraries.
  help
      Print this message or the help of the given subcommand(s)

Options:
  -c, --config <CONFIG>
      Optionally a path to your configuration file. Without this option, euphony tries to load ./data/configuration.toml, but understandably this might not always be the most convenient location.        
  -v, --verbose
      Increase the verbosity of output.
  -h, --help
      Print help information (use `-h` for a summary)
  -V, --version
      Print version information
```

For more info about each individual command, run `euphony <command-name> --help`.

### 4.1 About transcoding ("aggregation")
Using the `transcode` command will attempt to transcode (also called aggregate) the entire music library 
into a single folder called the aggregated library path (see `aggregated_library.path` in the configuration file).
This is the directory that will contain all the transcodes, or to put it differently, this is the portable smaller library. 
The files will be MP3 V0 by default (changing this should be reasonably easy - see `tools.ffmpeg.to_mp3_v0_args` in the configuration file).


#### 4.2 `.album.euphony` implementation details
To make sure we don't have to transcode or copy all the files again when changing a single one, 
euphony stores a special file in the root directory of each **album** called `.album.euphony`.

The contents of the file a JSON document, similar to this one:
```json5
{
  // All tracked files in the directory are listed here. 
  // Which files are tracked is dictated by the configuration in the file_metadata table 
  // (tracked_audio_extensions and tracked_other_extensions) and not by any other option.
  "files": {
    // Each file has several attributes - if any of them don't match, 
    // the file has likely changed and will be transcoded or copied again.
    // Paths are relative to the .album.euphony file in question.
    "01 Amos Roddy - Aeronaut.mp3": {
      "size_bytes": 235901,
      "time_modified": 9234759811, // or null
      "time_created": 1394853, // or null
    },
    // other files ...
  }
}
```

Fields:
- `size_bytes` is the size of the entire file in bytes,
- `time_modified` is the file modification time (as reported by OS, compared to one decimal of precision),
- `time_created` is the file creation time (as reported by OS, compared to one decimal of precision).

If any of these attributes don't match for a given file, we can be pretty much certain the file has changed.
The opposite is not entirely true, but enough for most purposes.
