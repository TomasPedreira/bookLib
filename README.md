# BookLib para Android

App pessoal de biblioteca e acompanhamento de leituras, feita com Tauri 2, Rust e HTML/CSS/JavaScript. Os livros, leituras, classificações, notas e progresso ficam em SQLite na pasta privada da app. Não há servidor HTTP, contas ou sincronização. Pesquisa no catálogo e capas usam a Open Library; a biblioteca local funciona sem internet.

## Preparar o Android no Windows

1. Instala Node.js 22.12+ e Rust. `rust-toolchain.toml` seleciona a versão Rust deste projeto.
2. Instala Android Studio e um JDK 17 ou superior (recomendado: o JDK incluído no Android Studio).
3. No SDK Manager instala Android SDK Platform 36, Platform-Tools, Build-Tools 36.0.0, Command-line Tools e NDK (Side by side). Aceita as licenças do SDK durante a instalação.
4. Define as variáveis na sessão PowerShell, ajustando os caminhos à instalação:

```powershell
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:ANDROID_HOME = "$env:LOCALAPPDATA\Android\Sdk"
$env:NDK_HOME = "$env:ANDROID_HOME\ndk\VERSAO_INSTALADA"
npm ci
```

Consulta também os [pré-requisitos oficiais do Tauri](https://v2.tauri.app/start/prerequisites/).

## Gerar e instalar o APK pessoal

```powershell
.\scripts\android.ps1 -Mode build
```

O script valida o ambiente, instala o target Rust ARM64, inicializa o projeto Android em `src-tauri/gen/android` e gera um APK **debug assinado**, instalável diretamente no telemóvel. O script copia o APK para `artifacts/BookLib-0.2.5-aarch64.apk` (o nome acompanha a versão e o target). Os APKs originais ficam em `src-tauri/gen/android/app/build/outputs/apk/`. Transfere-o para o Android e abre-o para instalar, ou usa `adb install -r CAMINHO_DO_APK` com depuração USB ativa.

Se o Windows não permitir ligações simbólicas, o modo build usa automaticamente uma cópia da biblioteca Rust e empacota-a com Gradle, sem alterar as definições do sistema. O APK distribuído não inclui um servidor de desenvolvimento.

Para desenvolver com um dispositivo ligado:

```powershell
.\scripts\android.ps1 -Mode dev
```

Para um emulador x86_64, usa `-Target x86_64`. O modo dev escolhe o dispositivo; `-Target` instala o target Rust necessário e escolhe a arquitetura no modo build. O servidor Vite existe apenas durante desenvolvimento. O APK contém a interface e o Rust e funciona sozinho.

Mantém a mesma chave de assinatura nas atualizações. Para este APK debug, guarda a chave local gerada pelo Android em `%USERPROFILE%\.android\debug.keystore`; mudar a chave impede atualizar a instalação existente. Desinstalar a app apaga a base de dados privada, pelo que deves exportar uma cópia antes.

## Dados e backups

Durante esta fase de testes, correções pequenas atualizam o APK da release existente, mantendo a versão. As notas da release indicam o commit e o checksum do APK mais recente.

Em **My data**, exporta ou restaura um JSON usando o seletor de documentos do Android. Restaurar substitui a biblioteca atual numa transação; uma importação inválida preserva os dados existentes. São aceites as versões 1 e 2 dos backups anteriores.

Se tens a base de dados da antiga versão web, podes criar uma cópia para importar no telemóvel:

```powershell
cargo run --example export_backup -- booklib.sqlite booklib-backup.json
```

O destino não é sobrescrito se já existir. O comando abre a base existente e aplica eventuais migrações pendentes. A app Android não copia automaticamente ficheiros deste PC.

## Verificar o projeto

```powershell
cargo test -p booklib
npm test
npm run build
cargo check -p booklib-app
```

O último comando verifica o shell Tauri no sistema anfitrião; não substitui a compilação nem o teste no Android. `npm run dev` sozinho serve os assets, mas as operações de biblioteca precisam do runtime Tauri.

## Estrutura

- `src/lib.rs`: inicialização SQLite, cliente do catálogo e erros serializáveis.
- `src/library.rs`: operações da biblioteca, leituras, progresso, estatísticas e backups, independentes do transporte.
- `src/catalog.rs`: integração Open Library, limite de pedidos e cache temporária.
- `src-tauri/`: shell da app, comandos nativos e permissões.
- `web/services.js`: chamadas estruturadas aos comandos Rust.
- `web/platform.js`: documentos, confirmações, links externos e botão voltar.
- `web/state.js`, `web/topics.js`, `web/errors.js`, `web/backup.js`: estado, temas e validação.
- `web/app.js`, `web/style.css`, `web/index.html`: interface.
- `migrations/`: esquema SQLite, preservado da versão anterior.

Os testes de interface usam DOM e comandos nativos simulados. Seletores de documentos, permissões, teclado, botão voltar e comportamento ao suspender a app devem ser confirmados num dispositivo Android.

## Ambiente de compilação verificado

O APK ARM64 foi compilado neste PC com JDK 21, SDK Platform 36, Build-Tools 36.0.0 e NDK 29.0.14206865. Foram verificadas a assinatura APK v2, a arquitetura ARM64 e o alinhamento de 16 KB. O projeto Android gerado em `src-tauri/gen/android` usa compileSdk/targetSdk 36 e minSdk 24 (Android 7). A execução num telemóvel real continua a precisar de validação.
