use std::process::Stdio;
use tokio::{process::Command, io::{BufReader, AsyncBufReadExt}};
use crate::{Result, Error, paths, VideoMetadata};

pub async fn get_video_framerate(input: &str) -> Result<f64> {
    
    // ffprobe -v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate infile
    let output = Command::new("ffprobe")
        .arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(input)
        .output().await      
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffprobe: {e}")
        ))?;        
    
    let output = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8 from console");    
    let first_line = output
        .lines().next()
        .expect("No output from ffprobe");        

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

pub async fn extract_video_frames(input: &str, fps: f64, width: i32, height: i32) -> Result<VideoMetadata> {
    println!("Extracting frames to {} ...", paths::cache_frames().to_str().unwrap());
    
    if let Err(e) = std::fs::create_dir_all(paths::cache_frames()) {
        match e.kind() {
            std::io::ErrorKind::AlreadyExists => {},
            _ => return Err(Error::Io(e))
        }
    }       
    
    // TODO: realtime progress
    // https://superuser.com/questions/1459810/how-can-i-get-ffmpeg-command-running-status-in-real-time

    let mut cmd = Command::new("ffmpeg")
        .arg("-i")
        .arg(input)
        .arg("-vf")
        .arg(format!("fps={fps},scale={width}:{height}"))
        .arg("-progress").arg("-").arg("-nostats") // black magic
        
        .arg(format!("{}/frame%d.ppm", paths::cache_frames().to_str().unwrap()))
        .stdout(Stdio::piped())     
        .stderr(Stdio::piped())                   
        .spawn()
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffmpeg{e}")
        ))?;

    let stdout = cmd.stdout.take().unwrap();    

    let mut stdout_reader = BufReader::new(stdout);    
    
    let counter = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(1));        
    let counter_ref = counter.clone();

    let handle = tokio::spawn(async move {
        let mut line = String::new();
        loop {
            let len = stdout_reader.read_line(&mut line).await.unwrap();
            if len == 0 {
                break;
            }
            
            if line.starts_with("frame=") {                
                let count = line
                    .lines().next().unwrap()
                    .split("=").nth(1).unwrap()
                    .parse::<usize>().unwrap();
                counter_ref.store(count, std::sync::atomic::Ordering::Relaxed);
            }

            line.clear();
        }
    });

    let cursorpos = crossterm::cursor::position().unwrap();

    let print_over_line = |str: &str| {
        crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, cursorpos.1)).unwrap();
        // clear
        crossterm::execute!(std::io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::CurrentLine)).unwrap();
        crossterm::execute!(std::io::stdout(), crossterm::style::Print(str)).unwrap();            
    };

    while !handle.is_finished() {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let count = counter.load(std::sync::atomic::Ordering::Relaxed);
        print_over_line(&format!("{} frames extracted", count));
    }
    println!("\nDone.");

    let _ = cmd.wait().await
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffmpeg: {e}")
        ))?;
    
    
    let frame_count = std::fs::read_dir(paths::cache_frames())?.count();

    Ok(VideoMetadata::create(fps, frame_count))
}