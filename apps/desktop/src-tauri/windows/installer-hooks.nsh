; RepoDNA NSIS installer version-resource metadata.
; Tauri includes installerHooks at the top level of its NSIS script. The
; default installer template sets the product and version but omits CompanyName.
; This is a compile-time directive, not an install/uninstall command.
VIAddVersionKey "CompanyName" "RepoDNA"
