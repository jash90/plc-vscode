/*
 * Implementation of the C-linkage shim over the CPDev C++ VM.
 *
 * Every entry point is wrapped in try/catch(...) so a C++ exception (tinyxml2
 * parse, `new`, or the bare `throw;` inside VMLinux::WM_GetDataByte — which this
 * shim never calls) can never unwind into Rust. Overloads are disambiguated by
 * calling the explicit VMVariable* handle forms; default arguments are passed
 * explicitly.
 */
#include "cpdev_shim.h"

#include "vm_linux.h"
#include "vm_variable.h"

#include <cstdio>
#include <vector>
// POSIX/Windows headers for the StdoutSilencer fd redirection below.
#if defined(_WIN32)
#include <fcntl.h> // _O_WRONLY
#include <io.h>    // _dup, _dup2, _open, _close
#else
#include <fcntl.h>  // O_WRONLY
#include <unistd.h> // dup, dup2, open, close, STDOUT_FILENO
#endif

struct CpdevVm {
    VMLinux vm;
    VMDCP dcp;
    std::vector<VMVariable *> vars; // owned: freed in cpdev_free
    std::vector<unsigned char> code; // owned copy for the byte-array loader
    std::vector<unsigned char> data; // owned data segment (byte-array loader)
};

namespace {
// Floor a requested data-segment size at the VM's default so behaviour for
// small programs is unchanged, while larger programs get a correctly sized
// (heap) buffer instead of the vendored 256-byte static one.
int data_buffer_size(int datasize) {
    return datasize > DEFAULT_DATA_SIZE ? datasize : DEFAULT_DATA_SIZE;
}
} // namespace

namespace {
// RAII: redirect C stdout to /dev/null for its lifetime. The vendored VMDCP
// loader prints progress ("Loading DCP: ...", "OUT0 found at 0") to stdout; that
// would corrupt the CLI watch table, so we silence it around those calls without
// touching the vendored sources. Restores stdout on scope exit.
struct StdoutSilencer {
    int saved = -1;
    StdoutSilencer() {
        fflush(stdout);
#if defined(_WIN32)
        saved = _dup(_fileno(stdout));
        int devnull = _open("NUL", _O_WRONLY);
        if (devnull >= 0) {
            _dup2(devnull, _fileno(stdout));
            _close(devnull);
        }
#else
        saved = dup(STDOUT_FILENO);
        int devnull = open("/dev/null", O_WRONLY);
        if (devnull >= 0) {
            dup2(devnull, STDOUT_FILENO);
            close(devnull);
        }
#endif
    }
    ~StdoutSilencer() {
        if (saved >= 0) {
            fflush(stdout);
#if defined(_WIN32)
            _dup2(saved, _fileno(stdout));
            _close(saved);
#else
            dup2(saved, STDOUT_FILENO);
            close(saved);
#endif
        }
    }
};
} // namespace

extern "C" {

CpdevVm *cpdev_open(void) {
    try {
        return new CpdevVm();
    } catch (...) {
        return nullptr;
    }
}

void cpdev_free(CpdevVm *vm) {
    if (!vm) {
        return;
    }
    try {
        for (VMVariable *v : vm->vars) {
            delete v;
        }
    } catch (...) {
    }
    delete vm;
}

int cpdev_load_xcp_file(CpdevVm *vm, const char *path, int datasize) {
    if (!vm || !path) {
        return -100;
    }
    try {
        // The file loader malloc()s `datasize` and zero-fills it, so it already
        // supports buffers larger than the static default.
        return vm->vm.VMP_LoadProgramAndData(path, data_buffer_size(datasize));
    } catch (...) {
        return -101;
    }
}

int cpdev_load_xcp(CpdevVm *vm, const unsigned char *code, int len, int datasize) {
    if (!vm || !code || len <= 0) {
        return -100;
    }
    try {
        // Copy: the VM stores the code pointer WITHOUT copying, so the bytes must
        // outlive the VM. We own both the code copy and the data segment here.
        //
        // The vendored VMP_LoadProgramFromArray hardcodes the 256-byte static
        // `pgmDataDefault` and rejects datasize > 256, so we replicate its setup
        // (pgmCode/pgmData/wStatus1/nCycles are public registers) over a
        // shim-owned, correctly sized, zero-filled heap buffer instead.
        vm->code.assign(code, code + len);
        vm->data.assign(static_cast<size_t>(data_buffer_size(datasize)), 0);
        vm->vm.pgmCode = vm->code.data();
        vm->vm.pgmData = vm->data.data();
        vm->vm.wStatus1 = WMSTAT_COLDRESTART;
        vm->vm.nCycles = 0;
        return 0;
    } catch (...) {
        return -101;
    }
}

int cpdev_load_dcp(CpdevVm *vm, const char *path) {
    if (!vm || !path) {
        return -100;
    }
    try {
        StdoutSilencer hush;
        return vm->dcp.Load(path);
    } catch (...) {
        return -101;
    }
}

void *cpdev_var(CpdevVm *vm, const char *name) {
    if (!vm || !name) {
        return nullptr;
    }
    try {
        StdoutSilencer hush;
        VMVariable *v = vm->dcp.InitVariable(name);
        if (v) {
            vm->vars.push_back(v);
        }
        return v;
    } catch (...) {
        return nullptr;
    }
}

int cpdev_var_size(void *var) {
    if (!var) {
        return -1;
    }
    try {
        return static_cast<int>(static_cast<VMVariable *>(var)->GetSize());
    } catch (...) {
        return -1;
    }
}

void cpdev_set_task_cycle(CpdevVm *vm, unsigned short ms) {
    if (!vm) {
        return;
    }
    try {
        vm->vm.task_cycle = ms;
    } catch (...) {
    }
}

void cpdev_initialize(CpdevVm *vm, int mode) {
    if (!vm) {
        return;
    }
    try {
        vm->vm.WM_Initialize(mode);
    } catch (...) {
    }
}

void cpdev_run_cycle(CpdevVm *vm) {
    if (!vm) {
        return;
    }
    try {
        vm->vm.WM_RunCycle();
    } catch (...) {
    }
}

int cpdev_set(CpdevVm *vm, void *var, const unsigned char *buf, int len) {
    if (!vm || !var || !buf) {
        return -1;
    }
    try {
        VMVariable *v = static_cast<VMVariable *>(var);
        if (static_cast<int>(v->GetSize()) != len) {
            return -1;
        }
        // WM_SetData reads from buf (memcpy into VM data memory); const_cast is
        // safe because the buffer is never written.
        return vm->vm.WM_SetData(v, const_cast<WM_BYTE *>(buf));
    } catch (...) {
        return -1;
    }
}

int cpdev_get(CpdevVm *vm, void *var, unsigned char *buf, int len) {
    if (!vm || !var || !buf) {
        return -1;
    }
    try {
        VMVariable *v = static_cast<VMVariable *>(var);
        if (len < static_cast<int>(v->GetSize())) {
            return -1;
        }
        return vm->vm.WM_GetData(v, buf);
    } catch (...) {
        return -1;
    }
}

unsigned short cpdev_status1(CpdevVm *vm) {
    if (!vm) {
        return 0;
    }
    try {
        return vm->vm.WM_GetStatus1();
    } catch (...) {
        return 0;
    }
}

unsigned char cpdev_run_mode(CpdevVm *vm) {
    if (!vm) {
        return 0;
    }
    try {
        return vm->vm.bRunMode;
    } catch (...) {
        return 0;
    }
}

} // extern "C"
