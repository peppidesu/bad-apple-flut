use std::io::{BufWriter, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use rayon::prelude::*;

use bad_apple_flut::*;

pub fn progress_tracker(counter: Arc<std::sync::atomic::AtomicUsize>, max: usize, descr: String) -> JoinHandle<()> {    
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

pub fn compress_frames_to_vec(args: &Args) -> Vec<FrameData> {
    println!("Compressing frames ...");
        
    let total_frames = std::fs::read_dir(FRAMES_DIR).unwrap().count();
    let frame_data_vec = Arc::new(Mutex::new(vec![FrameData::Empty; total_frames]));

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress_tracker(Arc::clone(&counter), total_frames, "frames compressed".to_string());

    let frame_files = (1..=total_frames)
        .into_par_iter()
        .map(|i| FrameFile::new(i))       
        .collect::<Vec<_>>();

    let chunks = frame_files.chunks(200);

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    thread_pool.scope(|s| {
        for chunk in chunks {
            let counter = Arc::clone(&counter);
            let frame_data_vec = Arc::clone(&frame_data_vec);
            s.spawn(move |_| {
                let mut thread_frame_data_vec = Box::new(Vec::new());
                let mut compressor = DeltaCompressorV1::new(
                    args.compression.into(), 
                    args.debug
                );

                for frame_file in chunk {
                    let frame_data = compressor.compress_frame(&frame_file.load().unwrap());
                    thread_frame_data_vec.push(frame_data);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                
                frame_data_vec.lock().unwrap().splice(
                    chunk[0].idx()-1..chunk[0].idx()-1+thread_frame_data_vec.len(), 
                    thread_frame_data_vec.into_iter()
                );
            });
        }
    });

    progress.join().unwrap();
    
    Arc::try_unwrap(frame_data_vec)
        .expect("More than 1 strong reference (impossible)")
        .into_inner().unwrap()
}

fn main() {   
    let args = Args::parse();

    let cache_key = args.clone().into();

    if !is_cache_valid(&cache_key).unwrap_or(false) 
    || !std::path::Path::new(FRAMES_DIR).exists()
    ||  args.nocache {
        std::fs::remove_dir_all(FRAMES_DIR).unwrap_or_else(|_| {});                
        std::fs::remove_file(CACHE_ID_PATH).unwrap_or_else(|_| {});        
        
        extract_video_frames(
            &args.input,
            args.fps.unwrap_or(15.0),
            args.width.unwrap_or(-1),
            args.height.unwrap_or(-1)
        ).unwrap();

        std::fs::write(CACHE_ID_PATH, gen_cache_id(&cache_key).to_string()).unwrap();        
    }    

    let frame_rate = args.fps.unwrap_or(15.0);
    let delay = (1000.0 / frame_rate) as u64;
    let mut lag = 0;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(THREAD_COUNT)
        .build()
        .unwrap();

    let frame_data_vec = compress_frames_to_vec(&args);

    println!("Playing video on {}", config.host);
    loop {
        
        let stream = Arc::new(TcpStream::connect(&config.host).unwrap());
        
        for frame_data in frame_data_vec.iter() {
            // sleep thread
            let start_time = std::time::Instant::now();
            let frame_duration = delay.saturating_sub(lag);
            let sleep_thread = thread::spawn(move || {                
                thread::sleep(std::time::Duration::from_millis(frame_duration));
            });
            lag = lag.saturating_sub(delay);
            
            let frame_data = frame_data.to_owned();
            // get pixels
            let pixels = frame_data.to_pixels();
    
            let len = pixels.len();
            if len != 0 {                
                // send pixels
                let msgs = pixels.into_par_iter()
                    .map(|p| p.to_pixelflut_string(
                        args.x_offset.unwrap_or(0), 
                        args.y_offset.unwrap_or(0))
                    )
                    .chunks(len.div_ceil(config.thread_count)) // rchunks doesn't exist for par_iter, also we need the length anyway
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

            sleep_thread.join().unwrap();
            let end_time = std::time::Instant::now();
            let elapsed = end_time.duration_since(start_time).as_millis() as u64;
            lag += elapsed.saturating_sub(delay);            
        }
    }    
}