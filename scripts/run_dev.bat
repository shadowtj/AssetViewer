@echo off
setlocal
pushd "%~dp0..\"
cargo run
popd
endlocal
