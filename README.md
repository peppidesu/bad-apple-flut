# bad-apple-flut
A general-purpose pixelflut video player. 
https://github.com/defnull/pixelflut

## Dependencies
bad-apple-flut depends on `ffmpeg` for extracting frames from videos and applying the scaling + frame-rate conversion.

## Usage
```
bad-apple-flut -i <input-file> [-x <x-offset>] [-y <y-offset] [--width <width>] [--height <height>] [--compression <none|low|medium|high|trash-compactor>] [--jit] [--nocache]

Options:
  -i, --input <INPUT>              Input file
  -x <X_OFFSET>                    Horizontal offset (in px) [default: 0]
  -y <Y_OFFSET>                    Vertical offset (in px) [default: 0]
  --width <WIDTH>                  Width (in px) [default: same as source]
  --height <HEIGHT>                Height (in px) [default: same as source]
  --fps <FPS>                      Frame-rate (in fps) [default: same as source]
  --compression <COMPRESSION>      Compression level [default: medium] [possible values: none, low, medium, high, trash-compactor]
  --nocache                        Ignore frame cache
  --jit                            Compress frames just-in-time
  --debug                      
  -h, --help                       Print help

```
The input file can be any video format so long as ffmpeg supports it.

### Frame extraction & cache directories
Video frames are extracted and stored in a cache directory ahead-of-time, due to limitations of the ffmpeg CLI and performance considerations. This cache directory is stored in `<cache_dir>/bad-apple-flut`, where `<cache_dir>` is the users cache directory (see https://docs.rs/dirs/latest/dirs/fn.cache_dir.html).

Be aware that this cache directory can become very large for long videos, so make sure you have sufficient disk space available. 20 GB of free disk space is the recommended minimum. A solution for reducing the cache size is planned and will be added in a future update.

### JIT compression
By default, bad-apple-flut will generate the compressed data stream ahead-of-time in RAM to improve streaming performance. This means bad-apple-flut may run out of memory for long/high-resolution videos.

To avoid this problem, or to reduce the amount of RAM bad-apple-flut uses in general, you can use the `--jit` flag to instead compress frames just-in-time. This will defer frame compression until right before the frame gets sent to the server. 

## Configuration
The configuration file is stored in `<config_dir>/bad-apple-flut/config.toml`, where `<config_dir>` is the users config directory (see https://docs.rs/dirs/latest/dirs/fn.config_dir.html).

```toml
host = "localhost:1234"
thread_count = 8
```

- `host` contains the address of the pixelflut server to send the video to.
- `thread_count` sets the amount of threads to use for compression (when compressing frames ahead-of-time), and for pushing pixelflut commands onto the connection.

## Contributions
Contributions to the project are welcome, so feel free to suggest changes or report issues.

## Special thanks
- [patagona](https://github.com/patagonaa), for hosting the pixelflut server that gave birth to this project
- The awsome people in the Crow Academy discord server who also made pixelflut clients:
  - berries :3
  - ked
  - trintler
  - vym
  - pioli
  - zetty
  - skelly
