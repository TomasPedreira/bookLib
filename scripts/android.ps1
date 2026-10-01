param(
    [ValidateSet('init', 'dev', 'build')]
    [string]$Mode = 'build',
    [ValidateSet('aarch64', 'x86_64', 'armv7', 'i686')]
    [string]$Target = 'aarch64'
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Push-Location $projectRoot
try {
    if (-not $env:JAVA_HOME -and (Test-Path 'C:\Program Files\Java\jdk-21')) {
        $env:JAVA_HOME = 'C:\Program Files\Java\jdk-21'
    }
    if (-not $env:ANDROID_HOME) {
        $defaultSdk = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
        if (Test-Path $defaultSdk) { $env:ANDROID_HOME = $defaultSdk }
    }
    if (-not $env:ANDROID_HOME -or -not (Test-Path $env:ANDROID_HOME)) {
        throw 'Instala o SDK Android e define ANDROID_HOME. Consulta o README.'
    }
    if (-not $env:NDK_HOME) {
        $ndkFolder = Join-Path $env:ANDROID_HOME 'ndk'
        if (Test-Path $ndkFolder) {
            $ndk = Get-ChildItem $ndkFolder -Directory |
                Where-Object { $_.Name -match '^\d+\.\d+\.\d+$' } |
                Sort-Object { [version]$_.Name } | Select-Object -Last 1
            if ($ndk) { $env:NDK_HOME = $ndk.FullName }
        }
    }
    if (-not $env:JAVA_HOME -or -not (Test-Path (Join-Path $env:JAVA_HOME 'bin\java.exe'))) {
        throw 'Define JAVA_HOME para um JDK 17 ou superior.'
    }
    if (-not $env:NDK_HOME -or -not (Test-Path $env:NDK_HOME)) {
        throw 'Instala NDK (Side by side) no SDK Manager e define NDK_HOME.'
    }
    $env:PATH = "$(Join-Path $env:JAVA_HOME 'bin');$env:PATH"
    if (-not (Test-Path 'node_modules')) {
        npm ci
        if ($LASTEXITCODE -ne 0) { throw 'npm ci falhou' }
    }
    $rustTargets = @{ aarch64 = 'aarch64-linux-android'; x86_64 = 'x86_64-linux-android'; armv7 = 'armv7-linux-androideabi'; i686 = 'i686-linux-android' }
    rustup target add $rustTargets[$Target]
    if ($LASTEXITCODE -ne 0) { throw 'Não foi possível instalar o target Rust' }
    if (-not (Test-Path 'src-tauri\gen\android')) {
        npm run android:init -- --ci --skip-targets-install
        if ($LASTEXITCODE -ne 0) { throw 'Inicialização Android falhou' }
    }
    switch ($Mode) {
        'dev' { npm run android:dev }
        'build' {
            $artifactFolder = Join-Path $projectRoot 'artifacts'
            New-Item -ItemType Directory -Force -Path $artifactFolder | Out-Null
            $buildLog = Join-Path $artifactFolder 'android-build.log'
            npm run tauri -- android build --debug --apk --target $Target 2>&1 |
                Tee-Object -FilePath $buildLog
            $tauriExit = $LASTEXITCODE
            if ($tauriExit -ne 0) {
                $log = Get-Content -LiteralPath $buildLog -Raw
                if ($log -notmatch 'Creation symbolic link is not allowed') {
                    throw 'Compilação Android falhou. Consulta artifacts/android-build.log.'
                }
                # Tauri has already built Rust and generated the plugin bindings.
                # Package a copy when Windows does not allow file symlinks.
                $abis = @{ aarch64 = 'arm64-v8a'; x86_64 = 'x86_64'; armv7 = 'armeabi-v7a'; i686 = 'x86' }
                $flavors = @{ aarch64 = 'Arm64'; x86_64 = 'X86_64'; armv7 = 'Arm'; i686 = 'X86' }
                $nativeSource = Join-Path $projectRoot "target/$($rustTargets[$Target])/debug/libbooklib_app.so"
                if (-not (Test-Path $nativeSource)) { throw 'Biblioteca Android compilada não encontrada' }
                $nativeFolder = Join-Path $projectRoot "src-tauri/gen/android/app/src/main/jniLibs/$($abis[$Target])"
                New-Item -ItemType Directory -Force -Path $nativeFolder | Out-Null
                $nativeCopy = Join-Path $nativeFolder 'libbooklib_app.so'
                Copy-Item -LiteralPath $nativeSource -Destination $nativeCopy -Force
                & "$env:NDK_HOME/toolchains/llvm/prebuilt/windows-x86_64/bin/llvm-strip.exe" --strip-debug $nativeCopy
                if ($LASTEXITCODE -ne 0) { throw 'Não foi possível remover os símbolos de depuração' }
                $flavor = $flavors[$Target]
                & './src-tauri/gen/android/gradlew.bat' --project-dir src-tauri/gen/android "assemble${flavor}Debug" -x "rustBuild${flavor}Debug" --console=plain --no-daemon
                if ($LASTEXITCODE -ne 0) { throw 'Empacotamento Gradle falhou' }
            }
            $flavorFolders = @{ aarch64 = 'arm64'; x86_64 = 'x86_64'; armv7 = 'arm'; i686 = 'x86' }
            $apkFolder = Join-Path $projectRoot "src-tauri/gen/android/app/build/outputs/apk/$($flavorFolders[$Target])/debug"
            # The normal Tauri command may produce a universal APK with one ABI.
            if (-not (Test-Path $apkFolder)) {
                $apkFolder = Join-Path $projectRoot 'src-tauri/gen/android/app/build/outputs/apk/universal/debug'
            }
            $apk = Get-ChildItem -LiteralPath $apkFolder -Filter '*.apk' | Select-Object -First 1
            if (-not $apk) { throw 'APK não encontrado após a compilação' }
            $version = (Get-Content src-tauri/tauri.conf.json -Raw | ConvertFrom-Json).version
            $destination = Join-Path $artifactFolder "BookLib-$version-$Target.apk"
            Copy-Item -LiteralPath $apk.FullName -Destination $destination -Force
            Write-Host "APK pronto: $destination"
        }
        'init' { Write-Host 'Projeto Android inicializado.'; return }
    }
    if ($LASTEXITCODE -ne 0) { throw "Tauri Android $Mode falhou" }
} finally { Pop-Location }
