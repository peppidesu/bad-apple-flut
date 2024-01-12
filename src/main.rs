use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use ffmpeg_next as ffmpeg;
use ffmpeg::frame::Video;
use ffmpeg::software::scaling::{Flags, Context};

use rayon::prelude::*;



use serde::{Serialize, Deserialize};
use clap::Parser;
///////////////////////////////////////////////////////////////////////////

const HOST: &str = "pixelflut.uwu.industries:1234";
const FRAMES_DIR: &str = "cache/frames";
const COMP_FRAMES_DIR: &str = "cache/comp-frames";
const THREAD_COUNT: usize = 12;

#[derive(Parser)]
struct Args {
    #[clap(short, long)]
    input: String,
    #[clap(short)]
    x: Option<usize>,
    #[clap(short)]
    y: Option<usize>,
    #[clap(long)]
    nocache: bool,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Config {

}

fn rgb2yuv(r: u8, g: u8, b: u8) -> (u16, u16, u16) {
    let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let u = -0.14713 * r as f32 - 0.28886 * g as f32 + 0.436 * b as f32;
    let v = 0.615 * r as f32 - 0.51499 * g as f32 - 0.10001 * b as f32;
    
    
    return (l as u16, u as u16, v as u16)
}

impl Frame {
    fn from_file(path: &str) -> std::io::Result<Self> {
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
            
            // let l = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
            // let u = -0.14713 * r as f32 - 0.28886 * g as f32 + 0.436 * b as f32;
            // let v = 0.615 * r as f32 - 0.51499 * g as f32 - 0.10001 * b as f32;
            
            // // chroma quantization
            // let l = (l as u8 / 8) * 8;
            // let u = (u as u8 / 16) * 16;
            // let v = (v as u8 / 16) * 16;

            // let r = (l as f32 + 1.13983 * v as f32) as u8;
            // let g = (l as f32 - 0.39465 * u as f32 - 0.58060 * v as f32) as u8;
            // let b = (l as f32 + 2.03211 * u as f32) as u8;

            Color(r,g,b)
        }).collect::<Vec<_>>();
        Ok(Self {
            width,
            height,
            data: data.into(),
        })
    }

    fn delta(old: &Self, new: &Self) -> Vec<Pixel> {
        assert_eq!(old.width, new.width);
        assert_eq!(old.height, new.height);        

        old.data.into_par_iter()
            .zip(new.data.into_par_iter())
            .enumerate()
            .filter_map(|(i, (old_val, new_val))| {

                // temporal chroma subsampling
                let (y1,u1,v1) = rgb2yuv(old_val.0, old_val.1, old_val.2);
                let (y2,u2,v2) = rgb2yuv(new_val.0, new_val.1, new_val.2);
                
                if y1.abs_diff(y2) * 2 + u1.abs_diff(u2) + v1.abs_diff(v2) > 10 {
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
    fn to_pixelflut_string(&self, offset_x: usize, y: usize) -> String {
        format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x+offset_x, self.y+y, self.color.0, self.color.1, self.color.2)
    }
}

fn extract_video_frames(args: &Args) -> std::io::Result<()> {
    println!("Extracting frames ...");
    std::fs::create_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});

    ffmpeg::init()?;

    if let Ok(mut ictx) = ffmpeg::format::input(&args.input) {
        let input = ictx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();
        
        let framerate = input.avg_frame_rate();
        let framerate = framerate.numerator() as f64 / framerate.denominator() as f64;
        std::fs::write("cache/framerate", framerate.to_string())?;
        
        let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
        let mut decoder = context_decoder.decoder().video()?;

        let mut scaler = Context::get(
            decoder.format(),
            decoder.width(),
            decoder.height(),
            ffmpeg::format::Pixel::RGB24,
            decoder.width(),
            decoder.height(),
            Flags::BILINEAR,
        )?;

        let mut frame_index = 0;
        let mut file_index = 0;
        let mut receive_and_process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Video| -> std::io::Result<()> {
                let mut decoded = Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {                    
                    let mut frame = Video::empty();
                    scaler.run(&decoded, &mut frame)?;                                        
                    if frame_index % 2 == 0 {
                        frame_index += 1;
                        continue;
                    }
                    let mut file = File::create(format!("{FRAMES_DIR}/frame{file_index}.ppm"))?;                    
                    file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
                    file.write_all(frame.data(0))?;
                    frame_index += 1;
                    file_index += 1;
                }
                Ok(())
            };

        for (stream, packet) in ictx.packets() {
            if stream.index() == video_stream_index {
                decoder.send_packet(&packet)?;
                receive_and_process_decoded_frames(&mut decoder)?;
            }
        }
        decoder.send_eof()?;
        receive_and_process_decoded_frames(&mut decoder)?;
    }

    Ok(())
}

