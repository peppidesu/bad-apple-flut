use crate::{Result, Error, FRAMES_DIR};

pub fn get_video_framerate(input: &str) -> Result<f64> {
    // ffprobe -v 0 -of csv=p=0 -select_streams v:0 -show_entries stream=r_frame_rate infile
    let output = std::process::Command::new("ffprobe")
        .arg("-v").arg("0")
        .arg("-of").arg("csv=p=0")
        .arg("-select_streams").arg("v:0")
        .arg("-show_entries").arg("stream=r_frame_rate")
        .arg(input)
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

pub fn extract_video_frames(input: &str, fps: f64, width: i32, height: i32) -> Result<()> {
    println!("Extracting frames ...");
    std::fs::create_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});
    // ffmpeg -i infile out%d.ppm
    let mut options = std::process::Command::new("ffmpeg");
    options.arg("-i");
    options.arg(input);
    options.arg("-vf");
    options.arg(format!("fps={fps},scale={width}:{height}"));
    options.arg(format!("{FRAMES_DIR}/frame%d.ppm"));
    options.output()
        .map_err(|e| Error::FFmpegError(
            format!("failed to execute ffmpeg{e}")
        ))?;
    Ok(())
}