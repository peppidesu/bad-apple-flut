use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use rayon::prelude::*;



use serde::{Serialize, Deserialize};
use clap::{Parser, ValueEnum};
///////////////////////////////////////////////////////////////////////////

const HOST: &str = "pixelflut.uwu.industries:1234";
const FRAMES_DIR: &str = "cache/frames";
const THREAD_COUNT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq,ValueEnum)]
enum CompressionLevel {
    AirstrikeMode,
    None,
    Low,
    Medium,
    High,
    TrashCompactor,
}

impl CompressionLevel {
    fn luminance_treshold(&self) -> u16 {
        match self {
            Self::AirstrikeMode => 0,
            Self::None => 0,
            Self::Low => 5,
            Self::Medium => 10,
            Self::High => 20,
            Self::TrashCompactor => 64,
        }
    }
    fn chroma_threshold(&self, y: u8) -> u16 {
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
    fn full_blank_interval(&self) -> usize {
        match self {
            Self::AirstrikeMode => 1,
            Self::None => 20,
            Self::Low => 50,
            Self::Medium => 100,
            Self::High => 500,
            Self::TrashCompactor => 2000,
        }
    }
    fn quantize(&self, c: &Color) -> Color {
        if !matches!(self, Self::TrashCompactor) {
            return *c;
        }
        let (y,u,v) = rgb2yuv(c.0, c.1, c.2);
        let y = y >> 2 << 2;
        let u = u >> 4 << 4;
        let v = v >> 4 << 4;
        let (r,g,b) = yuv2rgb(y,u,v);
        Color(r,g,b)
    }
    
}

#[derive(Parser)]
struct Args {
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

#[derive(Debug, Clone)]
struct Frame {
    width: usize,
    height: usize,
    data: Box<[Color]>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
struct Color(u8, u8, u8);

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
struct Pixel {
    x: usize,
    y: usize,
    color: Color
}

#[derive(Debug, Serialize, Deserialize, Clone)]
enum FrameData {
    Delta(Vec<Pixel>),
    Full(u16,u16,Vec<Color>),
    Empty
}


fn rgb2yuv(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let u = -0.14713 * r as f32 - 0.28886 * g as f32 + 0.436 * b as f32;
    let v = 0.615 * r as f32 - 0.51499 * g as f32 - 0.10001 * b as f32;
    
    
    return (l as u8, (u+128.0) as u8, (v+128.0) as u8 )
    // the quick brwon
}

fn yuv2rgb(y: u8, u: u8, v: u8) -> (u8, u8, u8) {
    let r = y as f32 + 1.13983 * (v as f32 - 128.0);
    let g = y as f32 - 0.39465 * (u as f32 - 128.0) - 0.58060 * (v as f32 - 128.0);
    let b = y as f32 + 2.03211 * (u as f32 - 128.0);
    
    return (r as u8, g as u8, b as u8)
}



impl Frame {
    fn debug(width: usize, height: usize) -> Self {
        Self {
            width: width,
            height: height,
            data: vec![Color(128,128,128); width * height].into(),
        }
    }
    fn from_file(path: &str, level: &CompressionLevel) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();

        reader.read_line(&mut String::new())?; // skip P6
        reader.read_line(&mut line)?; // width + height        
        let mut iter = line.split_whitespace();
        let width = iter.next().unwrap().parse::<usize>().unwrap();
        let height = iter.next().unwrap().parse::<usize>().unwrap();
        
        reader.read_line(&mut String::new())?; // skip maxval

        let mut data = Vec::new();
        reader.read_to_end(&mut data)?;
        let data = data.chunks(3).map(|c| {
            let r = c[0];
            let g = c[1];
            let b = c[2];
            
            level.quantize(&Color(r,g,b))
        }).collect::<Vec<_>>();
        Ok(Self {
            width,
            height,
            data: data.into(),
        })
    }

