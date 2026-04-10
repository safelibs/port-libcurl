set(PORT_LIBCURL_SAFE_MANIFEST "${CMAKE_CURRENT_LIST_DIR}/../metadata/test-manifest.json")

function(port_libcurl_json_get out json)
  string(JSON _value GET "${json}" ${ARGN})
  set(${out} "${_value}" PARENT_SCOPE)
endfunction()

function(port_libcurl_json_length out json)
  string(JSON _value LENGTH "${json}" ${ARGN})
  set(${out} "${_value}" PARENT_SCOPE)
endfunction()

function(port_libcurl_target_name out target_id)
  string(REPLACE ":" "__" _name "${target_id}")
  string(REPLACE "-" "_" _name "${_name}")
  set(${out} "compat_${_name}" PARENT_SCOPE)
endfunction()
