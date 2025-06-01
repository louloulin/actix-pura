@echo off
echo ========================================
echo DataFlare 大数据性能测试
echo ========================================
echo.

echo 📊 测试配置:
echo    - 数据量: 500,000 条记录
echo    - 文件大小: ~91 MB
echo    - 批次大小: 5,000 条/批次
echo    - 预期批次数: ~100 批次
echo.

echo 🚀 开始执行大数据工作流...
echo 开始时间: %date% %time%
echo.

set start_time=%time%

target\release\dataflare.exe execute -f examples\workflows\large_data_test.yaml

set end_time=%time%

echo.
echo ========================================
echo 📈 性能测试结果
echo ========================================
echo 开始时间: %start_time%
echo 结束时间: %end_time%
echo.

if exist "dataflare\output\large_data_output.csv" (
    echo ✅ 输出文件已生成
    for %%A in ("dataflare\output\large_data_output.csv") do (
        echo 📁 输出文件大小: %%~zA 字节
    )
    
    echo.
    echo 📊 验证输出文件行数...
    powershell -Command "(Get-Content 'dataflare\output\large_data_output.csv' | Measure-Object -Line).Lines"
) else (
    echo ❌ 输出文件未生成
)

echo.
echo 🎯 性能测试完成！
pause
