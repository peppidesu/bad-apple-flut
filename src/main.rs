use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;

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

struct Frame {
    width: usize,
    height: usize,
    data: Box<[Color]>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
struct Color(u8, u8, u8);

#[derive(Debug, Serialize, Deserialize)]
struct Pixel {
    x: usize,
    y: usize,
    v: Color
}

#[derive(Serialize, Deserialize)]
enum FrameData {
    Delta(Vec<Pixel>),
    Full(u16,u16,Vec<Color>),
    Empty
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
        let data = data.chunks(3).map(|c| Color(c[0], c[1], c[2])).collect::<Vec<_>>();
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
            if old_val.0.abs_diff(new_val.0) as usize > 20
            || old_val.1.abs_diff(new_val.1) as usize > 20
            || old_val.2.abs_diff(new_val.2) as usize > 20 {
                let x = i % old.width;
                let y = i / old.width;
                Some(Pixel { x, y, v: *new_val })
            } else {
                None
            }
        }).collect()
    } 
    fn to_pixels(&self) -> Vec<Pixel> {
        self.data.into_par_iter()
            .enumerate()
            .map(|(i, v)| {
                let x = i % self.width;
                let y = i / self.width;
                Pixel { x, y, v: *v }
            })
            .collect()
    }
}

impl Pixel {    
    fn to_pixelflut_string(&self, offset_x: usize, y: usize) -> String {
        format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x+offset_x, self.y+y, self.v.0, self.v.1, self.v.2)
    }
}

fn extract_video_frames(args: &Args) -> std::io::Result<()>{
    std::fs::create_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});

    ffmpeg::init()?;

    if let Ok(mut ictx) = ffmpeg::format::input(&args.input) {
        let input = ictx
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = input.index();

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
fn compress_frames_to_file() {
    std::fs::create_dir_all(COMP_FRAMES_DIR).unwrap_or_else(|_| {}); 

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let pthread_counter = Arc::clone(&counter);
    let frame_count = std::fs::read_dir(FRAMES_DIR).unwrap().count();
    let progress_thread = thread::spawn(move || {
        loop {
            let count = pthread_counter.load(std::sync::atomic::Ordering::Relaxed);
            println!("{} / {}", count, frame_count);
            if count == frame_count {
                break;
            }
            thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    let ranges = (0..frame_count).into_par_iter().chunks(100).collect::<Vec<_>>();

    rayon::scope(|s| {
        for idxs in ranges {
            let counter = Arc::clone(&counter);
            s.spawn(move |_| {
                let mut last_frame: Option<Frame> = None;
                for i in idxs {        
                    let old_path = format!("{FRAMES_DIR}/frame{i}.ppm");
                    let new_path = format!("{COMP_FRAMES_DIR}/frame{i}.bin");
                    let mut file = File::create(new_path).unwrap();
                    let frame = Frame::from_file(&old_path).expect(
                        format!("failed to read frame {}", i).as_str());            
            
                    let data = if i % 100 == 0 { // full frame every 100 frames to mitigate overwrites
                        FrameData::Full(frame.width as u16, frame.height as u16, frame.data.to_vec())
                    } else {
                        FrameData::Delta(last_frame.as_ref()
                        .map(|lf| Frame::delta(lf, &frame)).unwrap())
                    };
            
                    let len = match &data {
                        FrameData::Delta(d) => d.len(),
                        FrameData::Full(..,d) => d.len(),
                        FrameData::Empty => panic!("unreachable"),
                    };                
                    let data = if len == 0 {
                        FrameData::Empty            
                    }
                    else {
                        data
                    };
                    last_frame = Some(frame);
                    bincode::serialize_into(&mut file, &data).unwrap();
                    
                    // increment counter
                    let counter = Arc::clone(&counter);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
            });
        }
    });    
    progress_thread.join().unwrap();    
}
fn gen_cache_id() -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    "cache_id".hash(&mut hasher);
    hasher.finish()
}

fn is_cache_valid(filename: &str) -> std::io::Result<bool> {
    let hash = gen_cache_id();

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

const THREAD_COUNT: usize = 8;
fn main() {   
    let args = Args::parse();
    if !is_cache_valid(&args.input).unwrap_or(false) 
    || !std::path::Path::new("comp-frames").exists() 
    || !std::path::Path::new("frames").exists()
    ||  args.nocache {
        std::fs::remove_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});
        std::fs::remove_dir_all(COMP_FRAMES_DIR).unwrap_or_else(|_| {});
        std::fs::remove_file("cache_id").unwrap_or_else(|_| {});
        println!("extracting frames");
        extract_video_frames(&args).unwrap();
        println!("compressing frames");
        compress_frames_to_file();
        std::fs::write("cache_id", &args.input).unwrap();        
    }
    
    let frame_count = std::fs::read_dir(COMP_FRAMES_DIR).unwrap().count();



    loop {
        
        let stream = Arc::new(TcpStream::connect("pixelflut.uwu.industries:1234").unwrap());
        
        for i in 0..frame_count {
            let file = File::open(format!("comp-frames/frame{}.bin",i)).unwrap();
            let mut reader = BufReader::new(file);    
            // sleep thread
            let sleep = thread::spawn(|| {
                thread::sleep(std::time::Duration::from_millis(66));
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