    fn delta(old: &Self, new: &Self, args: &Args) -> Vec<Pixel> {
        assert_eq!(old.width, new.width);
        assert_eq!(old.height, new.height);        
        
        old.data.into_par_iter()
            .zip(new.data.into_par_iter())
            .enumerate()
            .filter_map(|(i, (old_val, new_val))| {

                // temporal chroma subsampling
                let (y1,u1,v1) = rgb2yuv(old_val.0, old_val.1, old_val.2);
                let (y2,u2,v2) = rgb2yuv(new_val.0, new_val.1, new_val.2);
                
                if y1.abs_diff(y2) as u16 > args.compression.luminance_treshold()
                || u1.abs_diff(u2) as u16 + v1.abs_diff(v2) as u16 > args.compression.chroma_threshold(y1) {
                    let x = i % old.width;
                    let y = i / old.width;
                    
                    Some(Pixel { x, y, color: *new_val })
                } else {
                    None
                }
            })
            .collect()
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
            FrameData::Full(w,h,d) => Self {
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

impl Pixel {    
    fn to_pixelflut_string(&self, offset_x: usize, offset_y: usize) -> String {
        format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x+offset_x, self.y+offset_y, self.color.0, self.color.1, self.color.2)
    }
}

fn get_framerate_cli(args: &Args) -> f64 {
    // ffprobe -v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate infile
    let output = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("0")
        .arg("-of")
        .arg("csv=p=0")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=r_frame_rate")
        .arg(&args.input)
        .output()
        .expect("failed to execute ffprobe");
    
    let output = String::from_utf8(output.stdout).unwrap();
    let output = output.lines().next().unwrap();

    let mut iter = output.split('/');    
    let numerator = iter.next().unwrap().parse::<i32>().unwrap();
    let denominator = iter.next().unwrap().parse::<i32>().unwrap();
    numerator as f64 / denominator as f64

}

fn extract_video_frames_cli(args: &Args) {
    println!("Extracting frames ...");
    std::fs::create_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});
    // ffmpeg -i infile out%d.ppm
    let mut options = std::process::Command::new("ffmpeg");
    options.arg("-i");
    options.arg(&args.input);
    options.arg("-vf");
    options.arg("fps=15");
    options.arg(format!("{FRAMES_DIR}/frame%d.ppm"));
    options.output().expect("failed to execute ffmpeg");

    options.status().expect("failed to execute ffmpeg");
}

fn compress_frame(last_frame: &Frame, new_frame: &Frame, idx: usize, args: &Args) -> FrameData {
    if matches!(args.compression, CompressionLevel::AirstrikeMode) {
        return FrameData::Full(
            new_frame.width as u16, 
            new_frame.height as u16, 
            new_frame.data.to_vec()
        );
    }
    if idx % args.compression.full_blank_interval() == 0 { // full frame every 100 frames to mitigate overwrites
        FrameData::Full(
            new_frame.width as u16, 
            new_frame.height as u16, 
            new_frame.data.to_vec()
        )
    } else {
        let data = Frame::delta(last_frame, new_frame, args);
        
        if data.len() == 0  {
            FrameData::Empty
        } else {
            FrameData::Delta(data)
        }
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
    
    let mut frame_data_vec = Vec::new();
    let mut last_frame: Option<Frame> = None;
    let total_frames = std::fs::read_dir(FRAMES_DIR).unwrap().count();

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress_tracker(Arc::clone(&counter), total_frames, "frames compressed".to_string());

    for i in 0..total_frames {
        let frame_id = i+1;
        let src_path = format!("{FRAMES_DIR}/frame{frame_id}.ppm");
        
        let frame = Frame::from_file(&src_path, &args.compression)
                        .expect(format!("failed to read frame {}", i).as_str());                    
        
        let frame_data = match &last_frame {
            Some(lf) => {
                let data = compress_frame(&lf, &frame, i, &args);
                last_frame = Some(lf.apply_frame_data(&data));
                if args.debug {
                    let debug_frame = Frame::debug(frame.width, frame.height);
                    let debug_frame = debug_frame.apply_frame_data(&data);
                    FrameData::Full(
                        debug_frame.width as u16, 
                        debug_frame.height as u16, 
                        debug_frame.data.to_vec()
                    )
                }
                else {
                    data
                }
            },
            None => {
                let data = FrameData::Full(frame.width as u16, frame.height as u16, frame.data.to_vec());
                last_frame = Some(Frame {
                    width: frame.width,
                    height: frame.height,
                    data: frame.data.clone(),
                });
                data
            }
        };
        counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        frame_data_vec.push(frame_data);
    }
    progress.join().unwrap();
    frame_data_vec
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

    let cache_id = File::open("cache_id")?;
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
        std::fs::remove_file("cache_id").unwrap_or_else(|_| {});        
        extract_video_frames_cli(&args);                
        std::fs::write("cache_id", gen_cache_id(&args).to_string()).unwrap();        
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
                FrameData::Full(w,h,d) => {
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