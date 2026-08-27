param(
    [switch]$ValidateOnly,
    [string]$DownloadRoot,
    [string]$Ledger,
    [string]$PublisherProxy = "http://127.0.0.1:3456"
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$showcaseRoot = Split-Path -Parent $scriptDir
$repoRoot = Split-Path -Parent $showcaseRoot

function Resolve-PrimaryRepoRoot([string]$CurrentRepoRoot) {
    try {
        $commonDir = (& git -C $CurrentRepoRoot rev-parse --git-common-dir 2>$null)
        if ($LASTEXITCODE -eq 0 -and $commonDir) {
            $trimmed = $commonDir.Trim()
            if (-not [System.IO.Path]::IsPathRooted($trimmed)) {
                $trimmed = [System.IO.Path]::GetFullPath((Join-Path $CurrentRepoRoot $trimmed))
            }
            $candidate = Split-Path -Parent $trimmed
            if (Test-Path $candidate -PathType Container) {
                return $candidate
            }
        }
    } catch {}
    return $CurrentRepoRoot
}

if (-not $DownloadRoot) {
    $DownloadRoot = Join-Path $repoRoot "showcase\output\release-run\downloads"
}
if (-not $Ledger) {
    if ($env:READMD_PUBLICATION_LEDGER) {
        $Ledger = $env:READMD_PUBLICATION_LEDGER
    } else {
        $primaryRoot = Resolve-PrimaryRepoRoot $repoRoot
        $primaryCandidate = Join-Path $primaryRoot "showcase\content\publication-ledger.jsonl"
        $localCandidate = Join-Path $repoRoot "showcase\content\publication-ledger.jsonl"
        if (Test-Path $primaryCandidate -PathType Leaf) {
            $Ledger = $primaryCandidate
        } elseif (Test-Path $localCandidate -PathType Leaf) {
            $Ledger = $localCandidate
        } else {
            throw "Publication ledger not found at '$primaryCandidate' or '$localCandidate'. Specify -Ledger or set READMD_PUBLICATION_LEDGER."
        }
    }
}

if (-not (Test-Path $DownloadRoot -PathType Container)) {
    throw "Review download directory does not exist: $DownloadRoot"
}

# Pick the most recent generated confirmation request so the operator does not
# have to copy long paths after reviewing the PDF.
$request = Get-ChildItem -LiteralPath $DownloadRoot -Filter "*.approval-request.json" |
    Sort-Object LastWriteTime -Descending |
    Select-Object -First 1
if (-not $request) {
    throw "No poster approval request found in: $DownloadRoot"
}

$requestData = Get-Content -LiteralPath $request.FullName -Raw | ConvertFrom-Json
$batch = Join-Path $DownloadRoot $requestData.batch
$pdf = Join-Path $DownloadRoot $requestData.review_pdf
foreach ($path in @($batch, $pdf)) {
    if (-not (Test-Path $path -PathType Leaf)) {
        throw "Approved review artifact is missing: $path"
    }
}

$pythonScript = Join-Path $scriptDir "publish_approved_batch.py"
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$arguments = @(
    $pythonScript,
    "--batch", $batch,
    "--approval-request", $request.FullName,
    "--root", $DownloadRoot,
    "--work-dir", (Join-Path $DownloadRoot "one-click-work-$stamp"),
    "--state", (Join-Path $DownloadRoot "one-click-state-$stamp.json"),
    "--ledger", $Ledger,
    "--publisher-proxy", $PublisherProxy
)
if ($ValidateOnly) {
    $arguments += "--validate-only"
}

Write-Host "Review PDF: $pdf"
Write-Host "Batch file: $batch"
Write-Host "Mode: $(if ($ValidateOnly) { 'validate only' } else { 'publish' })"
& python @arguments
exit $LASTEXITCODE
