use std::sync::{mpsc, Arc, Mutex};          // multi-threading
use std::thread;                          
use std::time::{Duration, Instant};         // for timing requests
use chrono::{DateTime, Utc};                // timestamping for websites
use reqwest::blocking::Client;              
use std::fs::File;                          // file management
use std::io::BufWriter;                     // writing to file    
use std::env;                               // command-line arguments
use serde_json::json;                  

struct WebsiteStatus {                      // website info that will be printed to status.json file
    url: String,
    status: Result<u16, String>,
    response_time: Duration,
    timestamp: DateTime<Utc>,
}

enum Message {                              // for main thread and worker threads to communicate
    Job(String),
    Shutdown,
}

fn log_info(message: &str) {
    println!("[INFO] [{}] {}", Utc::now(), message);    // timestamp
}

fn log_error(message: &str) {
    eprintln!("[ERROR] [{}] {}", Utc::now(), message);
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut urls = vec![];
    let mut workers = num_cpus::get();
    let mut timeout = 5;
    let mut retries = 0;

    let mut file_mode = false;  // checks if websites are being read on file

    let mut i = 1;
    while i < args.len() {          // loops through all arguments and checks the flags: file, workers, timeout, and retries
        match args[i].as_str() {
            "--file" => {           // reads the file
                file_mode = true;
                if i + 1 < args.len() {
                    let filename = &args[i + 1];
                    match std::fs::read_to_string(filename) {
                        Ok(content) => {
                            for line in content.lines() {
                                if !line.starts_with('#') && !line.trim().is_empty() {
                                    urls.push(line.to_string());
                                }
                            }
                        },
                        Err(e) => {
                            log_error(&format!("Failed to read file '{}': {}", filename, e));
                            std::process::exit(1);
                        }
                    }
                    i += 1;
                }
            }
            "--workers" => {            // reads the number of workers
                if i + 1 < args.len() {
                    workers = args[i + 1].parse().unwrap_or_else(|_| {
                        log_error("Invalid worker count specified.");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            "--timeout" => {           // reads the timeout time
                if i + 1 < args.len() {
                    timeout = args[i + 1].parse().unwrap_or_else(|_| {
                        log_error("Invalid timeout value specified.");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            "--retries" => {            // reads the retries if a request fails
                if i + 1 < args.len() {
                    retries = args[i + 1].parse().unwrap_or_else(|_| {
                        log_error("Invalid retry count specified.");
                        std::process::exit(1);
                    });
                    i += 1;
                }
            }
            arg => {            // adds URLs to the vector. This is the default case
                if !file_mode {
                    urls.push(arg.to_string());
                }
            }
        }
        i += 1; // goes to the next argument
    }

    if urls.is_empty() {            // if no URLS, send error message
        log_error("No URLs provided. Usage: website_checker [--file <path>] [URL ...] [--workers N] [--timeout S] [--retries N]");
        std::process::exit(2);
    }

    log_info(&format!("Starting check with {} workers, {}s timeout, {} retries", workers, timeout, retries));

    let (tx, rx) = mpsc::channel();         // tx is transmitter that sends messages and rx is receiver that gets messages
    let rx = Arc::new(Mutex::new(rx));      // allows rx to be shared by threads
    let results = Arc::new(Mutex::new(vec![]));
    let mut handles = vec![];               // keeps track of all thread handles

    for _ in 0..workers {                   // makes worker threads
        let rx_clone = Arc::clone(&rx);
        let results_clone = Arc::clone(&results);
        let client = Client::new();
        let handle = thread::spawn(move || {
            loop {
                let message = {
                    let rx = rx_clone.lock().unwrap();
                    rx.recv()
                };

                match message {
                    Ok(Message::Job(url)) => {
                        println!("[DISPATCH] Sending job for URL: {}", url);
                        let now = Instant::now();
                        let mut attempts = 0;
                        let mut response;
                        loop {
                            response = client.get(&url).timeout(Duration::from_secs(timeout as u64)).send();    // tracks response time
                            match &response {
                                Ok(_) => break,
                                Err(e) if attempts < retries => {
                                    attempts += 1;
                                    thread::sleep(Duration::from_secs(2));
                                },
                                Err(_) => break,
                            }
                        }

                        let status = match response {
                            Ok(ref resp) => Ok(resp.status().as_u16()),    // successful
                            Err(e) => Err(format!("Request failed: {}", e.to_string())) // failed, writes down error
                        };

                        let duration = now.elapsed();
                        let record = WebsiteStatus {
                            url: url.clone(),
                            status,
                            response_time: duration,
                            timestamp: Utc::now(),
                        };

                        results_clone.lock().unwrap().push(record);
                    }
                    Ok(Message::Shutdown) | Err(_) => break,        // breaks thread
                }
            }
        });

        handles.push(handle);
    }

    for url in urls {
        tx.send(Message::Job(url)).unwrap();        // every website is an job for worker
    }

    for _ in 0..workers {
        tx.send(Message::Shutdown).unwrap();        // stops worker
    }

    for handle in handles {
        handle.join().expect("Thread panicked");
    }

    // JSON file writing
    let file = File::create("status.json").expect("Unable to create file");
    let writer = BufWriter::new(file); 

    let results = results.lock().unwrap();
    let json_results: Vec<_> = results.iter().map(|r| {
        json!({
            "url": r.url,
            "status": match &r.status {
                Ok(code) => code.to_string(),
                Err(err) => err.to_string()
            },
            "response_time_ms": r.response_time.as_millis(),
            "timestamp": r.timestamp.to_rfc3339(),
        })
    }).collect();

    serde_json::to_writer_pretty(writer, &json_results).expect("Failed to write JSON");

    log_info("All checks complete. Results written to status.json.");       // end message
}
