cmake_minimum_required(VERSION 3.15)
project(Prim)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)


# ===================== 路径配置 =====================
set(SRC_DIR ${CMAKE_SOURCE_DIR}/src)
set(INCLUDE_DIR ${SRC_DIR}/include)

set(LIBRARY_OUTPUT_DIR ${CMAKE_BINARY_DIR}/lib)
set(EXECUTABLE_OUTPUT_DIR ${CMAKE_BINARY_DIR}/bin)



# ===================== FetchContent 模块 =====================
include(FetchContent)

# --------------------- 1. Highway ---------------------
# 禁用 Highway 的测试和示例以避免依赖问题
set(HWY_ENABLE_TESTS OFF CACHE BOOL "" FORCE)
set(HWY_ENABLE_EXAMPLES OFF CACHE BOOL "" FORCE)
set(HWY_ENABLE_INSTALL OFF CACHE BOOL "" FORCE)

FetchContent_Declare(
    highway
    GIT_REPOSITORY https://github.com/google/highway.git
    GIT_TAG 1.2.0
)
FetchContent_MakeAvailable(highway)

# --------------------- 2. fmt ---------------------
FetchContent_Declare(
    fmt
    GIT_REPOSITORY https://github.com/fmtlib/fmt.git
    GIT_TAG 11.0.0
)
FetchContent_MakeAvailable(fmt)

# --------------------- 3. spdlog ---------------------
FetchContent_Declare(
    spdlog
    GIT_REPOSITORY https://github.com/gabime/spdlog.git
    GIT_TAG v1.13.0
)
FetchContent_MakeAvailable(spdlog)



# ===================== 项目源文件 =====================
file(GLOB_RECURSE SRC_FILES ${SRC_DIR}/*.cpp)

add_executable(Prim ${SRC_FILES})

# ===================== 依赖链接 =====================
target_link_libraries(Prim
    PRIVATE
        fmt::fmt
        spdlog::spdlog
        hwy
)

# ===================== 包含目录 =====================
target_include_directories(Prim
    PRIVATE
        ${INCLUDE_DIR}
        ${SRC_DIR}
)

# ===================== 可执行文件输出路径 =====================
set_target_properties(Prim PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY ${EXECUTABLE_OUTPUT_DIR}
    LIBRARY_OUTPUT_DIRECTORY ${LIBRARY_OUTPUT_DIR}
    ARCHIVE_OUTPUT_DIRECTORY ${LIBRARY_OUTPUT_DIR}
)

# ===================== re2c 生成 lexer.hpp =====================
find_program(RE2C_EXECUTABLE re2c)
if(NOT RE2C_EXECUTABLE)
    message(FATAL_ERROR "re2c not found. Please install re2c.")
endif()

set(LEXER_RE_FILE ${SRC_DIR}/lexer.re)
set(LEXER_HPP_FILE ${INCLUDE_DIR}/lexer.hpp)

add_custom_command(
    OUTPUT ${LEXER_HPP_FILE}
    COMMAND ${RE2C_EXECUTABLE} -W -i -c -o ${LEXER_HPP_FILE} ${LEXER_RE_FILE}
    DEPENDS ${LEXER_RE_FILE}
    COMMENT "Generating lexer.hpp from lexer.re using re2c"
    VERBATIM
)

add_custom_target(generate_lexer DEPENDS ${LEXER_HPP_FILE})
add_dependencies(Prim generate_lexer)


add_custom_command(
    TARGET Prim POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        $<TARGET_FILE:Prim>
        ${PROJECT_SOURCE_DIR}/main$<$<CONFIG:Debug>:_debug>$<TARGET_FILE_SUFFIX:Prim>
    COMMENT "Copying executable to project root"
)