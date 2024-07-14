# bad-apple-flut
A general-purpose pixelflut video player. 
https://github.com/defnull/pixelflut

## Dependencies
bad-apple-flut depends on `ffmpeg` for extracting frames from videos and applying the scaling + 
frame-rate conversion.

## Building from source

### Build dependencies
- `rustc 1.80.0-nightly`
- `ffmpeg`

### Steps
1. Clone the repository
   ```
   git clone https://github.com/peppidesu/bad-apple-flut.git
   ```
2. `cd` into the directory and execute the following:
   ```
   cargo build --release
   ```
3. Executable can be found in `./target/release`.

If you want to maximize performance, you can try building with:
```
RUSTFLAGS="-C target-cpu=native" cargo build --release
```
This will enable CPU-specific optimizations.

## Usage
```
bad-apple-flut -i <input-file> [-x <x-offset>] [-y <y-offset>] [--width <width>] [--height <height>] 
  [--compression <none|low|medium|high|trash-compactor|number>] [--jit] [--nocache]

Options:
  -i, --input <INPUT>              Input file
  -x <X_OFFSET>                    Horizontal offset (in px) [default: 0]
  -y <Y_OFFSET>                    Vertical offset (in px) [default: 0]
  --width <WIDTH>                  Width (in px) [default: same as source]
  --height <HEIGHT>                Height (in px) [default: same as source]
  --fps <FPS>                      Frame-rate (in fps) [default: same as source]
  --compression <COMPRESSION>      Compression level [none|low|medium|high|trash-compactor|number]
  --canvas <CANVAS>                Canvas to draw to 
  --nocache                        Ignore frame cache
  --jit                            Compress frames just-in-time
  --debug                      
  -h, --help                       Print help

```
The input file can be any video format so long as ffmpeg supports it.

### Frame extraction & cache directories
Video frames are extracted and stored in a cache directory ahead-of-time, due to limitations of the 
ffmpeg CLI and performance considerations. This cache directory is stored in 
`<cache_dir>/bad-apple-flut`, where `<cache_dir>` is the users cache directory (see 
https://docs.rs/dirs/latest/dirs/fn.cache_dir.html).

Be aware that this cache directory can become very large for long videos, so make sure you have 
sufficient disk space available. 20 GB of free disk space is the recommended minimum. A solution for
reducing the cache size is planned and will be added in a future update.

### Compression algorithms
bad-apple-flut supports the following compression algorithms:

#### v1
Fixed delta-treshold compression. Only updates pixels when color difference in YUV space exceeds a given amount.

**Pros**:
- Fast to compute

**Cons**:
- Artifacts tend to accumulate
- Large spikes in bandwidth usage 
- Can cause video stuttering

**Compression levels**: `none`, `low`, `medium`, `high`, `trash-compactor`

#### v2
Updates a fixed number of most significant pixels each frame. Uses CIELAB space to determine pixel significance. 

**Pros**:
- Fine-tuned control over bandwidth usage

**Cons**:
- Slow to compute
- Sometimes more artifacts/smearing in scene transitions/scenes with many moving objects

**Compression level**: Number specifying pixel-rate in kpx/s (1 kpx/s = 1024 pixels per second)


### JIT compression
By default, bad-apple-flut will generate the compressed data stream ahead-of-time in RAM to improve
streaming performance. This means bad-apple-flut may run out of memory for long/high-resolution 
videos.

To avoid this problem, or to reduce the amount of RAM bad-apple-flut uses in general, you can use 
the `--jit` flag to instead compress frames just-in-time. This will defer frame compression until 
right before the frame gets sent to the server. 


### Canvas 
If the chosen protocol supports it, a canvas can be specified with `--canvas <ID>` to target a
specific canvas on the server. 

| Protocol         | Supports multi-canvas | Canvas ID range |
| ---------------- | --------------------- | --------------- |
| `plaintext`      | ❌ No                 | N/A             |
| `bin-flutties`   | ✅ Yes                | 0 - 16          |
| `bin-flurry`     | ✅ Yes                | 0 - 255         |

## Configuration
The configuration file is stored in `<config_dir>/bad-apple-flut/config.toml`, where `<config_dir>` 
is the users config directory (see https://docs.rs/dirs/latest/dirs/fn.config_dir.html).

```toml
host = "localhost:1234"
send_threads = 8
compress_threads = 8
aot_frame_group_size = 100
protocol = "plaintext"
compression_algorithm = "v2"
```

- `host` — Address of the pixelflut server to send the video to.
- `send_threads` — Size of the thread pool used to send pixels to the server.
- `compress_threads` — Size of the thread pool used for ahead-of-time video compression. 
- `aot_frame_group_size` — Size of frame groups in ahead-of-time compression. Frame groups are
  processed in parallel by the thread pool. Smaller groups improve load-balancing, but introduces
  full frame blanks at group boundaries which increases spikes in network usage.
- `protocol` — Protocol to use. Must be supported by the server. Available protocols:
  - `plaintext` — Default plaintext TCP protocol (`PX xxxx yyyy rrggbb`)
  - `bin-flutties` — Binary protocol used by the [Flutties server](https://github.com/itepastra/flutties) (obsolete) (`(B0-BF) XHXL YHYL RR GG BB`)
  - `bin-flurry` — Binary protocol used by the [Flurry server](https://github.com/itepastra/flurry) (`80 (00-FF) XLXH YLYH RR GG BB`)

## Contributions
Contributions to the project are welcome, so feel free to suggest changes or report issues.

## Special thanks
- [patagona](https://github.com/patagonaa), for hosting the pixelflut server that gave birth to this 
  project
- The awsome people in the Crow Academy & solrunners Discord servers who also made pixelflut clients:
  - berries
  - ked
  - trintler
  - vym
  - pioli
  - zetty
  - skelly
  - noa
  - vesmir
- [Noa](https://github.com/itepastra), for making Flutties and Flurry, implementing the binary 
  protocol and reminding me that UnsafeCell exists.
- 

