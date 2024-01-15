use std::borrow::Borrow;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::net::TcpStream;
use std::ops::Deref;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use rayon::prelude::*;

use clap::{Parser, ValueEnum};

///////////////////////////////////////////////////////////////////////////

pub const HOST: &str = "pixelflut.uwu.industries:1234";
pub const FRAMES_DIR: &str = "cache/frames";
pub const CACHE_ID_PATH: &str = "cache/cache_id";
pub const THREAD_COUNT: usize = 12;

///////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    FileParseError(String),
    FFmpegError(String),
}

pub type Result<T> = core::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

///////////////////////////////////////////////////////////////////////////
 
pub trait VideoCompressor {
    fn compress(&self, frames: Vec<FrameFile>) -> FrameData;
}

///////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone, Copy, PartialEq, Eq,ValueEnum)]
pub enum CompressionLevel {
    AirstrikeMode,
    None,
    Low,
    Medium,
    High,
    TrashCompactor
}

impl CompressionLevel {
    pub fn luminance_treshold(&self) -> u16 {
        match self {
            Self::AirstrikeMode => 0,
            Self::None => 0,
            Self::Low => 5,
            Self::Medium => 10,
            Self::High => 20,
            Self::TrashCompactor => 64,
        }
    }
    pub fn chroma_threshold(&self, y: u8) -> u16 {
        let y = y as f32 / 255.0;
        let t = match self {
            Self::AirstrikeMode => 0.0,
            Self::None => 0.0,
            Self::Low => (1.0 - y) * 10.0,
            Self::Medium => (1.0 - y.powi(2) * 0.85) * 20.0,
            Self::High => (1.0 - y.powi(2) * 0.75) * 48.0,
            Self::TrashCompactor => 96.0,
        };
        t as u16
    }
    pub fn full_blank_interval(&self) -> usize {
        match self {
            Self::AirstrikeMode => 1,
            Self::None => 20,
            Self::Low => 50,
            Self::Medium => 100,
            Self::High => 500,
            Self::TrashCompactor => 2000,
        }
    }
}

///////////////////////////////////////////////////////////////////////////

#[derive(Parser)]
pub struct Args {
    #[clap(short, long)]
    input: String,

    #[clap(short)]
    x_offset: Option<usize>,

    #[clap(short)]
    y_offset: Option<usize>,
    
    #[clap(long)]
    width: Option<u32>,
    #[clap(long)]
    height: Option<u32>,
    
    #[clap(long)]
    nocache: bool,

    #[clap(long, default_value = "medium")]
    compression: CompressionLevel,

    #[clap(long)]
    debug: bool,
}

///////////////////////////////////////////////////////////////////////////

#[derive(Debug, Clone)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    data: Box<[Color]>,
}

#[derive(Debug, Clone)]
pub struct FrameFile {
    pub idx: usize,
    path: String
}

impl FrameFile {
    pub fn new(idx: usize) -> Self {
        let path = format!("{FRAMES_DIR}/frame{idx}.ppm");
        
        Self { idx, path }
    }
    pub fn load(&self) -> Result<Frame> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        
        let mut line = String::new();
        reader.read_line(&mut String::new())?; // skip P6
        reader.read_line(&mut line)?; // width + height        
        
        let mut iter = line.split_whitespace();
        
        let width = iter.next()
            .expect("Unreachable")
            .parse::<usize>()
            .map_err(|e| Error::FileParseError(e.to_string()))?;

        let height = iter.next()
            .ok_or(Error::FileParseError(
                "Unexpected end of line".to_string()
            ))?
            .parse::<usize>()
            .map_err(|e| Error::FileParseError(e.to_string()))?;
        
        reader.read_line(&mut String::new())?; // skip maxval

        let mut data = Vec::new();        
        reader.read_to_end(&mut data)?;

        let data = data.chunks(3)
            .map(|c| Color::new(c[0], c[1], c[2]))
            .collect::<Vec<_>>();

        Ok(Frame { width, height, data: data.into(), })        
    }
}

impl Frame {
    fn debug(width: usize, height: usize) -> Self {
        let data = vec![Color::new(128, 128, 128); width * height].into(); 
        Self { width, height, data }    
    }

