$initialDir = Get-Location

# Release to crates-io
$releaseDirs = @(
    "rmk-macro",
    "rmk"
)

foreach ($dir in $releaseDirs) {
    Set-Location $dir
    cargo release --registry crates-io patch --execute
    Set-Location $initialDir
}