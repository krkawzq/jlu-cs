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



# ===================== 依赖管理 =====================
# 优先使用本地 lib 目录的依赖，如果不存在则使用 FetchContent
set(LOCAL_LIB_DIR ${CMAKE_SOURCE_DIR}/lib)

# --------------------- 1. fmt ---------------------
if(EXISTS ${LOCAL_LIB_DIR}/fmt)
    message(STATUS "Using local fmt from ${LOCAL_LIB_DIR}/fmt")
    add_subdirectory(${LOCAL_LIB_DIR}/fmt EXCLUDE_FROM_ALL)
else()
    message(STATUS "Fetching fmt from GitHub...")
    include(FetchContent)
    FetchContent_Declare(
        fmt
        GIT_REPOSITORY https://github.com/fmtlib/fmt.git
        GIT_TAG 11.0.0
    )
    FetchContent_MakeAvailable(fmt)
endif()

# --------------------- 2. spdlog ---------------------
if(EXISTS ${LOCAL_LIB_DIR}/spdlog)
    message(STATUS "Using local spdlog from ${LOCAL_LIB_DIR}/spdlog")
    add_subdirectory(${LOCAL_LIB_DIR}/spdlog EXCLUDE_FROM_ALL)
else()
    message(STATUS "Fetching spdlog from GitHub...")
    include(FetchContent)
    FetchContent_Declare(
        spdlog
        GIT_REPOSITORY https://github.com/gabime/spdlog.git
        GIT_TAG v1.13.0
    )
    FetchContent_MakeAvailable(spdlog)
endif()

# --------------------- 3. magic_enum ---------------------
if(EXISTS ${LOCAL_LIB_DIR}/magic_enum)
    message(STATUS "Using local magic_enum from ${LOCAL_LIB_DIR}/magic_enum")
    add_subdirectory(${LOCAL_LIB_DIR}/magic_enum EXCLUDE_FROM_ALL)
else()
    message(STATUS "Fetching magic_enum from GitHub...")
    include(FetchContent)
    FetchContent_Declare(
        magic_enum
        GIT_REPOSITORY https://github.com/Neargye/magic_enum.git
        GIT_TAG v0.9.6
    )
    FetchContent_MakeAvailable(magic_enum)
endif()



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

# ===================== Bison 生成 parser =====================
find_package(BISON REQUIRED)

set(PARSER_Y_FILE ${SRC_DIR}/parser.y)
set(PARSER_CPP_FILE ${CMAKE_BINARY_DIR}/parser.tab.cpp)
set(PARSER_HPP_FILE ${CMAKE_BINARY_DIR}/parser.tab.hpp)

BISON_TARGET(Parser
    ${PARSER_Y_FILE}
    ${PARSER_CPP_FILE}
    DEFINES_FILE ${PARSER_HPP_FILE}
    COMPILE_FLAGS "-Wall -Wcounterexamples"
)

# ===================== 项目源文件 =====================
# Include .cpp files from src directory
file(GLOB SRC_FILES
    ${SRC_DIR}/*.cpp
)

# 添加 Bison 生成的文件
add_executable(Prim 
    ${SRC_FILES}
    ${BISON_Parser_OUTPUTS}
)

# 确保在编译源文件前生成 lexer 和 parser
add_dependencies(Prim generate_lexer)
target_sources(Prim PRIVATE ${BISON_Parser_OUTPUTS})




# ===================== 依赖链接 =====================
target_link_libraries(Prim
    PRIVATE
        fmt::fmt
        spdlog::spdlog
        # hwy
        magic_enum::magic_enum
)

# ===================== 包含目录 =====================
target_include_directories(Prim
    PRIVATE
        ${INCLUDE_DIR}
        ${SRC_DIR}
        ${CMAKE_BINARY_DIR}  # For generated parser.tab.hpp
)

# ===================== 可执行文件输出路径 =====================
set_target_properties(Prim PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY ${EXECUTABLE_OUTPUT_DIR}
    LIBRARY_OUTPUT_DIRECTORY ${LIBRARY_OUTPUT_DIR}
    ARCHIVE_OUTPUT_DIRECTORY ${LIBRARY_OUTPUT_DIR}
)

# Copy executable to project root after build
add_custom_command(
    TARGET Prim POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        $<TARGET_FILE:Prim>
        ${PROJECT_SOURCE_DIR}/$<IF:$<CONFIG:Debug>,debug,$<IF:$<CONFIG:Release>,release,>>$<TARGET_FILE_SUFFIX:Prim>
    COMMENT "Copying executable to project root"
)
