#![feature(sync_unsafe_cell)]
use std::cell::SyncUnsafeCell;
use std::io::Write;
use std::net::TcpStream;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use rayon::{prelude::*, ThreadPool};

use bad_apple_flut::*;

struct Context {
    args: Args,
    config: Config,
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
        self.handle.take()
            .expect("wait() called before start()")
            .join().unwrap();

        let end_time = std::time::Instant::now();
        let elapsed = end_time.duration_since(self.start).as_millis() as u64;
        self.lag += elapsed.saturating_sub(self.duration);            
    }
}


fn compress_frames_to_vec(context: &Context, compressor: VideoCompressor) -> Vec<FrameData> {
    
    println!("Compressing frames ...");
            
    let frame_data_vec = Arc::new(
        SyncUnsafeCell::new(
            vec![FrameData::Empty; context.metadata.frame_count]
        )
    );

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let progress = progress_tracker(
        counter.clone(), 
        context.metadata.frame_count, 
        "frames compressed".to_string()
    );

    let frame_files = (1..=context.metadata.frame_count)
        .into_par_iter()
        .map(|i| FrameFile::new(i))       
        .collect::<Vec<_>>();

    
    let chunks = frame_files.chunks(500);

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(context.config.thread_count)
        .build()
        .unwrap();    

    let compressors = Arc::new(
        SyncUnsafeCell::new(
            vec![compressor.clone(); chunks.len()]
        )
    );

    thread_pool.scope(|s| {
        for (i, chunk) in chunks.enumerate() {
            let counter = Arc::clone(&counter);
            let compressors = Arc::clone(&compressors);
            let frame_data_vec = Arc::clone(&frame_data_vec);
            s.spawn(move |_| {
                let mut thread_frame_data_vec = Box::new(Vec::new());
                let compressor = unsafe { &mut compressors.get().as_mut().unwrap()[i] };

                for frame_file in chunk {
                    let frame_data = compressor.compress_frame(&frame_file.load().unwrap());
                    thread_frame_data_vec.push(frame_data);
                    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }

                let frame_data_vec = unsafe { frame_data_vec.get().as_mut().unwrap() };
                frame_data_vec.splice(
                    chunk[0].idx()-1..chunk[0].idx()-1+thread_frame_data_vec.len(), 
                    thread_frame_data_vec.into_iter()
                );
            });
        }
    });

    progress.join().unwrap();
    
    Arc::try_unwrap(frame_data_vec)
        .expect("More than 1 strong reference (impossible)")
        .into_inner()   
}

fn send_frame(context: &Context, frame_data: &FrameData) -> Result<()> {
    let pixels = frame_data.to_owned().to_pixels();
    let len = pixels.len();
    if len != 0 {                
        // send pixels
        let msgs = pixels.par_chunks(200)
            .map(|chunk| pixels_to_cmds(
                context.config.protocol,
                chunk, 
                context.args.x_offset, 
                context.args.y_offset
            ))
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
                                        Err("Unable to send frame: Connection closed by server".to_string())
                                    }
                                    _ => Err("Unable to send frame: Unknown error".to_string())
                                }
                            }                    
                        }
                        Err(_) => Err("Failed to lock stream".to_string())
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
            });
            timer.wait();
        }
    }
}

pub fn verify_args(args: &Args) -> Result<()> {
    if args.width.is_some() && args.width.unwrap() < 0 {
        return Err(Error::InvalidArgs(
            "--width must be positive".to_string()
        ));
    }
    if args.height.is_some() && args.height.unwrap() < 0 {
        return Err(Error::InvalidArgs(
            "--height must be positive".to_string()
        ));
    }
    if args.fps.is_some() && args.fps.unwrap() <= 0.0 {
        return Err(Error::InvalidArgs(
            "--fps must be greater than 0.0".to_string()
        ));
    }
    if args.input.is_empty() {
        return Err(Error::InvalidArgs(
            "No input file specified".to_string()
        ));
    }
    let path = std::path::Path::new(&args.input);
    if !path.exists() {
        return Err(Error::InvalidArgs(
            format!("Input file '{}' does not exist", path.to_str().unwrap())
        ));
    }
    if !path.is_file() {
        return Err(Error::InvalidArgs(
            format!("Input file '{}' is not a file", path.to_str().unwrap())
        ));
    }
    
    Ok(())
}
pub fn verify_config(config: &Config) -> Result<()> {
    if config.thread_count == 0 {
        return Err(Error::InvalidConfig(
            "Thread count must be greater than 0".to_string()
        ));
    }
    if config.host.is_empty() {
        return Err(Error::InvalidConfig(
            "Host must not be empty".to_string()
        ));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {   
    let args = Args::parse();
    let config = Config::load()?;

    verify_args(&args)?;
    verify_config(&config).unwrap_or_else(|e| {
        eprintln!("Invalid config: {:?}", e);
    });        

    let cache_key = args.clone().into();

    if !is_cache_valid(&cache_key).unwrap_or(false) || args.nocache {
        clean_cache()?;       
        
        let metadata = extract_video_frames(
            &args.input,
            args.fps.unwrap_or(
                get_video_framerate(&args.input).await?
            ),
            args.width.unwrap_or(-1),
            args.height.unwrap_or(-1)
        ).await?;
        
        metadata.write()?;

        write_cache_id(&cache_key)?;     
    }    
    let metadata = VideoMetadata::load()?;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.thread_count)
        .build()
        .unwrap();
    
    let mut context = Context {
        args,
        config,
        stream: None,
        metadata,
        thread_pool,
    };

    let compressor = VideoCompressor::new(
        context.config.compression_algorithm.clone(),
        CompressionLevelArg::try_from(context.args.compression.clone())
            .map_err(|e| Error::InvalidArgs(e.to_string()))?,
        context.args.debug
    )?;

    if context.args.jit {        
        println!("{}", context.config.host);
        context.stream = Some(
            Arc::new(
                Mutex::new(
                    TcpStream::connect(&context.config.host).unwrap()
                )
            )
        );                           
        println!("Playing video on {}", context.config.host);
        loop_just_in_time(&context, compressor)?;
    } else {
        let frame_data_vec = compress_frames_to_vec(&context, compressor);        
        context.stream = Some(
            Arc::new(
                Mutex::new(
                    TcpStream::connect(&context.config.host).unwrap()
                )
            )
        );  
        println!("Playing video on {}", context.config.host);
        loop_ahead_of_time(&context, frame_data_vec)?;
    }

    Ok(())
}