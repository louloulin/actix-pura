# PowerShell script to generate test data for DataFlare performance testing
param(
    [int]$RecordCount = 500000,
    [string]$OutputFile = "examples\data\extra_large_test.csv"
)

Write-Host "========================================" -ForegroundColor Cyan
Write-Host "DataFlare Test Data Generator" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Configuration:" -ForegroundColor Yellow
Write-Host "  Record Count: $RecordCount" -ForegroundColor White
Write-Host "  Output File: $OutputFile" -ForegroundColor White
Write-Host ""

# Create output directory if it doesn't exist
$outputDir = Split-Path -Parent $OutputFile
if (!(Test-Path $outputDir)) {
    Write-Host "Creating output directory: $outputDir" -ForegroundColor Green
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
}

Write-Host "Generating CSV file header..." -ForegroundColor Green
'id,name,email,age' | Out-File -FilePath $OutputFile -Encoding UTF8

Write-Host "Generating data records..." -ForegroundColor Green
$batchSize = 10000
$batches = [math]::Ceiling($RecordCount / $batchSize)

$startTime = Get-Date

for ($batch = 0; $batch -lt $batches; $batch++) {
    $startId = $batch * $batchSize + 1
    $endId = [math]::Min(($batch + 1) * $batchSize, $RecordCount)
    
    $records = @()
    for ($i = $startId; $i -le $endId; $i++) {
        $name = 'User' + $i.ToString('D6')
        $email = 'user' + $i + '@example.com'
        $age = Get-Random -Minimum 18 -Maximum 80
        $records += "$i,$name,$email,$age"
    }
    
    $records | Out-File -FilePath $OutputFile -Append -Encoding UTF8
    
    $progress = [math]::Round(($batch + 1) / $batches * 100, 1)
    $currentTime = Get-Date
    $elapsed = ($currentTime - $startTime).TotalSeconds
    $estimatedTotal = $elapsed / ($progress / 100)
    $remaining = $estimatedTotal - $elapsed
    
    Write-Host "Progress: $progress% (Batch $($batch + 1)/$batches) - ETA: $([math]::Round($remaining, 1))s" -ForegroundColor Cyan
}

$endTime = Get-Date
$totalTime = ($endTime - $startTime).TotalSeconds

Write-Host ""
Write-Host "Data generation completed!" -ForegroundColor Green
Write-Host "Total time: $([math]::Round($totalTime, 2)) seconds" -ForegroundColor White

if (Test-Path $OutputFile) {
    $fileSize = (Get-Item $OutputFile).Length / 1MB
    $lineCount = (Get-Content $OutputFile | Measure-Object -Line).Lines
    
    Write-Host "File size: $([math]::Round($fileSize, 2)) MB" -ForegroundColor White
    Write-Host "Line count: $lineCount" -ForegroundColor White
    Write-Host "Records per second: $([math]::Round($RecordCount / $totalTime, 0))" -ForegroundColor White
    
    Write-Host ""
    Write-Host "File preview:" -ForegroundColor Yellow
    Get-Content $OutputFile -Head 5 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
    Write-Host "  ..." -ForegroundColor Gray
    Get-Content $OutputFile -Tail 2 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
} else {
    Write-Host "Error: Output file was not created!" -ForegroundColor Red
    exit 1
}

Write-Host ""
Write-Host "Test data is ready for DataFlare CLI execution!" -ForegroundColor Green