fn compress_frame(last_frame: &Frame, new_frame: &Frame, idx: usize) -> FrameData {
    if idx % 100 == 0 { // full frame every 100 frames to mitigate overwrites
        FrameData::Full(
            new_frame.width as u16, 
            new_frame.height as u16, 
            new_frame.data.to_vec()
        )
    } else {
        let data = Frame::delta(last_frame, new_frame);
        
        if data.len() == 0 {
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
        
        crossterm::execute!(std::io::stdout(), crossterm::cursor::Hide).unwrap();
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

fn compress_frames_to_file() {
    println!("Compressing frames ...");
    
    let mut frame_data_vec = Vec::new();
    let mut last_frame: Option<Frame> = None;
    let total_frames = std::fs::read_dir(FRAMES_DIR).unwrap().count();

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress_tracker(Arc::clone(&counter), total_frames, "frames compressed".to_string());

    for i in 0..total_frames {
        let src_path = format!("{FRAMES_DIR}/frame{i}.ppm");
        
        let frame = Frame::from_file(&src_path)
                        .expect(format!("failed to read frame {}", i).as_str());                    

        let frame_data = match &last_frame {
            Some(lf) => {
                let data = compress_frame(&lf, &frame, i);
                last_frame = Some(lf.apply_frame_data(&data));
                data
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


    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    println!("Writing frames to disk ...");

    std::fs::create_dir_all(COMP_FRAMES_DIR).unwrap_or_else(|_| {}); 
    
    let progress = progress_tracker(Arc::clone(&counter), total_frames, "frames written".to_string());

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    thread_pool.scope(|s| {
        for (i, frame_data) in frame_data_vec.into_iter().enumerate() {
            let counter = Arc::clone(&counter);
            s.spawn(move |_| {
                let mut file = File::create(format!("{COMP_FRAMES_DIR}/frame{i}.bin")).unwrap();
                bincode::serialize_into(&mut file, &frame_data).unwrap();
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            });
        }
    });   
    
    progress.join().unwrap();    
}
fn gen_cache_id(filename: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    filename.hash(&mut hasher);
    hasher.finish()
}

fn is_cache_valid(filename: &str) -> std::io::Result<bool> {
    let hash = gen_cache_id(filename);

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
    if !is_cache_valid(&args.input).unwrap_or(false) 
    || !std::path::Path::new(COMP_FRAMES_DIR).exists() 
    || !std::path::Path::new(FRAMES_DIR).exists()
    ||  args.nocache {
        std::fs::remove_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});
        std::fs::remove_dir_all(COMP_FRAMES_DIR).unwrap_or_else(|_| {});
        std::fs::remove_file("cache_id").unwrap_or_else(|_| {});        
        extract_video_frames(&args).unwrap();        
        compress_frames_to_file();
        std::fs::write("cache_id", gen_cache_id(&args.input).to_string()).unwrap();        
    }
    
    let frame_count = std::fs::read_dir(COMP_FRAMES_DIR).unwrap().count();

    let frame_rate = std::fs::read_to_string("cache/framerate").unwrap().parse::<f64>().unwrap() * 0.5;
    let delay = (1000.0 / frame_rate) as u64;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    println!("Playing video on {HOST}");
    loop {
        
        let stream = Arc::new(TcpStream::connect(HOST).unwrap());
        
        for i in 0..frame_count {
            let file = File::open(format!("{COMP_FRAMES_DIR}/frame{}.bin",i)).unwrap();
            let mut reader = BufReader::new(file);    
            // sleep thread
            let sleep = thread::spawn(move || {
                thread::sleep(std::time::Duration::from_millis(*&delay));
            });
    
            // read frame
            let frame_data: FrameData = bincode::deserialize_from(&mut reader).unwrap();
            
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
                        args.x.unwrap_or(0), 
                        args.y.unwrap_or(0))
                    )
                    .chunks(len.div_ceil(THREAD_COUNT))
                    .map(|c| c.join(""))
                    .collect::<Vec<_>>();
    
                rayon::scope(|s| {
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