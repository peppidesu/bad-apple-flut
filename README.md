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
Usage: bad-apple-flut [OPTIONS]

Options:
  -i, --input <INPUT>
          Input file
      --target [<TARGET>]
          Target section from config file to use
      --host [<HOST>]
          Host to connect to
      --protocol <PROTOCOL>
          Protocol to use for sending frames [possible values: plaintext, bin-flutties, bin-flurry]
      --canvas <CANVAS>
          Target canvas (if supported)
  -x <X_OFFSET>
          Horizontal offset (in px)
  -y <Y_OFFSET>
          Vertical offset (in px)
      --width [<WIDTH>]
          Width (in px) [default: same as source]
      --height [<HEIGHT>]
          Height (in px) [default: same as source]
      --fps [<FPS>]
          Frame-rate (in fps) [default: same as source]
      --send-threads <SEND_THREADS>
          Number of threads to use for sending pixels
      --compress-threads <COMPRESS_THREADS>
          Number of threads to use for compressing frames
      --compression-algorithm <COMPRESSION_ALGORITHM>
          Compression algorithm to use [possible values: v1, v2]
      --compression-level <COMPRESSION_LEVEL>
          Compression level [none|low|medium|high|trash-compactor|number]
      --aot-frame-group-size <AOT_FRAME_GROUP_SIZE>
          Number of frames to group together when compressing ahead-of-time
      --nocache
          Ignore frame cache
      --jit
          Compress frames just-in-time
      --debug
          Enable debug output
  -h, --help
          Print help
  -V, --version
          Print version
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

#### Ahead-of-time frame group size
Frame groups are processed in parallel by the thread pool. The size of these groups is controlled by
the `aot_frame_group_size` option. Smaller groups improve load-balancing, but introduces full frame 
blanks at group boundaries which increases spikes in network usage.

A frame group size of 0 disables multithreading altogether.

### Canvas 
If the chosen protocol supports it, a canvas can be specified with `--canvas <ID>` to target a
specific canvas on the server. 

| Protocol         | Supports multi-canvas | Canvas ID range |
| ---------------- | --------------------- | --------------- |
| `plaintext`      | ❌ No                 | N/A             |
| `bin-flutties`   | ✅ Yes                | 0 - 16          |
| `bin-flurry`     | ✅ Yes                | 0 - 255         |

### Protocol
The protocol option defines the format in which pixels are sent to the server. The following protocols
are supported:
  - `plaintext` — Default plaintext TCP protocol (`PX xxxx yyyy rrggbb`)
  - `bin-flutties` — Binary protocol used by the [Flutties server](https://github.com/itepastra/flutties) (obsolete) (`(B0-BF) XHXL YHYL RR GG BB`)
  - `bin-flurry` — Binary protocol used by the [Flurry server](https://github.com/itepastra/flurry) (`80 (00-FF) XLXH YLYH RR GG BB`)


## Configuration
The configuration file is stored in `<config_dir>/bad-apple-flut/config.toml`, where `<config_dir>` 
is the users config directory (see https://docs.rs/dirs/latest/dirs/fn.config_dir.html).

```toml
[args]
target = example
## `target` overrides `host`, `protocol` and `canvas` specified in the `[args]` section
#host = "foo.bar.com:1234"
#protocol = "plaintext"
#canvas = 0

#x_offset = 0
#y_offset = 0
#width =
#height =
#fps =

send_threads = 4
compress_threads = 4

aot_frame_group_size = 100
compression_algorithm = "v2"
compression_level = "768"

#nocache = false
#jit = false
#debug = false

# Example target specification
[targets.example]
host = "pixelflut.example.com:1234"
protocol = "bin-flutties"
canvas = 1
```


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

