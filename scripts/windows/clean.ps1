$ErrorActionPreference = "Stop"

cargo clean

$directories = @(
    ".pytest_cache",
    ".ruff_cache"
)

foreach ($directory in $directories) {
    if (Test-Path -LiteralPath $directory) {
        Remove-Item -LiteralPath $directory -Recurse -Force
    }
}

$pending = [System.Collections.Generic.Stack[string]]::new()
$pending.Push((Get-Location).Path)
while ($pending.Count -gt 0) {
    foreach ($item in Get-ChildItem -LiteralPath $pending.Pop() -Force) {
        if ($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) {
            continue
        }
        if ($item.PSIsContainer) {
            if ($item.Name -in @(".git", "work_dir")) { continue }
            if ($item.Name -eq "__pycache__") {
                Remove-Item -LiteralPath $item.FullName -Recurse -Force
            } else {
                $pending.Push($item.FullName)
            }
        } elseif ($item.Extension -eq ".pyc") {
            Remove-Item -LiteralPath $item.FullName -Force
        }
    }
}
