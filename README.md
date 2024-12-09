# DuckDB pprof extension

### Build
```
make configure
make debug
```

### Test

```sql
D LOAD './build/debug/quack_pprof.duckdb_extension';
D SELECT * FROM trace_start();
┌──────────────────────────────────────────────────┐
│                      status                      │
│                     varchar                      │
├──────────────────────────────────────────────────┤
│ Profiling started with signal-safe configuration │
└──────────────────────────────────────────────────┘

--- Perform some actions...
D SELECT version();
┌─────────────┐
│ "version"() │
│   varchar   │
├─────────────┤
│ v1.1.3      │
└─────────────┘

--- Check for results
D SELECT * FROM trace_results();
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                  stack_trace                                                                  │
│                                                                    varchar                                                                    │
├───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ FRAME: backtrace::backtrace::libunwind::trace -> backtrace::backtrace::trace_unsynchronized -> FRAME: <pprof::backtrace::backtrace_rs::Trac…  │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘

--- Dump to pprof protobuf file
D SELECT * FROM trace_stop('duckdb.pprof');
┌───────────────────────────────┐
│            status             │
│            varchar            │
├───────────────────────────────┤
│ Profile saved to duckdb.pprof │
└───────────────────────────────┘
```

### Output
![image](https://github.com/user-attachments/assets/1ded4e67-9a63-4c52-a0b4-f6fbbb271e94)

