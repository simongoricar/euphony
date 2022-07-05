<div align="center">
  <h1 align="center">euphony</h1>
  <h6 align="center">an opinionated music library transcode manager</h6>
</div>

# Philosophy
> Over the years I've been collecting an offline music library that has been growing in size, but simulteneously getting harder to maintain.
> Considering you're here you might've encountered the same :). Before I describe the workings of euphony
> I'll just quickly outline why I'm doing this and how I've decided to approach the problem.
>
> In my case, file organisation became a problem relatively quickly: let's say most of your music library is lossless, but some of it is lossy,
> so you either have it all in one big folder (e.g. organized by artist, then by album), or separated into a lossless and a lossy library
> (or possibly even more if you're doing some involved sorting).
>
> Now, if you only listen on one device, none of those approaches are likely to be a problem, but for multi-device users,
> this quickly becomes a storage nightmare.
> Ideally, you'd want a way to maintain two separate copies of the entire library:
> - one with all the **original files intact** (be it lossless or lossy) and
> - a separate ("aggregated", if you will) copy that contains all the files transcoded down to a more manageable size.
>
> **Managing this system easily and efficiently is what euphony was made to solve.**

Euphony's philosophy is that you should split your library into smaller chunks: one directory for lossless, one for lossy audio, one for a specific
collection, etc. (as many as you need). It does not force you to have multiple libraries, it works just as well with one library. However, 
as described in the preamble, this philosophy also acknowledges that you might want to take the library with you on the 
go, something that is hard to do when a part of your library contains possibly huge lossless files.

Here's how euphony opts to solve this:
- *you register a list of sublibraries* that contain the same basic folder structure (one directory per artist containing one directory per album),
- you may opt to *validate the library for any collisions* (see the `validate` command) so you don't store two copies of the same album in two separate sublibraries,
- when you wish to assemble your entire library into a smaller transcoded copy, you run one of the `transcode-*` commands, 
  which takes all of your registered sublibraries that contain original files and transcodes each one into MP3 V0 and puts it into the transcoded library.

As mentioned, audio files are transcoded into MP3 V0 in the process. I've chosen MP3 V0 for now due to a 
good tradeoff between space on disk and quality (V0 is pretty much transparent anyway, and you still have original files).
For transcoding efficiency it also stores very minimal metadata about each album in a file called `.librarymeta` in order 
to know which files haven't changed and can be skipped the next time you request transcoding of your library.

More importantly, **euphony *does not* organise your (original) audio files** - [MusicBrainz Picard](https://picard.musicbrainz.org/) 
is a full-featured tagger, a several magnitudes better fit for this than this project could ever achieve. 
You may even opt to use [Beets](https://beets.readthedocs.io/en/stable/) for most of this work. Regardless, euphony's place
in the music library toolset is well-defined: software for validating your library and managing transcodes.  

---

## 1. Installation
Prerequisites for installation:
- [Rust](https://www.rust-lang.org/),
- a copy of [ffmpeg](https://ffmpeg.org/) handy for later ([Windows builds](https://www.gyan.dev/ffmpeg/builds/)).

Clone (or download) the repository to your local machine, then move into the directory of the project and:
- Windows: run the `./install-euphony.ps1` PowerShell script to compile the project and copy the required files into the `bin` directory,
- Other: run `cargo build --release` to compile the project, after which you'll have to get the binary 
  from `./target/release/euphony.exe` and copy it to a place of your choosing.

## 2. Preparation
Before running the binary you've built in the previous step, make sure you have the `configuration.TEMPLATE.toml` handy.
If you used the `install-euphony.ps1` script, it will already be prepared. If you're on a different platform, copy one from `data`.

**The `configuration.toml` file must be in `./data/configuration.toml` (relative to the binary).** Again, the Windows install script
places this automatically (you just need to rename and fill out the file), other platforms will require a manual copy.

Make sure the file name is `configuration.toml` and fill out the configuration. It is mostly about specifying where
your libraries reside and what you want to have/forbid in them.

Next, **extract the portable copy of ffmpeg** that was mentioned above. Again, unless you know how this works,
it should be just next to the binary in a folder called `tools`. Adapt the `tools.ffmpeg.binary` value in the 
configuration file to a relative path from the euphony binary to the ffmpeg binary.

Change any other configuration values you wish to, then save. You're done!

## 2. Usage
Run euphony with the `--help` option to get all available commands:
```html
euphony 

USAGE:
    euphony.exe <SUBCOMMAND>

OPTIONS:
    -h, --help    Print help information

SUBCOMMANDS:
    help                 Print this message or the help of the given subcommand(s)
    show-config          Show the current configuration.
    transcode-album      Transcode the selected album into the aggregated library.
    transcode-all        Transcode all available libraries into the aggregated library.
    transcode-library    Transcode the selected library into the aggregated library.
    validate-all         Validate all libraries for aggregation (collisions, unwanted files, etc.).
```

For more info about each individual command, run `euphony <command-name> --help`. A quick rundown:

| Command           | Description                                                                                                                                   |
|-------------------|-----------------------------------------------------------------------------------------------------------------------------------------------|
| show-config       | Loads, validates and prints the current configuration from `./data/configuration.toml`.                                                       |
| validate-all      | Validates all of the available (sub)libraries for inconsistencies, such as forbidden files, any inter-library collisions, etc.                |
| transcode-album   | Transcode a single album. By default euphony uses the current directory, but you may pass a different one using `--dir <path>`.               |
| transcode-library | Transcode an entire (sub)library. Requires a single positional parameter: the library name (by key), as configured in the configuration file. |
| transcode-all     | Transcode all of the available (sub)libraries.                                                                                                |


### 2.1 About transcoding ("aggregation")
Using any of the `transcode-*` command will attempt to transcode the selected part of the music library 
into a single folder called the aggregated library path (see `aggregated_library.path` in the configuration file).
This is the directory that will contain all the transcodes, or to put it differently, this is the portable smaller library. 
The files are all MP3 V0, reasoning explained above.


#### 2.2 `.librarymeta` implementation details
To make sure we don't have to transcode or copy all the files again when changing a single one, 
euphony stores a special file in the root directory of each **album**: `.librarymeta`.

The contents of the file a JSON document, similar to this one:
```json5
{
  // All tracked files in the directory are listed here.
  "files": {
    // Each file has several attributes - if any of them 
    // mismatch, the file has likely changed.
    // Paths are relative to the .librarymeta file.
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
