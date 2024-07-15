#![feature(sync_unsafe_cell)]
#![feature(ptr_as_ref_unchecked)]
use clap_serde_derive::ClapSerde;
use rayon::{prelude::*, ThreadPool};
use std::cell::SyncUnsafeCell;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use bad_apple_flut::*;
use colored::Colorize;

struct Context {
    args: Args,    
    stream: Option<Arc<Mutex<TcpStream>>>,
    metadata: VideoMetadata,
    thread_pool: ThreadPool,
}

struct FrameTimer {
    start: std::time::Instant,
    delay: u64,
    duration: u64,
    lag: u64,
    handle: Option<JoinHandle<()>>,
}

impl FrameTimer {
    pub fn new(fps: f64) -> Self {
        Self {
            start: std::time::Instant::now(),
            delay: (1000.0 / fps) as u64,
            duration: 0,
            lag: 0,
            handle: None,
        }
    }
    pub fn start(&mut self) {
        self.start = std::time::Instant::now();

        self.duration = self.delay.saturating_sub(self.lag);
        let duration = self.duration; // required bc of move into thread
        self.handle = Some(thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(duration));
        }));
        self.lag = self.lag.saturating_sub(self.delay);
    }
    pub fn wait(&mut self) {
        self.handle
            .take()
            .expect("wait() called before start()")
            .join()
            .unwrap();

        let end_time = std::time::Instant::now();
        let elapsed = end_time.duration_since(self.start).as_millis() as u64;
        self.lag += elapsed.saturating_sub(self.duration);
    }
}

