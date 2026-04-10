cmake_minimum_required(VERSION 3.20)
project(port_libcurl_compat C)

include("${CMAKE_CURRENT_LIST_DIR}/generated-sources.cmake")

if(NOT DEFINED PORT_LIBCURL_UPSTREAM_WORKTREE)
  message(FATAL_ERROR "PORT_LIBCURL_UPSTREAM_WORKTREE is required")
endif()
if(NOT DEFINED PORT_LIBCURL_STAGE_DIR)
  message(FATAL_ERROR "PORT_LIBCURL_STAGE_DIR is required")
endif()

set(PORT_LIBCURL_WORKTREE "${PORT_LIBCURL_UPSTREAM_WORKTREE}")
set(PORT_LIBCURL_STAGE "${PORT_LIBCURL_STAGE_DIR}")

file(READ "${PORT_LIBCURL_SAFE_MANIFEST}" PORT_LIBCURL_MANIFEST_JSON)

add_library(compat_libcurl SHARED IMPORTED GLOBAL)
set_target_properties(
  compat_libcurl
  PROPERTIES
  IMPORTED_LOCATION "${PORT_LIBCURL_STAGE}/lib/libcurl.so"
  INTERFACE_INCLUDE_DIRECTORIES "${PORT_LIBCURL_STAGE}/include"
)

function(port_libcurl_collect_sources out component target_index)
  if(component STREQUAL "src")
    set(component_dir "${PORT_LIBCURL_WORKTREE}/src")
  elseif(component STREQUAL "libtest")
    set(component_dir "${PORT_LIBCURL_WORKTREE}/tests/libtest")
  elseif(component STREQUAL "server")
    set(component_dir "${PORT_LIBCURL_WORKTREE}/tests/server")
  elseif(component STREQUAL "http-client")
    set(component_dir "${PORT_LIBCURL_WORKTREE}/tests/http/clients")
  else()
    message(FATAL_ERROR "unsupported component: ${component}")
  endif()

  port_libcurl_json_length(source_count "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} sources)
  math(EXPR source_last "${source_count} - 1")
  set(result)
  if(source_count GREATER 0)
    foreach(source_index RANGE 0 ${source_last})
      port_libcurl_json_get(source_token "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} sources ${source_index})
      if(source_token MATCHES "\\.c$")
        cmake_path(NORMAL_PATH source_token OUTPUT_VARIABLE source_rel)
        list(APPEND result "${component_dir}/${source_rel}")
      endif()
    endforeach()
  endif()
  set(${out} "${result}" PARENT_SCOPE)
endfunction()

function(port_libcurl_apply_common_flags target_name component target_index)
  target_compile_features(${target_name} PRIVATE c_std_11)
  target_compile_options(
    ${target_name}
    PRIVATE
    -Wall
    -Wextra
    -Wno-deprecated-declarations
    -Wno-unused-parameter
    -Wno-sign-compare
    -Wno-pointer-sign
  )
  target_compile_definitions(${target_name} PRIVATE HAVE_CONFIG_H)
  target_include_directories(
    ${target_name}
    PRIVATE
    "${PORT_LIBCURL_WORKTREE}/include"
    "${PORT_LIBCURL_WORKTREE}/lib"
    "${PORT_LIBCURL_WORKTREE}/src"
    "${PORT_LIBCURL_WORKTREE}/tests/libtest"
    "${PORT_LIBCURL_WORKTREE}/tests/server"
    "${PORT_LIBCURL_WORKTREE}/tests/http/clients"
  )

  port_libcurl_json_get(role "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} role)
  if(role STREQUAL "libcurl-consumer" OR role STREQUAL "curl-tool")
    target_link_libraries(${target_name} PRIVATE compat_libcurl)
  endif()

  string(JSON has_cppflags ERROR_VARIABLE cppflags_missing GET "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} fields cppflags resolved)
  if(NOT cppflags_missing)
    separate_arguments(parsed_cppflags UNIX_COMMAND "${has_cppflags}")
    target_compile_options(${target_name} PRIVATE ${parsed_cppflags})
  endif()
endfunction()

port_libcurl_json_length(target_count "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets)
math(EXPR target_last "${target_count} - 1")
foreach(target_index RANGE 0 ${target_last})
  port_libcurl_json_get(target_id "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} target_id)
  port_libcurl_json_get(component "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} component)
  port_libcurl_json_get(target_name_raw "${PORT_LIBCURL_MANIFEST_JSON}" compatibility_build targets ${target_index} name)
  port_libcurl_target_name(cmake_target "${target_id}")
  port_libcurl_collect_sources(target_sources "${component}" "${target_index}")
  if(target_sources)
    add_executable(${cmake_target} ${target_sources})
    set_target_properties(${cmake_target} PROPERTIES OUTPUT_NAME "${target_name_raw}")
    port_libcurl_apply_common_flags(${cmake_target} "${component}" "${target_index}")
  endif()
endforeach()

find_package(PkgConfig REQUIRED)
pkg_check_modules(LDAP IMPORTED_TARGET ldap)
if(LDAP_FOUND)
  add_executable(
    compat_debian__ldap_bindata
    "${PORT_LIBCURL_WORKTREE}/debian/tests/LDAP-bindata.c"
  )
  target_compile_features(compat_debian__ldap_bindata PRIVATE c_std_11)
  target_compile_definitions(compat_debian__ldap_bindata PRIVATE HAVE_CONFIG_H)
  target_include_directories(
    compat_debian__ldap_bindata
    PRIVATE
    "${PORT_LIBCURL_WORKTREE}/include"
    "${PORT_LIBCURL_WORKTREE}/lib"
  )
  target_link_libraries(
    compat_debian__ldap_bindata
    PRIVATE
    compat_libcurl
    PkgConfig::LDAP
  )
  set_target_properties(compat_debian__ldap_bindata PROPERTIES OUTPUT_NAME "ldap-bindata")
endif()
