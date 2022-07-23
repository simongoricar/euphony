<div align="center">
  <h1 align="center">euphony</h1>
  <h6 align="center">an opinionated <sup>(read: personal)</sup> music library transcode manager</h6>
</div>

# Philosophy
> Over the years I've been collecting an offline music library that has been growing in size, but simulteneously getting harder to maintain.
> Considering you're here you might've encountered the same :). Before I describe the workings of euphony
> I'll just quickly outline why I'm doing this and how I've decided to approach the problem.
>
> In my case, portable file organisation became a problem relatively quickly: let's say most of your music library is lossless, but some of it is lossy,
> so you either have it all in one big folder (e.g. organized by artist, then by album), or separated into a lossless and a lossy library
> (or possibly even more if you're doing some involved sorting).
>
> Now, if you only listen on one device, none of those approaches are likely to be a problem, but for multi-device users,
> this quickly becomes a storage and deduplication nightmare.
> Ideally, you'd want a way to maintain two separate copies of the entire library:
> - one with all the **original files intact** (be it lossless or lossy) and
> - a separate (aggregated, if you will) copy that contains all the files transcoded down to a more manageable size.
>
> **`euphony` was written to solve this problem efficiently.**

Euphony's philosophy is that you should split your library into smaller chunks: one directory for lossless, one for lossy audio, one for a specific
collection, etc. (as many as you need). It does not force you to have multiple libraries, it works just as well with one library. However, 
as described in the preamble, this philosophy also acknowledges that you might want to take the library with you on the 
go, something that is hard to do when a part of your library contains possibly huge lossless files.

Here's how euphony opts to solve this:
- *you register a list of sub-libraries* that contain the same basic folder structure (one directory per artist containing one directory per album),
- you may opt to *validate the library for any collisions* (see the `validate-all` command) so you don't store two copies of the same album in two separate sublibraries,
- when you wish to assemble your entire library into a smaller transcoded copy, you run one of the `transcode-*` commands, 
  which takes all of your registered sub-libraries that contain original files and transcodes everything into MP3 V0 and puts the resulting files into the transcoded 
  library - this is the library that you probably want to take with you "on the go".

As mentioned, audio files are transcoded into MP3 V0 in the process. I've chosen MP3 V0 for now due to a 
good tradeoff between space on disk and quality (V0 is pretty much transparent anyway and should be more than enough for on-the-go listening, and you *still* have original files).
For transcoding efficiency euphony also stores minimal metadata about each album's contents in a file called `.librarymeta`. 
This is done to know which files haven't changed and can be skipped the next time you request transcoding of your library. 
Implementation details are available below in `4.2`.

**More importantly, euphony *does not* organise your (original) audio files** - for this job [MusicBrainz Picard](https://picard.musicbrainz.org/) 
is a full-featured tagger (just a recommendation); it is several magnitudes better than this project could ever achieve. 
You may even opt to use [Beets](https://beets.readthedocs.io/en/stable/) for most tasks regarding source library organisation.

**Regardless, `euphony`'s place in my (and maybe yours) music library toolset is well-defined: 
a CLI for validating your library and managing transcodes for on-the-go listening quickly and efficiently.**  

---

<div align="center">
  <img src="https://raw.githubusercontent.com/DefaultSimon/euphony/master/assets/euphony-short-demo.gif" width="90%" height="auto">
  <div>Short demo of the transcoding process.</div>
</div>

---

## 1. Library structure
Having the library structure be configurable would get incredibly complex very quickly, so `euphony` expects the user
to have the following exact structure in each registered library:

```markdown
  <library directory>
  |-- <artist directory>
  |   |-- <album directory>
  |   |   |-- <... audio files (whichever types you allow inside each library's configuration)>
  |   |   |-- <... optionally, cover art>
  |   |   |-- <... optionally, some album-related README, logs, etc.>
  |   |   |-- <... optionally, other directories that don't really matter for this purpose (they are ignored)>
  |   |   |   [the second two are examples, euphony will allow whatever you set in the validation configuration]
  |   |-- <... possibly some artist-related README, etc. (whatever you allow in the validation configuration table)>
  | [other artist directories ...]
  | [other files (again, whichever types/names you allow in the validation configuration) ...]
```

Any other structure will almost certainly fail with `euphony`.

## 2. Installation
Prerequisites for installation:
- [Rust](https://www.rust-lang.org/),
- a [copy of ffmpeg](https://ffmpeg.org/) binaries ([Windows builds](https://www.gyan.dev/ffmpeg/builds/)).

Clone (or download) the repository to your local machine, then move into the directory of the project and do the following:
- on Windows, run the `./install-euphony.ps1` PowerShell script to compile the project and copy the required files into the `bin` directory,
- otherwise, run `cargo build --release` to compile the project, after which you'll have to get the binary 
  from `./target/release/euphony.exe` and copy it (and the configuration file) to a place of your choosing.

## 3. Preparation
Before running the binary you've built in the previous step, make sure you have the `configuration.TEMPLATE.toml` handy.
If you used the `install-euphony.ps1` script, it will already be prepared in the `bin` directory. 
If you're on a different platform, copy one from the `data` directory.

The `configuration.toml` file must be in `./data/configuration.toml` (relative to the binary) or explicitly stated with the `--config` option.
The PowerShell install script places this automatically (you just need to rename and fill out the file), other platforms will require a manual copy.

Make sure the file name is `configuration.toml`, *carefully read* the explanations inside and fill out the contents. 
It is mostly about specifying where ffmpeg is, which files to track, where your libraries reside and what files you want to allow or forbid inside.

> As an example, let's say I have two separate libraries: a lossy and a lossless one. The lossless one has its 
> `allowed_audio_files_by_extension` value set to `["flac"]`, as I don't want any other file types inside. The lossy one instead
> has the value `["mp3"]`, because MP3 is my format of choice for lossy audio for now. If I were to place a non-FLAC file inside the
> lossless library, euphony would flag it for me when I tried to run `euphony validate-all`.

Next, **extract the portable copy of ffmpeg** that was mentioned above. Again, unless you know how this works,
it should be just next to the binary in a folder called `tools`. Adapt the `tools.ffmpeg.binary` configuration value in the 
configuration file to a path to the ffmpeg binary.

Change any other configuration values you haven't yet, then save. **You're ready!**

## 4. Usage
Run `euphony` with the `--help` option to get all available commands and their short explanations:
```html
euphony 0.1.0
Simon G. <simon.peter.goricar@gmail.com>
Euphony is an opinionated music library transcode manager that allows the user to retain high quality audio files in multiple separate libraries while also
enabling the listener to transcode their library with ease into a smaller format (MP3 V0) to take with them on the go. For more info, see the README file in
the repository.

USAGE:
euphony.exe [OPTIONS] <SUBCOMMAND>

  OPTIONS:
  -c, --config <CONFIG>
        Optionally a path to your configuration file. Without this option, euphony tries to load ./data/configuration.toml, but understandably this might
        not always be the most convinient location.

  -h, --help
        Print help information

  -V, --version
        Print version information

  SUBCOMMANDS:
  help
        Print this message or the help of the given subcommand(s)
  list-libraries
        List all the registered libraries.
  show-config
        Loads, validates and prints the current configuration from `./data/configuration.toml`.
  transcode-album
        Transcode only the specified album into the aggregated (transcoded) library. The current directory is used by default, but you may pass a
        different one using "--dir <path>".
  transcode-all
        Transcode all registered libraries into the aggregated (transcoded) library.
  transcode-library
        Transcode only the specified library into the aggregated (transcoded) library. Requires a single positional parameter: the library name (by full
        name), as configured in the configuration file.
  validate-all
        Validate all the available (sub)libraries for inconsistencies, such as forbidden files, any inter-library collisions that would cause problems
        when aggregating (transcoding), etc.
  validate-library
        Validate a specific library for inconsistencies, such as forbidden files.
```

For more info about each individual command, run `euphony <command-name> --help`.

### 4.1 About transcoding ("aggregation")
Using any of the `transcode-*` command will attempt to transcode (sometimes called aggregate) the selected part of the music library 
into a single folder called the aggregated library path (see `aggregated_library.path` in the configuration file).
This is the directory that will contain all the transcodes, or to put it differently, this is the portable smaller library. 
The files are all MP3 V0 (though customizing should be reasonably easy through the configuration file, see `tools.ffmpeg.to_mp3_v0_args`), reasoning explained above.


#### 4.2 `.librarymeta` implementation details
To make sure we don't have to transcode or copy all the files again when changing a single one, 
euphony stores a special file in the root directory of each **album** called `.librarymeta`.

The contents of the file a JSON document, similar to this one:
```json5
{
  // All tracked files in the directory are listed here. 
  // Which files are tracked is dictated by the configuration in the file_metadata table 
  // (tracked_audio_extensions and tracked_other_extensions) and not by any other option.
  "files": {
    // Each file has several attributes - if any of them don't match, 
    // the file has likely changed and will be transcoded or copied again.
    // Paths are relative to the .librarymeta file in question.
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
- `time_modified` is the file modification time (as reported by OS),
- `time_created` is the file creation time (as reported by OS).

If any of these attributes don't match for a certain file, we can be pretty much certain the file has changed.
The opposite is not entirely true, but enough for most purposes.