    fn to_full_frame_data(&self) -> FrameData {
        FrameData::Full {
            width: self.width as u16, 
            height: self.height as u16, 
            data: self.data.to_vec() 
        }
    }
    fn apply_pixels(&self, pixels: &Vec<Pixel>) -> Self {
        let mut data = self.data.clone();
        for p in pixels {
            let i = p.y * self.width + p.x;
            data[i] = p.color;
        }
        Self {
            width: self.width,
            height: self.height,
            data,
        }
    }
    fn apply_frame_data(&self, data: &FrameData) -> Self {
        match data {
            FrameData::Delta(d) => self.apply_pixels(d),
            FrameData::Full { width: w, height: h, data: d } => Self {
                width: *w as usize,
                height: *h as usize,
                data: d.clone().into(),
            },
            FrameData::Empty => self.clone(),
        }
    }

    fn to_pixels(&self) -> Vec<Pixel> {
        self.data.into_par_iter()
            .enumerate()
            .map(|(i, v)| {
                let x = i % self.width;
                let y = i / self.width;
                Pixel { x, y, color: *v }
            })
            .collect()
    }
}

fn delta(old: &Frame, new: &Frame, args: &Args) -> FrameData {
    assert_eq!(old.width, new.width);
    assert_eq!(old.height, new.height);        
    
    let px_vec: Vec<_> = old.data.into_par_iter()
        .zip(new.data.into_par_iter())
        .enumerate()
        .filter_map(|(i, (old_val, new_val))| {

            // temporal chroma subsampling
            let (old_y, old_u, old_v) = old_val.to_yuv();
            let (new_y, new_u, new_v) = new_val.to_yuv();
            
            if old_y.abs_diff(new_y) as u16 > args.compression.luminance_treshold()
            || old_u.abs_diff(new_u) as u16 + old_v.abs_diff(new_v) as u16 > args.compression.chroma_threshold(old_y) {
                let x = i % old.width;
                let y = i / old.width;
                
                Some(Pixel { x, y, color: *new_val })
            } else {
                None
            }
        })
        .collect();

    if px_vec.len() == 0 {
        FrameData::Empty
    } else {
        FrameData::Delta(px_vec)
    }
}

///////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Color { pub r: u8, pub g: u8, pub b: u8 }

impl Color {
    #[inline]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    /// Converts RGB to YUV
    // https://en.wikipedia.org/wiki/Y%E2%80%B2UV#Conversion_to/from_RGB
    pub fn to_yuv(&self) -> (u8, u8, u8) {
        let r = self.r as f32; let g = self.g as f32; let b = self.b as f32;
        
        let l = r * 0.299    + g * 0.587   + b * 0.114;
        let u = r * -0.14713 - g * 0.28886 + b * 0.436   + 128.0;
        let v = r * 0.615    - g * 0.51499 - b * 0.10001 + 128.0;

        (l as u8, u as u8, v as u8)    
    }
}

///////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Pixel {
    pub x: usize,
    pub y: usize,
    pub color: Color
}

#[derive(Debug, Clone)]
pub enum FrameData {
    Delta(Vec<Pixel>),
    Full { width: u16, height: u16, data: Vec<Color> },
    Empty
}


impl Pixel {    
    fn to_pixelflut_string(&self, offset_x: usize, offset_y: usize) -> String {
        format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x+offset_x, self.y+offset_y, self.color.r, self.color.g, self.color.b)
    }
}

fn get_framerate_cli(args: &Args) -> Result<f64> {
    // ffprobe -v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate infile
    let output = std::process::Command::new("ffprobe")
        .arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(&args.input)
        .output()
        .expect("failed to execute ffprobe");
    
    let output = String::from_utf8(output.stdout)
        .expect("invalid UTF-8 from console");    
    let first_line = output
        .lines().next()
        .ok_or(Error::FFmpegError("No output from command".to_string()))?;

    let mut iter = first_line.split('/');    
    let numerator = iter.next()
        .expect("Unreachable")
        .parse::<i32>()
        .map_err(|_| Error::FFmpegError(
            format!("'{first_line}' is not a valid framerate")
        ))?;
    
    let denominator = iter.next()
        .ok_or(Error::FFmpegError(
            format!("'{first_line}' is not a valid framerate")
        ))?
        .parse::<i32>()
        .map_err(|_| Error::FFmpegError(
            format!("'{first_line}' is not a valid framerate")
        ))?;

    Ok(numerator as f64 / denominator as f64)
}

