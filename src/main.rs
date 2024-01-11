use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write, Read};
use std::net::TcpStream;
use std::sync::Arc;
use std::thread;

use ffmpeg_next as ffmpeg;
use ffmpeg::frame::Video;
use ffmpeg::software::scaling::{Flags, Context};

use rayon::prelude::*;

use serde::{Serialize, Deserialize};

struct Frame {
    width: usize,
    height: usize,
    data: Box<[u8]>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Pixel {
    x: usize,
    y: usize,
    v: u8,
}

#[derive(Serialize, Deserialize)]
enum FrameData {
    Delta(Vec<Pixel>),
    Full(Vec<u8>),
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
            if old_val != new_val {
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
    fn to_string(&self) -> String {
        format!("PX {} {} {:02x}{:02x}{:02x}\n", self.x+600, self.y, self.v, self.v, self.v)
    }
}

fn extract_video_frames() -> std::io::Result<()>{
    ffmpeg::init()?;

    if let Ok(mut ictx) = ffmpeg::format::input(&std::env::args().nth(1).expect("Please provide an input file to generate frame data")) {
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

        let mut receive_and_process_decoded_frames =
            |decoder: &mut ffmpeg::decoder::Video| -> std::io::Result<()> {
                let mut decoded = Video::empty();
                while decoder.receive_frame(&mut decoded).is_ok() {
                    let mut frame = Video::empty();
                    scaler.run(&decoded, &mut frame)?;
                    
                    std::fs::create_dir("frames").unwrap_or_else(|_| {});
                    let mut file = File::create(format!("frames/frame{}.ppm", frame_index))?;

                    file.write_all(format!("P6\n{} {}\n255\n", frame.width(), frame.height()).as_bytes())?;
                    file.write_all(frame.data(0))?;
                    frame_index += 1;
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
    std::fs::create_dir("comp-frames").unwrap_or_else(|_| {}); 
    (0..6571).into_par_iter().chunks(100).for_each(|idxs| {
        let mut last_frame: Option<Frame> = None;
        for i in idxs {        
            let old_path = format!("frames/frame{}.ppm", i);
            let new_path = format!("comp-frames/frame{}.bin", i);
            let mut file = File::create(new_path).unwrap();
            let frame = Frame::from_file(&old_path).unwrap();            
    
            let data = if i % 100 == 0 { // full frame every 100 frames to mitigate overwrites
                FrameData::Full(frame.data.to_vec())
            } else {
                FrameData::Delta(last_frame.as_ref()
                .map(|lf| Frame::delta(lf, &frame)).unwrap())
            };
    
            let len = match &data {
                FrameData::Delta(d) => d.len(),
                FrameData::Full(d) => d.len(),
                FrameData::Empty => panic!("unreachable"),
            };    
    
            println!("frame: {}", i);
            let data = if len == 0 {
                FrameData::Empty            
            }
            else {
                data
            };
            last_frame = Some(frame);
            bincode::serialize_into(&mut file, &data).unwrap();
        }
    }); 
        
    std::fs::remove_dir_all("frames").unwrap_or_else(|_| {});
}



const THREAD_COUNT: usize = 8;
fn main() {   
    
    if !std::path::Path::new("comp-frames").exists() {
        if !std::path::Path::new("frames").exists() {
            println!("extracting frames");
            extract_video_frames().unwrap();
        }
        println!("applying temporal frame compression");
        thread::sleep(std::time::Duration::from_millis(1000));
        compress_frames_to_file();
    }
    loop {
        
        let stream = Arc::new(TcpStream::connect("pixelflut.uwu.industries:1234").unwrap());
        
        for i in 0..6571 {
            let file = File::open(format!("comp-frames/frame{}.bin",i)).unwrap();
            let mut reader = BufReader::new(file);    
            // sleep thread
            let sleep = thread::spawn(|| {
                thread::sleep(std::time::Duration::from_millis(33));
            });
    
            // read frame
            let frame_data: FrameData = bincode::deserialize_from(&mut reader).unwrap();
            
            // get pixels
            let pixels = match frame_data {
                FrameData::Delta(d) => d,
                FrameData::Full(d) => {
                    let frame = Frame {
                        width: 480,
                        height: 360,
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
                    .map(|p| p.to_string())
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
