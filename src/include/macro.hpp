// macro.hpp
// 宏工具 - 跨平台优化指令
#pragma once

// ============================================================================
// 编译器检测
// ============================================================================
#if defined(_MSC_VER)
    #define COMPILER_MSVC_ 1
#elif defined(__clang__)
    #define COMPILER_CLANG_ 1
#elif defined(__GNUC__) || defined(__GNUG__)
    #define COMPILER_GCC_ 1
#endif

// ============================================================================
// 平台检测
// ============================================================================
#if defined(_WIN32) || defined(_WIN64)
    #define PLATFORM_WINDOWS_ 1
#elif defined(__linux__)
    #define PLATFORM_LINUX_ 1
#elif defined(__APPLE__) && defined(__MACH__)
    #define PLATFORM_MACOS_ 1
#endif

// ============================================================================
// 分支预测优化
// ============================================================================
// likely_: 提示编译器该分支很可能被执行
// unlikely_: 提示编译器该分支不太可能被执行
// 
// 用法示例:
//   if (likely_(ptr != nullptr)) { ... }
//   if (unlikely_(error_occurred)) { ... }

#if defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define likely_(x)   (__builtin_expect(!!(x), 1))
    #define unlikely_(x) (__builtin_expect(!!(x), 0))
#else
    // MSVC 可以使用 C++20 的 [[likely]]/[[unlikely]] 属性
    // 但为了保持语法一致性，这里定义为恒等宏
    #define likely_(x)   (x)
    #define unlikely_(x) (x)
#endif

// ============================================================================
// 内存预取指令
// ============================================================================
// prefetch_r_: 预取数据用于读取
// prefetch_w_: 预取数据用于写入
//
// 参数:
//   addr - 要预取的内存地址
//
// 用法示例:
//   prefetch_r_(&data[i + 8]);  // 提前预取未来要读取的数据
//   prefetch_w_(&output[i]);    // 提前预取未来要写入的位置

#if defined(COMPILER_MSVC_)
    // MSVC 使用 SSE intrinsics
    #include <xmmintrin.h>
    
    // _MM_HINT_T0: 预取到所有缓存级别
    // _MM_HINT_T1: 预取到 L2 和更高级别缓存
    // _MM_HINT_T2: 预取到 L3 和更高级别缓存
    // _MM_HINT_NTA: 非临时访问（跳过缓存）
    
    #define prefetch_r_(addr) _mm_prefetch((const char*)(addr), _MM_HINT_T0)
    #define prefetch_w_(addr) _mm_prefetch((const char*)(addr), _MM_HINT_T0)
    
#elif defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    // GCC/Clang 使用 __builtin_prefetch
    // 参数: (地址, rw, locality)
    //   rw: 0 = 读, 1 = 写
    //   locality: 0 (无时间局部性) 到 3 (高时间局部性)
    
    #define prefetch_r_(addr) __builtin_prefetch((const void*)(addr), 0, 3)
    #define prefetch_w_(addr) __builtin_prefetch((const void*)(addr), 1, 3)
    
#else
    // 不支持的编译器，定义为空操作
    #define prefetch_r_(addr) ((void)0)
    #define prefetch_w_(addr) ((void)0)
#endif

// ============================================================================
// 强制内联
// ============================================================================
// force_inline_: 强制函数内联
//
// 用法示例:
//   force_inline_ int add(int a, int b) { return a + b; }

#if defined(COMPILER_MSVC_)
    #define force_inline_ __forceinline
#elif defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define force_inline_ inline __attribute__((always_inline))
#else
    #define force_inline_ inline
#endif

// ============================================================================
// 禁止内联
// ============================================================================
// no_inline_: 禁止函数内联
//
// 用法示例:
//   no_inline_ void complex_function() { ... }

#if defined(COMPILER_MSVC_)
    #define no_inline_ __declspec(noinline)
#elif defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define no_inline_ __attribute__((noinline))
#else
    #define no_inline_
#endif

// ============================================================================
// 内存对齐
// ============================================================================
// aligned_(n): 指定变量或类型的对齐方式
//
// 用法示例:
//   aligned_(64) int cache_line[16];

#if defined(COMPILER_MSVC_)
    #define aligned_(n) __declspec(align(n))
#elif defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define aligned_(n) __attribute__((aligned(n)))
#else
    #define aligned_(n)
#endif

// ============================================================================
// 缓存行大小
// ============================================================================
// 常见 CPU 缓存行大小为 64 字节

#ifndef CACHE_LINE_SIZE_
    #define CACHE_LINE_SIZE_ 64
#endif

// ============================================================================
// 优化屏障
// ============================================================================
// 防止编译器重排序

#if defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define compiler_barrier_() __asm__ __volatile__("" ::: "memory")
#elif defined(COMPILER_MSVC_)
    #include <intrin.h>
    #define compiler_barrier_() _ReadWriteBarrier()
#else
    #define compiler_barrier_() ((void)0)
#endif


// ============================================================================
// 快速 log2（找到最左侧的 1）
// ============================================================================
// 计算整数的 log2 值（即最高位 1 的位置）
//
// 用法示例:
//   int pos = fast_log2_(256);  // 返回 8

#if defined(COMPILER_GCC_) || defined(COMPILER_CLANG_)
    #define fast_log2_(x) (31 - __builtin_clz(x))
    #define fast_log2_64_(x) (63 - __builtin_clzll(x))
#elif defined(COMPILER_MSVC_)
    #include <intrin.h>
    force_inline_ int fast_log2_(unsigned int x) {
        unsigned long index;
        _BitScanReverse(&index, x);
        return (int)index;
    }
    force_inline_ int fast_log2_64_(unsigned long long x) {
        unsigned long index;
        _BitScanReverse64(&index, x);
        return (int)index;
    }
#else
    force_inline_ int fast_log2_(unsigned int x) {
        int pos = 0;
        while (x >>= 1) pos++;
        return pos;
    }
    force_inline_ int fast_log2_64_(unsigned long long x) {
        int pos = 0;
        while (x >>= 1) pos++;
        return pos;
    }
#endif