fn extract_video_frames_cli(args: &Args) -> Result<()> {
    println!("Extracting frames ...");
    std::fs::create_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});
    // ffmpeg -i infile out%d.ppm
    let mut options = std::process::Command::new("ffmpeg");
    options.arg("-i");
    options.arg(&args.input);
    options.arg("-vf");
    options.arg("fps=15");
    options.arg(format!("{FRAMES_DIR}/frame%d.ppm"));
    options.output()
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffmpeg{e}")
        ))?;
    Ok(())
}

fn compress_frame(last_frame: &Frame, new_frame: &Frame, idx: usize, args: &Args) -> FrameData {
    if matches!(args.compression, CompressionLevel::AirstrikeMode) 
    || idx % args.compression.full_blank_interval() == 0 { 
        new_frame.to_full_frame_data()
    } else {
        delta(last_frame, new_frame, args)
    }
}


fn progress_tracker(counter: Arc<std::sync::atomic::AtomicUsize>, max: usize, descr: String) -> JoinHandle<()> {    
    thread::spawn(move || {
        let last_count = counter.load(std::sync::atomic::Ordering::Relaxed);
        let start = std::time::Instant::now();
        let mut av_rate = 0.1;
        let mut warmup = 0;
        
        
        let cursorpos = crossterm::cursor::position().unwrap();

        let print_over_line = |str| {
            crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, cursorpos.1)).unwrap();
            // clear
            crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)).unwrap();
            crossterm::execute!(std::io::stdout(), crossterm::style::Print(str)).unwrap();            
        };        

        loop {
            let count = counter.load(std::sync::atomic::Ordering::Relaxed);
            warmup += 1;
            if count == max {
                println!("\nDone.");
                break;
            }
            if warmup < 5 {
                print_over_line(format!("{} / {} {}", count, max, descr));                
            }
            else {
                let elapsed = start.elapsed().as_secs_f64();
                
                let rate = (count - last_count) as f64 / elapsed;
                av_rate = av_rate * 0.95 + rate * 0.05;
                
                let eta = (max - count) as f64 / av_rate;
    
                let hrs = (eta / 3600.0) as i32;
                let mins = ((eta - hrs as f64 * 3600.0) / 60.0) as i32;
                let secs = (eta - hrs as f64 * 3600.0 - mins as f64 * 60.0) as i32;
                if hrs > 0 {
                    print_over_line(format!("{} / {} {} | ETA: {:}h{:02}m{:02}s", count, max, descr, hrs, mins, secs));
                } else if mins > 0 {
                    print_over_line(format!("{} / {} {} | ETA: {:}m{:02}s", count, max, descr, mins, secs));
                } else {
                    print_over_line(format!("{} / {} {} | ETA: {:}s", count, max, descr,  secs));
                }
            }
    
            
            thread::sleep(std::time::Duration::from_millis(500));
        }
    })
}

