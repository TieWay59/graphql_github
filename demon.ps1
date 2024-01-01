# 设置 cargo run 的路径和参数
$commandPath = "C:\Users\tieway59\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin\cargo.exe"
$commandArguments = "run"

# 定义守护进程函数
function Start-Daemon {
    while ($true) {
        # 启动 cargo run
        $process = Start-Process -FilePath $commandPath -ArgumentList $commandArguments -PassThru

        # 监视进程
        $process.WaitForExit()

        # 进程退出后等待一段时间再重新启动
        Start-Sleep -Seconds 5
    }
}

# 启动守护进程
Start-Daemon
