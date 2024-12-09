extern crate duckdb;
extern crate duckdb_loadable_macros;
extern crate libduckdb_sys;
extern crate pprof;

use duckdb::{
    core::{DataChunkHandle, Inserter, LogicalTypeHandle, LogicalTypeId},
    vtab::{BindInfo, Free, FunctionInfo, InitInfo, VTab},
    Connection, Result,
};
use duckdb_loadable_macros::duckdb_entrypoint_c_api;
use libduckdb_sys as ffi;
use std::{
    error::Error,
    ffi::{c_char, CString},
    sync::Arc,
    sync::Mutex,
    fs::File,
    io::Write,
};
use pprof::ProfilerGuard;
use pprof::protos::Message;

// Store just the guard
lazy_static::lazy_static! {
    static ref PROFILER_GUARD: Arc<Mutex<Option<ProfilerGuard<'static>>>> = Arc::new(Mutex::new(None));
}

// Empty struct that implements Free for BindData
#[repr(C)]
struct EmptyBindData;

impl Free for EmptyBindData {}

// Trace Start implementation
struct TraceStartVTab;

#[repr(C)]
struct TraceStartInitData {
    done: bool,
}

impl Free for TraceStartInitData {}

impl VTab for TraceStartVTab {
    type InitData = TraceStartInitData;
    type BindData = EmptyBindData;

    unsafe fn bind(bind: &BindInfo, _: *mut EmptyBindData) -> Result<(), Box<dyn Error>> {
        bind.add_result_column("status", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        Ok(())
    }

    unsafe fn init(_: &InitInfo, data: *mut TraceStartInitData) -> Result<(), Box<dyn Error>> {
        unsafe {
            (*data).done = false;
        }
        Ok(())
    }

    unsafe fn func(func: &FunctionInfo, output: &mut DataChunkHandle) -> Result<(), Box<dyn Error>> {
        let init_info = func.get_init_data::<TraceStartInitData>();
        
        unsafe {
            if (*init_info).done {
                output.set_len(0);
                return Ok(());
            }
            
            (*init_info).done = true;
            
            let mut guard = PROFILER_GUARD.lock().unwrap();
            if guard.is_some() {
                let vector = output.flat_vector(0);
                vector.insert(0, CString::new("Profiling already running")?);
                output.set_len(1);
                return Ok(());
            }

            // Using recommended blocklist for signal safety
            let new_guard = pprof::ProfilerGuardBuilder::default()
                .frequency(1000)
                .blocklist(&["libc", "libgcc", "pthread", "vdso"])
                .build()
                .unwrap();
            
            *guard = Some(new_guard);
            
            let vector = output.flat_vector(0);
            vector.insert(0, CString::new("Profiling started with signal-safe configuration")?);
            output.set_len(1);
        }
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        None
    }
}

// Trace Stop implementation
struct TraceStopVTab;

#[repr(C)]
struct TraceStopBindData {
    filename: *mut c_char,
}

#[repr(C)]
struct TraceStopInitData {
    done: bool,
}

impl Free for TraceStopBindData {
    fn free(&mut self) {
        unsafe {
            if !self.filename.is_null() {
                drop(CString::from_raw(self.filename));
            }
        }
    }
}

impl Free for TraceStopInitData {}

impl VTab for TraceStopVTab {
    type InitData = TraceStopInitData;
    type BindData = TraceStopBindData;

    unsafe fn bind(bind: &BindInfo, data: *mut TraceStopBindData) -> Result<(), Box<dyn Error>> {
        bind.add_result_column("status", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        let filename = bind.get_parameter(0).to_string();
        unsafe {
            (*data).filename = CString::new(filename).unwrap().into_raw();
        }
        Ok(())
    }

    unsafe fn init(_: &InitInfo, data: *mut TraceStopInitData) -> Result<(), Box<dyn Error>> {
        unsafe {
            (*data).done = false;
        }
        Ok(())
    }

    unsafe fn func(func: &FunctionInfo, output: &mut DataChunkHandle) -> Result<(), Box<dyn Error>> {
        let init_info = func.get_init_data::<TraceStopInitData>();
        let bind_info = func.get_bind_data::<TraceStopBindData>();
        
        unsafe {
            if (*init_info).done {
                output.set_len(0);
                return Ok(());
            }
            
            (*init_info).done = true;
            
            let filename_cstr = CString::from_raw((*bind_info).filename);
            let filename_str = filename_cstr.to_str()?;
            
            let mut guard_lock = PROFILER_GUARD.lock().unwrap();
            
            let result = if let Some(guard) = guard_lock.take() {
                let report_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    guard.report().build()
                }));

                match report_result {
                    Ok(Ok(report)) => {
                        let profile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            report.pprof()
                        }));