fn compress_frames_to_vec(
    context: &Context,
    compressor: VideoCompressor,
) -> Result<Vec<FrameData>> {
    println!("{} {}", "::".blue(), "Compressing frames ...");

    let frame_data_vec = Arc::new(SyncUnsafeCell::new(vec![
        FrameData::Empty;
        context.metadata.frame_count
    ]));    

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let progress = progress_tracker(
        counter.clone(),
        context.metadata.frame_count,
        "frames compressed".to_string(),
    );

    let frame_files = (1..=context.metadata.frame_count)
        .into_par_iter()
        .map(|i| FrameFile::new(i))
        .collect::<Vec<_>>();
    
    let chunks = frame_files.chunks(
        match context.args.aot_frame_group_size {
            0 => frame_files.len(),
            n => n,
        }
    );

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(context.args.compress_threads)
        .build()
        .expect("Failed to create thread pool");

    let compressors = Arc::new(SyncUnsafeCell::new(vec![compressor.clone(); chunks.len()]));

    thread_pool.scope(|s| {
        let (err_tx, err_rx) = channel::<String>();
        let err_tx = Arc::new(Mutex::new(err_tx));
        let err_rx = Arc::new(Mutex::new(err_rx));

        for (i, chunk) in chunks.enumerate() {
            let counter = Arc::clone(&counter);
            let compressors = Arc::clone(&compressors);
            let frame_data_vec = Arc::clone(&frame_data_vec);
            let err_tx = Arc::clone(&err_tx);
            let err_rx = Arc::clone(&err_rx);

            if let Ok(e) = err_rx.lock().unwrap().try_recv() {
                return Err(Error::Custom(e));
            }

            s.spawn(move |_| {
                let mut thread_frame_data_vec = Box::new(Vec::new());
                let compressor = unsafe { &mut compressors.get().as_mut_unchecked()[i] };

                for frame_file in chunk {
                    if let Ok(_) = err_rx.lock().unwrap().try_recv() {
                        return;
                    }

                    let frame = frame_file.load();

                    if let Err(e) = frame {
                        err_tx
                            .lock()
                            .unwrap()
                            .send(format!("Failed to load frame {}: {}", frame_file.idx(), e))
                            .unwrap_or_else(|_| {});
                        return;
                    }

                    let frame_data = compressor.compress_frame(&frame.unwrap());
                    thread_frame_data_vec.push(frame_data);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                let frame_data_vec = unsafe { frame_data_vec.get().as_mut_unchecked() };
                frame_data_vec.splice(
                    chunk[0].idx() - 1..chunk[0].idx() - 1 + thread_frame_data_vec.len(),
                    thread_frame_data_vec.into_iter(),
                );
            });
        }

        Ok(())
    })?;

    progress.join().unwrap();

    Ok(Arc::try_unwrap(frame_data_vec)
        .expect("More than 1 strong reference (impossible)")
        .into_inner())
}

fn send_frame(context: &Context, frame_data: &FrameData) -> Result<()> {
    let pixels = frame_data.to_owned().to_pixels();
    let len = pixels.len();
    if len != 0 {
        // send pixels
        let msgs = pixels
            .par_chunks(400)
            .map(|chunk| {
                pixels_to_cmds(
                    context.args.protocol,
                    context.args.canvas,
                    chunk,
                    context.args.x_offset,
                    context.args.y_offset,
                )
            })
            .collect::<Vec<_>>();

        context.thread_pool.scope(|s| {
            let (err_tx, err_rx) = channel::<String>();
            let err_tx = Arc::new(Mutex::new(err_tx));

            for msg in msgs {
                if let Ok(e) = err_rx.try_recv() {
                    return Err(Error::Custom(e));
                }

                let stream = Arc::clone(&context.stream.as_ref().expect("Stream not initialized"));
                let err_tx = Arc::clone(&err_tx);
                s.spawn(move |_| {
                    let result = match &mut stream.lock() {
                        Ok(stream) => {
                            let success = stream.write_all(&msg);
                            match success {
                                Ok(_) => {
                                    stream.flush().unwrap();
                                    Ok(())
                                }
                                Err(e) => match e.kind() {
                                    std::io::ErrorKind::BrokenPipe => {
                                        Err("Unable to send frame: Connection closed by server"
                                            .to_string())
                                    }
                                    _ => Err("Unable to send frame: Unknown error".to_string()),
                                },
                            }
                        }
                        Err(_) => Err("Failed to lock stream".to_string()),
                    };

                    if let Err(e) = result {
                        err_tx.lock().unwrap().send(e).unwrap_or_else(|_| {
                            // error probably already sent, rx out of scope
                        });
                    }
                });
            }

            Ok(())
        })
    } else {
        Ok(())
    }
}

fn loop_just_in_time(context: &Context, mut compressor: VideoCompressor) -> Result<()> {
    let mut timer = FrameTimer::new(context.metadata.fps);
    loop {
        for i in 1..=context.metadata.frame_count {
            timer.start();
            let frame = FrameFile::new(i).load()?;
            let frame_data = compressor.compress_frame(&frame);
            send_frame(&context, &frame_data).unwrap_or_else(|e| {
                eprintln!("{}", e);
                std::process::exit(1);
            });
            timer.wait();
        }
    }
}

fn loop_ahead_of_time(context: &Context, frames: Vec<FrameData>) -> Result<()> {
    let mut timer = FrameTimer::new(context.metadata.fps);
    loop {
        for frame_data in frames.iter() {
            timer.start();
            send_frame(&context, &frame_data).unwrap_or_else(|e| {
                eprintln!("Error: {:?}", e);
                std::process::exit(1);
            });
            timer.wait();
        }
    }
}

pub fn verify_args(args: &Args) -> Result<()> {
    if args.width.is_some() && args.width.unwrap() < 0 {
        return Err(Error::InvalidArgs("--width must be positive".to_string()));
    }
    if args.height.is_some() && args.height.unwrap() < 0 {
        return Err(Error::InvalidArgs("--height must be positive".to_string()));
    }
    if args.fps.is_some() && args.fps.unwrap() <= 0.0 {
        return Err(Error::InvalidArgs(
            "--fps must be greater than 0.0".to_string(),
        ));
    }
    if args.input.is_empty() {
        return Err(Error::InvalidArgs("No input file specified".to_string()));
    }
    let path = std::path::Path::new(&args.input);
    if !path.exists() {
        return Err(Error::InvalidArgs(format!(
            "Input file '{}' does not exist",
            path.to_str().unwrap()
        )));
    }
    if !path.is_file() {
        return Err(Error::InvalidArgs(format!(
            "Input file '{}' is not a file",
            path.to_str().unwrap()
        )));
    }
    if args.send_threads == 0 {
        return Err(Error::InvalidConfig(
            "send_threads must be greater than 0".to_string(),
        ));
    }
    if args.compress_threads == 0 {
        return Err(Error::InvalidConfig(
            "compress_threads must be greater than 0".to_string(),
        ));
    }
    if args.host.is_none() && args.target.is_none() {
        return Err(Error::InvalidConfig(
            "host or target must be specified".to_string(),
        ));
    }    
    if args.aot_frame_group_size == 0 {
        return Err(Error::InvalidConfig(
            "aot_frame_group_size must be greater than 0".to_string(),
        ));
    }

    Ok(())
}

fn connect(host: &str) -> Arc<Mutex<TcpStream>> {
    let stream = TcpStream::connect(&host).unwrap_or_else(|e| {
        eprintln!("{} Failed to connect to {}: {}", "::".red(), host, e);
        std::process::exit(1);
    });

    Arc::new(Mutex::new(stream))
}

#[tokio::main]
async fn main() -> Result<()> {    
    let config = Config::load().unwrap_or_else(
        |e| {
            eprintln!("Failed to load config:\n{}", e.to_string().red());
            eprintln!(
                "Edit the config file at [{}] to fix the problem.", 
                paths::config_file().to_str().unwrap().cyan()
            );
            eprintln!(
                "If you recently updated bad-apple-flut, you may need to add missing fields to the config file. See the latest README for details."
            );            
            std::process::exit(1);
        }
    );

    let mut args = config.args.clone().merge_clap();

    verify_args(&args)?;

    match &args.target {
        Some(target) => {
            let target = config.targets.get(target).unwrap_or_else(|| {
                eprintln!("Target '{}' not found in config", target);
                std::process::exit(1);
            });            

            args.host = Some(target.host.clone());
            args.protocol = target.protocol.clone();
            args.canvas = target.canvas;
        }
        None => {}
    }

    let cache_key = args.clone().into();

    if !is_cache_valid(&cache_key).unwrap_or(false) || args.nocache {
        clean_cache()?;

        let metadata = extract_video_frames(
            &args.input,
            args.fps.unwrap_or(get_video_framerate(&args.input).await?),
            args.width.unwrap_or(-1),
            args.height.unwrap_or(-1),
        )
        .await?;

        metadata.write()?;

        write_cache_id(&cache_key)?;
    }

    let metadata = VideoMetadata::load()?;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(args.send_threads)
        .build()
        .unwrap();

    let mut context = Context {
        args,
        stream: None,
        metadata,
        thread_pool,
    };
    
    let mut compression_level = 
        CompressionLevelArg::try_from(context.args.compression_level.clone())
            .map_err(|e| Error::InvalidArgs(e.to_string()))?;
    
    if let CompressionLevelArg::Number(n) = compression_level {
        compression_level = CompressionLevelArg::Number(
            ((n * 1024) as f64 / context.metadata.fps) as usize
        ); 
    }

    let compressor = VideoCompressor::new(
        context.args.compression_algorithm.clone(),
        compression_level,
        context.args.debug
    )?;

    let host = context.args.host.clone().unwrap();
    
    if context.args.jit {
        context.stream = Some(connect(&host));
        println!("{} Playing video on {}", "::".blue(), host);
        loop_just_in_time(&context, compressor)?;
    } else {
        let frame_data_vec = compress_frames_to_vec(&context, compressor).unwrap_or_else(|e| {
            eprintln!("Error: {:?}", e);
            std::process::exit(1);
        });

        context.stream = Some(connect(&host));
        println!("{} Playing video on {}", "::".blue(), host);
        loop_ahead_of_time(&context, frame_data_vec)?;
    }

    Ok(())
}
