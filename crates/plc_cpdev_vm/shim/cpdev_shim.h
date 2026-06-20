/*
 * Thin C-linkage shim over the CPDev C++ Virtual Machine.
 *
 * The CPDev VM exposes C++ classes (VMLinux / VMDCP / VMVariable) with
 * overloads, default arguments, and a method that can `throw`. None of that is
 * FFI-safe. This shim is the ONLY place those classes are touched: it exposes a
 * minimal, exception-free, overload-free C surface that Rust binds directly.
 *
 * Every function is exception-guarded in the .cpp so no C++ exception can ever
 * unwind across the FFI boundary. Error conditions are reported as return codes.
 */
#ifndef CPDEV_SHIM_H
#define CPDEV_SHIM_H

#ifdef __cplusplus
extern "C" {
#endif

/* Opaque handle: owns one VMLinux + VMDCP + the resolved variable handles +
 * (for the byte-array loader) a copy of the .XCP code. */
typedef struct CpdevVm CpdevVm;

/* Lifetime. */
CpdevVm *cpdev_open(void);
void cpdev_free(CpdevVm *vm);

/* Load the compiled program. Either from a file path (the VM reads + copies it
 * itself) or from a byte buffer (the shim copies it into a buffer that outlives
 * the VM). Both return 0 on success, negative on error. */
int cpdev_load_xcp_file(CpdevVm *vm, const char *path);
int cpdev_load_xcp(CpdevVm *vm, const unsigned char *code, int len);

/* Load the .DCP variable map (XML). 0 on success, negative on error. */
int cpdev_load_dcp(CpdevVm *vm, const char *path);

/* Resolve a declared variable by name -> opaque VMVariable* (NULL if missing).
 * The handle is owned by the CpdevVm and freed in cpdev_free. */
void *cpdev_var(CpdevVm *vm, const char *name);
/* Byte size of a resolved variable, or -1 if the handle is NULL. */
int cpdev_var_size(void *var);

/* Scan-cycle wall-clock pacing in ms; 0 = free-run (no sleep between cycles). */
void cpdev_set_task_cycle(CpdevVm *vm, unsigned short ms);

/* Initialize once before the first cycle (mode = WM_MODE_FIRST_START|NORMAL). */
void cpdev_initialize(CpdevVm *vm, int mode);
/* Run exactly one scan cycle. */
void cpdev_run_cycle(CpdevVm *vm);

/* Write/read a variable's raw little-endian bytes via its handle.
 * `len` must equal the variable size (write) or be at least it (read).
 * Return bytes transferred, or -1 on a size mismatch / NULL handle. */
int cpdev_set(CpdevVm *vm, void *var, const unsigned char *buf, int len);
int cpdev_get(CpdevVm *vm, void *var, unsigned char *buf, int len);

/* Status word (WMSTAT_* bits) and run mode (0 ⇒ a runtime fault fired). */
unsigned short cpdev_status1(CpdevVm *vm);
unsigned char cpdev_run_mode(CpdevVm *vm);

#ifdef __cplusplus
}
#endif

#endif /* CPDEV_SHIM_H */