                        match profile_result {
                            Ok(Ok(profile)) => {
                                match (|| -> Result<_, Box<dyn Error>> {
                                    let mut file = File::create(filename_str)?;
                                    let mut content = Vec::new();
                                    profile.encode(&mut content)?;
                                    file.write_all(&content)?;
                                    Ok(format!("Profile saved to {}", filename_str))
                                })() {
                                    Ok(msg) => msg,
                                    Err(e) => format!("Failed to write profile: {}", e)
                                }
                            },
                            Ok(Err(e)) => format!("Failed to create pprof profile: {}", e),
                            Err(_) => "Internal error: Failed to create pprof profile safely".to_string()
                        }
                    },
                    Ok(Err(e)) => format!("Failed to build report: {}", e),
                    Err(_) => "Internal error: Failed to build report safely".to_string()
                }
            } else {
                "No profiling session running".to_string()
            };
            
            let vector = output.flat_vector(0);
            vector.insert(0, CString::new(result)?);
            output.set_len(1);
            
            (*bind_info).filename = CString::into_raw(filename_cstr);
        }
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        Some(vec![LogicalTypeHandle::from(LogicalTypeId::Varchar)])
    }
}

// Trace Results implementation
struct TraceResultsVTab;

#[repr(C)]
struct TraceResultsInitData {
    done: bool,
}

impl Free for TraceResultsInitData {}

impl VTab for TraceResultsVTab {
    type InitData = TraceResultsInitData;
    type BindData = EmptyBindData;

    unsafe fn bind(bind: &BindInfo, _: *mut EmptyBindData) -> Result<(), Box<dyn Error>> {
        bind.add_result_column("stack_trace", LogicalTypeHandle::from(LogicalTypeId::Varchar));
        Ok(())
    }

    unsafe fn init(_: &InitInfo, data: *mut TraceResultsInitData) -> Result<(), Box<dyn Error>> {
        unsafe {
            (*data).done = false;
        }
        Ok(())
    }

    unsafe fn func(func: &FunctionInfo, output: &mut DataChunkHandle) -> Result<(), Box<dyn Error>> {
        let init_info = func.get_init_data::<TraceResultsInitData>();
        
        unsafe {
            if (*init_info).done {
                output.set_len(0);
                return Ok(());
            }
            
            (*init_info).done = true;
            
            let guard_lock = PROFILER_GUARD.lock().unwrap();
            
            if let Some(ref guard) = *guard_lock {
                let report_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    guard.report().build()
                }));

                match report_result {
                    Ok(Ok(report)) => {
                        let report_text = format!("{:?}", report);
                        let stacks: Vec<&str> = report_text.lines()
                            .filter(|line| line.contains("FRAME:"))
                            .collect();
                        
                        if stacks.is_empty() {
                            let vector = output.flat_vector(0);
                            vector.insert(0, CString::new("No stack traces collected yet")?);
                            output.set_len(1);
                        } else {
                            let vector = output.flat_vector(0);
                            for (idx, stack) in stacks.iter().enumerate() {
                                vector.insert(idx, CString::new(*stack)?);
                            }
                            output.set_len(stacks.len());
                        }
                    },
                    Ok(Err(e)) => {
                        let vector = output.flat_vector(0);
                        vector.insert(0, CString::new(format!("Error building report: {}", e))?);
                        output.set_len(1);
                    },
                    Err(_) => {
                        let vector = output.flat_vector(0);
                        vector.insert(0, CString::new("Internal error: Failed to build report safely")?);
                        output.set_len(1);
                    }
                }
            } else {
                let vector = output.flat_vector(0);
                vector.insert(0, CString::new("No profiling session running")?);
                output.set_len(1);
            }
        }
        Ok(())
    }

    fn parameters() -> Option<Vec<LogicalTypeHandle>> {
        None
    }
}

#[duckdb_entrypoint_c_api(ext_name = "quack_pprof", min_duckdb_version = "v0.0.1")]
pub unsafe fn extension_entrypoint(con: Connection) -> Result<(), Box<dyn Error>> {
    con.register_table_function::<TraceStartVTab>("trace_start")
        .expect("Failed to register trace_start function");
    con.register_table_function::<TraceStopVTab>("trace_stop")
        .expect("Failed to register trace_stop function");
    con.register_table_function::<TraceResultsVTab>("trace_results")
        .expect("Failed to register trace_results function");
    Ok(())
}
