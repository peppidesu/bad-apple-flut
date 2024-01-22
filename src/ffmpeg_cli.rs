use std::process::Command;

use crate::{Result, Error, paths, VideoMetadata};


pub fn get_video_framerate(input: &str) -> Result<f64> {
    
    // ffprobe -v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate infile
    let output = Command::new("ffprobe")
        .arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(input)
        .output()
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffprobe: {e}")
        ))?;        
    
    let output = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8 from console");    
    let first_line = output
        .lines().next().expect("No output from ffprobe");        

    let mut iter = first_line.split('/');    
    let numerator = iter.next()
        .unwrap() // unreachable
        .parse::<i32>()
        .expect(&format!("'{first_line}' is not a valid framerate"));
    
    let denominator = iter.next()
        .expect("Missing denominator")
        .parse::<i32>()
        .expect(&format!(""));

    Ok(numerator as f64 / denominator as f64)
}

pub fn extract_video_frames(input: &str, fps: f64, width: i32, height: i32) -> Result<VideoMetadata> {
    println!("Extracting frames to {} ...", paths::cache_frames().to_str().unwrap());
    
    if let Err(e) = std::fs::create_dir_all(paths::cache_frames()) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => {},
            _ => return Err(Error::Io(e))
        }
    }       
    
    // TODO: realtime progress
    // https://superuser.com/questions/1459810/how-can-i-get-ffmpeg-command-running-status-in-real-time

    let mut options = Command::new("ffmpeg");
    options.arg("-i");
    options.arg(input);
    options.arg("-vf");
    options.arg(format!("fps={fps},scale={width}:{height}"));
    options.arg(format!("{}/frame%d.ppm", paths::cache_frames().to_str().unwrap()));
    options.output()
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffmpeg{e}")
        ))?;
    
    let frame_count = std::fs::read_dir(paths::cache_frames())?.count();

    Ok(VideoMetadata::create(fps, frame_count))
}