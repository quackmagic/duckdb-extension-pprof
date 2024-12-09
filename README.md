# DuckDB pprof extension

```sql
D LOAD '/usr/src/duckdb-extension-pprof/build/debug/quack_pprof.duckdb_extension';
D SELECT * FROM trace_start();
┌──────────────────────────────────────────────────┐
│                      status                      │
│                     varchar                      │
├──────────────────────────────────────────────────┤
│ Profiling started with signal-safe configuration │
└──────────────────────────────────────────────────┘
D SELECT 1;
┌───────┐
│   1   │
│ int32 │
├───────┤
│     1 │
└───────┘
D SELECT version();
┌─────────────┐
│ "version"() │
│   varchar   │
├─────────────┤
│ v1.1.3      │
└─────────────┘
D SELECT * FROM trace_results();
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                  stack_trace                                                                  │
│                                                                    varchar                                                                    │
├───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┤
│ FRAME: backtrace::backtrace::libunwind::trace -> backtrace::backtrace::trace_unsynchronized -> FRAME: <pprof::backtrace::backtrace_rs::Trac…  │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
D SELECT * FROM trace_stop('duckdb.pprof');
┌───────────────────────────────┐
│            status             │
│            varchar            │
├───────────────────────────────┤
│ Profile saved to duckdb.pprof │
└───────────────────────────────┘
```
