# Music library manager


## 1. Function: library aggregation
Using the `aggregate` command will attempt to aggregate the various parts
of the music library into a single folder (see `aggregated_library.path` in `configuration.toml`).

The aggregated library is meant for portability (such as on a phone) 
and contains MP3 V0 or smaller files.

### Implementation details
To make sure we don't have to convert/copy all the files again when changing
a single one, we store a special file in the root directory of each **album**: `.librarymeta`.

The contents of the file a JSON document, example:
```json5
{
  // All files in the directory are listed here.
  "files": {
    // Each file has several attributes - if any of them 
    // mismatch, the file has likely changed.
    // Paths are relative to the .librarymeta file.
    "01 Amos Roddy - Aeronaut.mp3": {
      // Removed-mid design due to this likely being too slow.
      // "hash_blake3": "a8add4bdddfd93e4877d2746e62817.....",
      "size_bytes": 235901,
      "time_modified": 9234759811, // or null
      "time_created": 1394853, // or null
    },
    // other files ...
  }
}
```

- `hash_blake2` is a [BLAKE3](https://github.com/BLAKE3-team/BLAKE3) hash of the entire file (removed mid-design),
- `size_bytes` is the size of the entire file in bytes,
- `time_modified` is the file modification time (as reported by OS),
- `time_created` is the file creation time (as reported by OS).

If any of these attributes don't match for a certain file, we can be pretty much certain the file has changed.

---
TODO rest of documentation