fn compress_frames_to_vec(args: &Args) -> Vec<FrameData> {
    println!("Compressing frames ...");
    
    
    let total_frames = std::fs::read_dir(FRAMES_DIR).unwrap().count();
    let frame_data_vec = Arc::new(Mutex::new(vec![FrameData::Empty; total_frames]));

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress_tracker(Arc::clone(&counter), total_frames, "frames compressed".to_string());

    let frame_files = (1..=total_frames)
        .into_par_iter()
        .map(|i| FrameFile::new(i))       
        .collect::<Vec<_>>();

    let chunks = frame_files.rchunks(THREAD_COUNT);

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    thread_pool.scope(|s| {
        for chunk in chunks {
            let counter = Arc::clone(&counter);
            let frame_data_vec = Arc::clone(&frame_data_vec);
            s.spawn(move |_| {
                let mut thread_frame_data_vec = Vec::new();
                let mut last_frame: Option<Frame> = None;
                for frame_file in chunk {
                    let frame = frame_file.load().unwrap();
                    let frame_data = match &last_frame {
                        Some(lf) => {
                            let data = compress_frame(&lf, &frame, frame_file.idx, &args);
                            last_frame = Some(lf.apply_frame_data(&data));
                            if args.debug {
                                let debug_frame = Frame::debug(frame.width, frame.height);
                                let debug_frame = debug_frame.apply_frame_data(&data);
                                debug_frame.to_full_frame_data()
                            }
                            else {
                                data
                            }
                        },
                        None => {
                            let data = frame.to_full_frame_data();
                            last_frame = Some(Frame {
                                width: frame.width,
                                height: frame.height,
                                data: frame.data.clone(),
                            });
                            data
                        }
                    };
                    thread_frame_data_vec.push(frame_data);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                frame_data_vec.lock().unwrap().splice(
                    chunk[0].idx-1..chunk[0].idx-1+thread_frame_data_vec.len(), 
                    thread_frame_data_vec
                );
            });
        }
    });

    progress.join().unwrap();
    
    Arc::try_unwrap(frame_data_vec)
        .expect("More than 1 strong reference (impossible)")
        .into_inner().unwrap()
}
fn gen_cache_id(args: &Args) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    args.input.hash(&mut hasher);
    args.width.hash(&mut hasher);
    args.height.hash(&mut hasher);
    hasher.finish()
}

fn is_cache_valid(args: &Args) -> std::io::Result<bool> {
    let hash = gen_cache_id(args);

    let cache_id = File::open(CACHE_ID_PATH)?;
    let mut reader = BufReader::new(cache_id);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let old_hash = line.parse::<u64>()
        .map_err(|_| std::io::Error::new(
            std::io::ErrorKind::InvalidData, "invalid cache id"
        ))?;

    Ok(hash == old_hash)
}

fn main() {   
    let args = Args::parse();
    if !is_cache_valid(&args).unwrap_or(false) 
    || !std::path::Path::new(FRAMES_DIR).exists()
    ||  args.nocache {
        std::fs::remove_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});                
        std::fs::remove_file(CACHE_ID_PATH).unwrap_or_else(|_| {});        
        
        extract_video_frames_cli(&args).unwrap();

        std::fs::write(CACHE_ID_PATH, gen_cache_id(&args).to_string()).unwrap();        
    }    

    let frame_rate = 15.0;
    let delay = (1000.0 / frame_rate) as u64;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    let frame_data_vec = compress_frames_to_vec(&args);

    println!("Playing video on {HOST}");
    loop {
        
        let stream = Arc::new(TcpStream::connect(HOST).unwrap());
        
        for frame_data in frame_data_vec.iter() {
            let frame_data = frame_data.to_owned();
            // sleep thread
            let sleep = thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(*&delay));
            });
    
            // get pixels
            let pixels = match frame_data {
                FrameData::Delta(d) => d,
                FrameData::Full { width: w, height: h, data: d } => {
                    let frame = Frame {
                        width: w as usize,
                        height: h as usize,
                        data: d.into_boxed_slice(),
                    };
                    frame.to_pixels()
                },
                FrameData::Empty => Vec::new()
            };
    
            let len = pixels.len();
            if len != 0 {
                
                // send pixels
                let msgs = pixels.into_par_iter()
                    .map(|p| p.to_pixelflut_string(
                        args.x_offset.unwrap_or(0), 
                        args.y_offset.unwrap_or(0))
                    )
                    .chunks(len.div_ceil(THREAD_COUNT))
                    .map(|c| c.join(""))
                    .collect::<Vec<_>>();
    
                thread_pool.scope(|s| {
                    for msg in msgs {
                        let stream = Arc::clone(&stream);
                        s.spawn(move |_| {
                            let mut writer = BufWriter::new(stream.as_ref());
                            writer.write_all(msg.as_bytes()).unwrap();
                            writer.flush().unwrap();
                        });
                    }
                });
            }
    
            sleep.join().unwrap();
        }
    }    
}