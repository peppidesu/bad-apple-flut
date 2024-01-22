use std::io::{BufWriter, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use rayon::{prelude::*, ThreadPool};

use bad_apple_flut::*;

struct Context {
    args: Args,
    config: Config,
    stream: Arc<TcpStream>,
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

fn compress_frames_to_vec(args: &Args, frame_count: usize) -> Vec<FrameData> {
    println!("Compressing frames ...");
            
    let frame_data_vec = Arc::new(Mutex::new(vec![FrameData::Empty; frame_count]));

    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let progress = progress_tracker(Arc::clone(&counter), frame_count, "frames compressed".to_string());

    let frame_files = (1..=frame_count)
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

fn send_frame(context: &Context, frame_data: &FrameData) {
    let pixels = frame_data.to_owned().to_pixels();
    let len = pixels.len();
    if len != 0 {                
        // send pixels
        let msgs = pixels.into_par_iter()
            .map(|p| p.to_pixelflut_string(
                context.args.x_offset, 
                context.args.y_offset)
            )
            .chunks(len.div_ceil(context.config.thread_count))
            .map(|c| c.join(""))
            .collect::<Vec<_>>();

        context.thread_pool.scope(|s| {
            for msg in msgs {
                let stream = Arc::clone(&context.stream);
                s.spawn(move |_| {
                    let mut writer = BufWriter::new(stream.as_ref());
                    writer.write_all(msg.as_bytes()).unwrap();
                    writer.flush().unwrap();
                });
            }
        });
    }
}

fn loop_just_in_time(context: &Context) -> Result<()> {
    let mut timer = FrameTimer::new(context.metadata.fps);
    let mut compressor = DeltaCompressorV1::new(
        context.args.compression.into(), 
        context.args.debug
    );
    loop {
        for i in 1..=context.metadata.frame_count {
            timer.start();
            let frame = FrameFile::new(i).load()?;
            let frame_data = compressor.compress_frame(&frame);            
            send_frame(&context, &frame_data);
            timer.wait();
        }
    }
}

fn loop_ahead_of_time(context: &Context, frames: Vec<FrameData>) -> Result<()> {
    let mut timer = FrameTimer::new(context.metadata.fps);
    loop {
        for frame_data in frames.iter() {
            timer.start();
            send_frame(&context, frame_data);
            timer.wait();
        }
    }
}

fn main() -> Result<()> {   
    let args = Args::parse();
    let config = Config::load()?;

    let cache_key = args.clone().into();

    if !is_cache_valid(&cache_key).unwrap_or(false) || args.nocache {
        clean_cache()?;       
        
        let metadata = extract_video_frames(
            &args.input,
            args.fps.unwrap_or(
                get_video_framerate(&args.input).unwrap()
            ),
            args.width.unwrap_or(-1),
            args.height.unwrap_or(-1)
        )?;
        
        metadata.write()?;

        write_cache_id(&cache_key)?;     
    }    
    let metadata = VideoMetadata::load()?;

    let thread_pool = rayon::ThreadPoolBuilder::new()
        .num_threads(config.thread_count)
        .build()
        .unwrap();
    
    let stream = Arc::new(TcpStream::connect(&config.host).unwrap());
    
    let context = Context {
        args,
        config,
        stream,
        metadata,
        thread_pool,
    };
    if context.args.jit {
        println!("Playing video on {}", context.config.host);
        loop_just_in_time(&context)?;
    } else {
        let frame_data_vec = compress_frames_to_vec(&context.args, context.metadata.frame_count);
        println!("Playing video on {}", context.config.host);
        loop_ahead_of_time(&context, frame_data_vec)?;
    }

    Ok(())
}