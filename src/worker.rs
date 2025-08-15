use crate::{job::Job, share::Share};
use randomx_rs::{RandomXVM, RandomXFlag};
use std::{
    num::NonZeroUsize,
    sync::mpsc::{self, Receiver, TryRecvError},
    thread,
    time::Duration,
};
use watch::WatchSender;

pub struct Worker {
    share_rx: Receiver<Share>,
    job_tx: WatchSender<Job>,
}

impl Worker {
    #[tracing::instrument(skip(job))]
    pub fn init(job: Job, num_threads: NonZeroUsize, fast: bool, debug_all: bool, debug_hash_log: bool) -> Self {
        let (share_tx, share_rx) = mpsc::channel();
        let (job_tx, job_rx) = watch::channel(job.clone());
        let light_mode = !fast;
        
        
        for i in 0..num_threads.get() {
            let share_tx = share_tx.clone();
            let mut job_rx = job_rx.clone();
            
            let worker_light_mode = light_mode;
            thread::spawn(move || {
                let span = tracing::info_span!("thread", id = i);
                let _enter = span.enter();
                
                let mut vm: Option<RandomXVM> = None;
                let mut cache: Option<randomx_rs::RandomXCache> = None;
                let mut dataset: Option<randomx_rs::RandomXDataset> = None;
                let mut current_seed: Vec<u8> = Vec::new();
                let mut blob: Vec<u8> = Vec::new();
                let mut difficulty: u64 = 0;
                let mut job_id: String = String::new();
                let light_mode = worker_light_mode;
                
                
                let mut flags = RandomXFlag::get_recommended_flags();
                flags.insert(RandomXFlag::FLAG_LARGE_PAGES);
                flags.insert(RandomXFlag::FLAG_FULL_MEM);
                
                let thread_flags = flags;
                let mut flags = thread_flags;
                
                let debug_all = debug_all;
                let debug_hash_log = debug_hash_log;
                
                
                let thread_offset = i as u32;
                let thread_step = num_threads.get() as u32;
                let mut nonce_counter: u32 = thread_offset;
                
                #[repr(align(64))]
                struct AlignedBuffer([u8; 4]);
                let mut aligned_nonce = AlignedBuffer([0u8; 4]);
                
                let initial_job = job_rx.get();
                if !initial_job.seed.is_empty() {
                    current_seed = initial_job.seed.clone();
                    
                    let cache_result = randomx_rs::RandomXCache::new(flags, &current_seed);
                    cache = match cache_result {
                        Ok(c) => {
                            Some(c)
                        },
                        Err(e) => {
                            let mut fallback_flags = flags;
                            fallback_flags.remove(RandomXFlag::FLAG_LARGE_PAGES);
                            match randomx_rs::RandomXCache::new(fallback_flags, &current_seed) {
                                Ok(c) => {
                                    flags = fallback_flags;
                                    Some(c)
                                },
                                Err(_e2) => {
                                    eprintln!("ERROR: Thread {} - Failed to create RandomXCache even without large pages", i);
                                    return;
                                }
                            }
                        }
                    };
                    
                        if let Some(ref cache_ref) = cache {
                            let dataset_result = randomx_rs::RandomXDataset::new(flags, cache_ref.clone(), 0);
                            dataset = match dataset_result {
                                Ok(d) => Some(d),
                                Err(e) => {
                                    let mut fallback_flags = flags;
                                    fallback_flags.remove(RandomXFlag::FLAG_FULL_MEM);
                                    if let Ok(d) = randomx_rs::RandomXDataset::new(fallback_flags, cache_ref.clone(), 0) {
                                        flags = fallback_flags;
                                        Some(d)
                                    } else {
                                        return;
                                    }
                                }
                            };
                        
                        if let Some(ref dataset_ref) = dataset {
                            let vm_result = randomx_rs::RandomXVM::new(flags, Some(cache_ref.clone()), Some(dataset_ref.clone()));
                            match vm_result {
                                Ok(new_vm) => {
                                    vm = Some(new_vm);
                                },
                                Err(e) => {
                                    eprintln!("ERROR: Thread {} - Failed to create RandomXVM: {}", i, e);
                                    let mut fallback_flags = flags;
                                    fallback_flags.remove(RandomXFlag::FLAG_LARGE_PAGES);
                                    let vm_result = randomx_rs::RandomXVM::new(fallback_flags, Some(cache_ref.clone()), Some(dataset_ref.clone()));
                                    match vm_result {
                                        Ok(new_vm) => {
                                            vm = Some(new_vm);
                                        },
                                        Err(_e2) => {
                                            eprintln!("ERROR: Thread {} - Failed to create RandomXVM even with fallback flags", i);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    blob = initial_job.blob.clone();
                    difficulty = initial_job.difficulty();
                    job_id = initial_job.id.clone();
                    nonce_counter = thread_offset;
                }
                
                
                loop {
                    
                    if let Some(new_job) = job_rx.get_if_new() {
                        if current_seed != new_job.seed {
                            current_seed = new_job.seed.clone();
                            
                            let new_cache_result = randomx_rs::RandomXCache::new(flags, &current_seed);
                            let new_cache = match new_cache_result {
                                Ok(c) => c,
                                Err(e) => {
                                    eprintln!("ERROR: Thread {} - Failed to create new RandomXCache: {}", i, e);
                                    continue;
                                }
                            };
                            
                            if let Some(ref mut vm_ref) = vm {
                                if let Err(e) = vm_ref.reinit_cache(new_cache.clone()) {
                                    eprintln!("ERROR: Thread {} - Failed to reinitialize VM cache: {}", i, e);
                                    let vm_result = randomx_rs::RandomXVM::new(flags, Some(new_cache.clone()), dataset.clone());
                                    match vm_result {
                                        Ok(new_vm) => {
                                            vm = Some(new_vm);
                                        },
                                        Err(e2) => {
                                            eprintln!("ERROR: Thread {} - Failed to recreate RandomXVM after reinit_cache failure: {}", i, e2);
                                            continue;
                                        }
                                    }
                                }
                            } else {
                                let vm_result = randomx_rs::RandomXVM::new(flags, Some(new_cache.clone()), dataset.clone());
                                match vm_result {
                                    Ok(new_vm) => {
                                        vm = Some(new_vm);
                                    },
                                    Err(e) => {
                                        eprintln!("ERROR: Thread {} - Failed to create RandomXVM with new cache: {}", i, e);
                                        continue;
                                    }
                                }
                            }
                            
                            cache = Some(new_cache.clone());
                            
                            if flags.contains(RandomXFlag::FLAG_FULL_MEM) {
                                if let Some(ref cache_ref) = cache {
                                    let new_dataset_result = randomx_rs::RandomXDataset::new(flags, cache_ref.clone(), 0);
                                    let new_dataset = match new_dataset_result {
                                        Ok(d) => Some(d),
                                        Err(e) => {
                                            eprintln!("ERROR: Thread {} - Failed to create new RandomXDataset: {}", i, e);
                                            let mut fallback_flags = flags;
                                            fallback_flags.remove(RandomXFlag::FLAG_FULL_MEM);
                                            if let Ok(d) = randomx_rs::RandomXDataset::new(fallback_flags, cache_ref.clone(), 0) {
                                                flags = fallback_flags;
                                                Some(d)
                                            } else {
                                                eprintln!("ERROR: Thread {} - Failed to create RandomXDataset even in cache-only mode", i);
                                                continue;
                                            }
                                        }
                                    };
                                    
                                    if let Some(ref mut vm_ref) = vm {
                                        if let Some(ds) = new_dataset.clone() {
                                            if let Err(e) = vm_ref.reinit_dataset(ds) {
                                                eprintln!("ERROR: Thread {} - Failed to reinitialize VM dataset: {}", i, e);
                                                let vm_result = randomx_rs::RandomXVM::new(flags, cache.clone(), new_dataset.clone());
                                                match vm_result {
                                                    Ok(new_vm) => {
                                                        vm = Some(new_vm);
                                                    },
                                                    Err(e2) => {
                                                        eprintln!("ERROR: Thread {} - Failed to recreate RandomXVM after reinit_dataset failure: {}", i, e2);
                                                        continue;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    dataset = new_dataset;
                                }
                            }
                        }
                        
                        blob = new_job.blob.clone();
                        difficulty = new_job.difficulty();
                        job_id = new_job.id.clone();
                        nonce_counter = thread_offset;
                    }
                    
                    if let Some(ref vm) = vm {
                        const BATCH_SIZE: usize = 100;
                        
                        for batch_idx in 0..BATCH_SIZE {
                            nonce_counter = nonce_counter.wrapping_add(thread_step);
                            
                            aligned_nonce.0.copy_from_slice(&nonce_counter.to_be_bytes());
                            blob[39..=42].copy_from_slice(&aligned_nonce.0);
                            
                            let hash_result = vm.calculate_hash(&blob);
                            let hash = match hash_result {
                                Ok(h) => h,
                                Err(e) => {
                                    eprintln!("ERROR: Thread {} - Batch {} - Hash calculation failed: {}", i, batch_idx, e);
                                    continue;
                                }
                            };
                            
                            crate::hash_rate::get_hash_rate_tracker().lock().unwrap().increment(1);
                            
                            let hash_bytes: &[u8] = hash.as_ref();
                            let hash_value = u64::from_le_bytes([
                                hash_bytes[24], hash_bytes[25], 
                                hash_bytes[26], hash_bytes[27],
                                hash_bytes[28], hash_bytes[29], 
                                hash_bytes[30], hash_bytes[31]
                            ]);
                            
                            if debug_all || debug_hash_log {
                                crate::hash_logger::log_hash_value(nonce_counter, hash_value, difficulty, &job_id);
                            }
                            
                            if hash_value < difficulty {
                                let _ = share_tx.send(Share {
                                    job_id: job_id.clone(),
                                    nonce: aligned_nonce.0.to_vec(),
                                    hash: hash_bytes.into(),
                                });
                            }
                        }
                        
                        if light_mode {
                            std::thread::sleep(Duration::from_micros(100));
                        }
                        
                    } else {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                }
            });
        }
        
        Self {
            share_rx,
            job_tx,
        }
    }
    
    pub fn work(&self, job: Job) {
        self.job_tx.send(job);
    }
    
    pub fn try_recv_share(&self) -> Result<Share, TryRecvError> {
        self.share_rx.try_recv()
    }

    pub fn get_hash_rate(&self) -> f64 {
        crate::hash_rate::get_hash_rate_tracker().lock().unwrap().get_hash_rate()
    }

    pub fn get_total_hashes(&self) -> u64 {
        crate::hash_rate::get_hash_rate_tracker().lock().unwrap().get_total_hashes()
    }

    pub fn get_elapsed_time(&self) -> std::time::Duration {
        crate::hash_rate::get_hash_rate_tracker().lock().unwrap().get_elapsed_time()
    }
}

#[cfg(target_os = "linux")]
pub fn enable_huge_pages(num_threads: NonZeroUsize) {
    use std::process::{Command, Stdio};
    use std::io::Write;
    use sysinfo::{RefreshKind, System};

    println!("Checking for non-interactive sudo permissions...");
    let sudo_check = Command::new("sudo")
        .arg("-n")
        .arg("true")
        .output();

    if let Ok(output) = sudo_check {
        if !output.status.success() {
            println!("ℹ️  Sudo requires a password. Skipping automatic huge page configuration.");
            println!("   You can manually configure huge pages if needed.");
            return;
        }
        println!("✅ Sudo available without password.");
    } else {
        eprintln!("❌ Failed to run sudo check. Skipping automatic huge page configuration.");
        return;
    }

    let mut sys = System::new_with_specifics(RefreshKind::default().with_memory(sysinfo::MemoryRefreshKind::everything()));
    sys.refresh_memory();
    let total_memory_bytes = sys.total_memory();

    const RANDOMX_THREAD_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024;
    const HUGE_PAGE_SIZE_BYTES: u64 = 2 * 1024 * 1024;

    let required_memory_for_threads_bytes = num_threads.get() as u64 * RANDOMX_THREAD_MEMORY_BYTES;
    let required_huge_pages = required_memory_for_threads_bytes / HUGE_PAGE_SIZE_BYTES;

    const MIN_FREE_MEMORY_BYTES: u64 = 4 * 1024 * 1024 * 1024;
    const MAX_MEMORY_PERCENTAGE: f64 = 0.80;

    let max_allocatable_bytes = (total_memory_bytes as f64 * MAX_MEMORY_PERCENTAGE) as u64;
    let max_allocatable_leaving_free = total_memory_bytes.saturating_sub(MIN_FREE_MEMORY_BYTES);

    let effective_max_allocatable = max_allocatable_bytes.min(max_allocatable_leaving_free);

    if required_memory_for_threads_bytes > effective_max_allocatable {
        println!("⚠️  Not enough memory to safely allocate {} huge pages for {} threads.", required_huge_pages, num_threads);
        println!("   Required: {:.2} GB, Total System: {:.2} GB, Max Safe Allocation: {:.2} GB",
                 required_memory_for_threads_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                 total_memory_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
                 effective_max_allocatable as f64 / (1024.0 * 1024.0 * 1024.0));
        println!("   Skipping automatic huge page configuration to prevent system instability.");
        return;
    }

    println!("Attempting to configure {} huge pages...", required_huge_pages);

    let mut child = Command::new("sudo")
        .arg("tee")
        .arg("/proc/sys/vm/nr_hugepages")
        .stdin(Stdio::piped())
        .stdout(Stdio::null()) 
        .stderr(Stdio::inherit()) 
        .spawn()
        .expect("Failed to spawn sudo tee command");

    {
        let stdin = child.stdin.as_mut().expect("Failed to open stdin");
        stdin.write_all(format!("{}", required_huge_pages).as_bytes()).expect("Failed to write to stdin");
    }

    let status = child.wait().expect("Failed to wait on sudo tee command");

    if status.success() {
        println!("✅ Successfully configured {} huge pages.", required_huge_pages);
    } else {
        eprintln!("❌ Failed to configure huge pages. Status: {:?}", status);
        eprintln!("   Please ensure you have 'sudo' permissions and that the command is allowed.");
        eprintln!("   You can manually run: echo {} | sudo tee /proc/sys/vm/nr_hugepages", required_huge_pages);
    }
}

#[cfg(not(target_os = "linux"))]
pub fn enable_huge_pages(num_threads: NonZeroUsize) {
    println!("ℹ️  Huge pages support only available on Linux");
}

#[cfg(target_os = "linux")]
pub fn apply_msr_mods() {
    use std::process::{Command, Stdio};
    use sysinfo::{CpuRefreshKind, RefreshKind, System};

    println!("Checking for non-interactive sudo permissions for MSR modifications...");
    let sudo_check = Command::new("sudo")
        .arg("-n")
        .arg("true")
        .output();

    if let Ok(output) = sudo_check {
        if !output.status.success() {
            println!("ℹ️  Sudo requires a password. Skipping MSR modifications.");
            return;
        }
        println!("✅ Sudo available without password for MSR modifications.");
    } else {
        eprintln!("❌ Failed to run sudo check. Skipping MSR modifications.");
        return;
    }

    let mut sys = System::new_with_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()));
    sys.refresh_specifics(RefreshKind::nothing().with_cpu(CpuRefreshKind::everything()));
    let cpus = sys.cpus();
    if cpus.is_empty() {
        eprintln!("❌ No CPUs detected. Skipping MSR modifications.");
        return;
    }
    let vendor_id = cpus[0].vendor_id().to_lowercase();
    println!("Detected CPU vendor: {}", vendor_id);

    println!("Checking if 'msr' kernel module is loaded...");
    let msr_check = Command::new("lsmod")
        .arg("|")
        .arg("grep")
        .arg("msr")
        .output();

    let msr_loaded = match msr_check {
        Ok(output) => output.status.success(),
        Err(_) => {
            eprintln!("❌ Failed to check 'msr' module status. Skipping MSR modifications.");
            return;
        }
    };

    if !msr_loaded {
        eprintln!("❌ 'msr' kernel module is not loaded. Skipping MSR modifications.");
        eprintln!("   You may need to load it with: sudo modprobe msr");
        return;
    }
    println!("✅ 'msr' kernel module is loaded.");

    println!("Checking if 'msr-tools' is installed...");
    let msr_tools_check = Command::new("wrmsr")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Err(_) = msr_tools_check {
        eprintln!("❌ 'msr-tools' is not installed. Skipping MSR modifications.");
        eprintln!("   Please install it with: sudo apt install msr-tools");
        return;
    }
    println!("✅ 'msr-tools' is installed.");

    let (msr_address, msr_value, description) = if vendor_id.contains("intel") {
        (0x1a4, 0xf, "Intel hardware prefetchers")
    } else if vendor_id.contains("amd") {
        (0x1a0, 0x2000, "AMD data cache prefetcher")
    } else {
        println!("⚠️  Unknown CPU vendor '{}'. Skipping MSR modifications.", vendor_id);
        return;
    };

    println!("Attempting to disable {} via MSR 0x{:x}...", description, msr_address);

    let status = Command::new("sudo")
        .arg("wrmsr")
        .arg("-a")
        .arg(format!("0x{:x}", msr_address))
        .arg(format!("0x{:x}", msr_value))
        .status();

    if let Ok(exit_status) = status {
        if exit_status.success() {
            println!("✅ Successfully applied MSR modifications to disable {}.", description);
        } else {
            eprintln!("❌ Failed to apply MSR modifications. Command exited with status: {:?}", exit_status);
        }
    } else {
        eprintln!("❌ Failed to execute 'wrmsr' command.");
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_msr_mods() {
    println!("ℹ️  MSR modifications only available on Linux");
}
