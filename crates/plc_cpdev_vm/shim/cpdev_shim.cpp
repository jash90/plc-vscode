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
#include <fcntl.h>
#include <unistd.h>
#include <vector>

struct CpdevVm {
    VMLinux vm;
    VMDCP dcp;
    std::vector<VMVariable *> vars; // owned: freed in cpdev_free
    std::vector<unsigned char> code; // owned copy for the byte-array loader
};

namespace {
// RAII: redirect C stdout to /dev/null for its lifetime. The vendored VMDCP
// loader prints progress ("Loading DCP: ...", "OUT0 found at 0") to stdout; that
// would corrupt the CLI watch table, so we silence it around those calls without
// touching the vendored sources. Restores stdout on scope exit.
struct StdoutSilencer {
    int saved = -1;
    StdoutSilencer() {
        fflush(stdout);
        saved = dup(STDOUT_FILENO);
        int devnull = open("/dev/null", O_WRONLY);
        if (devnull >= 0) {
            dup2(devnull, STDOUT_FILENO);
            close(devnull);
        }
    }
    ~StdoutSilencer() {
        if (saved >= 0) {
            fflush(stdout);
            dup2(saved, STDOUT_FILENO);
            close(saved);
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

int cpdev_load_xcp_file(CpdevVm *vm, const char *path) {
    if (!vm || !path) {
        return -100;
    }
    try {
        return vm->vm.VMP_LoadProgramAndData(path, DEFAULT_DATA_SIZE);
    } catch (...) {
        return -101;
    }
}

int cpdev_load_xcp(CpdevVm *vm, const unsigned char *code, int len) {
    if (!vm || !code || len <= 0) {
        return -100;
    }
    try {
        // Copy: VMP_LoadProgramFromArray stores the pointer WITHOUT copying, so
        // the code bytes must outlive the VM. We own the copy here.
        vm->code.assign(code, code + len);
        return vm->vm.VMP_LoadProgramFromArray(vm->code.data(), DEFAULT_DATA_SIZE);
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
