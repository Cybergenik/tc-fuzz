// Fuzzer harness: recompiles tc_calc.c with external linkage
// so Rust can call calc_eval() via FFI.

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <assert.h>
#include <ctype.h>

// Override static linkage macros from main.c
#define function
#define global
#define cast(T) (T)

#ifndef TC_CALC_FORMAT_BUFFER_SIZE
#define TC_CALC_FORMAT_BUFFER_SIZE 256
#endif

#include "tc_calc.h"
#include "tc_calc.c"

const char *__ubsan_default_options() {
    return "abort_on_error=0:print_stacktrace=0:log_path=./crashes/ubsan";
}

