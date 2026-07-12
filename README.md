# Private Translator

브라우저처럼 사용하지만 번역 엔진과 기록은 개인 장치 안에서만 처리하는 오프라인 번역기입니다.

현재 MVP에는 다음이 구현되어 있습니다.

- Hy-MT2 1.8B Q4 모델과 `llama.cpp` CPU 엔진 자동 시작/종료
- 붙여넣기 자동 번역과 `Ctrl+Enter` 번역
- 토큰 예산에 맞춘 긴 문서 자동 분할과 잘림 재시도
- Lite/Quality 실행 파일 이름에 따른 프로필 자동 선택
- 로컬 암호화 기록 검색, 즐겨찾기, 다시 열기, 삭제
- 좋은 기록만 승인하는 번역 자산과 덮어쓰지 않는 수정 버전
- 요청별 기록 저장 끄기
- 숫자와 URL 보존 점검
- 모델·런타임 SHA-256 및 실행 중인 모델 ID 확인
- 외부 주소 차단과 loopback 전용 웹 서버

## 바로 실행

개발 빌드를 실행하려면 다음 스크립트를 사용합니다.

```text
scripts\run-lite.ps1
```

완성된 휴대용 폴더에서는 `PrivateTranslator-Lite.exe`를 더블클릭합니다. 앱은 실행 파일과 같은 묶음 안의 `runtime/llama.cpp/llama-server.exe`와 `models/Hy-MT2-1.8B-Q4_K_M.gguf`만 사용합니다. 시작 전에 파일 크기와 SHA-256을 확인하고, 엔진이 준비된 뒤 실제 모델 ID도 확인합니다. 사용을 마치면 프로그램 창을 닫습니다. 강제 종료되더라도 이 앱이 시작한 모델 엔진은 함께 종료됩니다.

두 개의 완전한 휴대용 폴더를 만들려면 다음을 실행합니다.

```powershell
.\scripts\package.ps1
```

결과:

```text
dist\PrivateTranslator-Lite\PrivateTranslator-Lite.exe
dist\PrivateTranslator-Quality\PrivateTranslator-Quality.exe
```

포장 과정은 1.13GB 모델을 하드링크해 개발 PC의 디스크를 중복 사용하지 않습니다. 폴더를 다른 PC로 복사하면 일반 파일처럼 함께 복사됩니다.

## 두 프로필

### Lite

- 일반 사무용 노트북 CPU 우선
- Hy-MT2 1.8B Q4 한 개만 표시
- CPU 논리 코어 수에 따라 최대 8스레드 자동 사용
- 모델 API `http://127.0.0.1:8080/v1`
- 브라우저 UI `http://127.0.0.1:8173`

### Quality

- Lite와 같은 암호화 기록고 사용
- 기본 1.8B 모델은 자동 시작
- Hy-MT2 7B와 TranslateGemma 12B 선택 항목 제공
- 7B/12B는 추가 모델팩 또는 개인 Mac mini/PC 서버가 필요
- 클라우드 API와 자동 폴백은 기본값에 없음

Quality 실행 파일은 이름에 `Quality`가 들어 있으면 자동으로 Quality 프로필을 사용합니다. 사용자 설정은 `--config <파일>`로 교체할 수 있습니다.

## 암호화 기록고

SQLite에는 무작위 ID, 생성·수정 시각, 즐겨찾기, 자산 버전 번호, nonce, 암호문 같은 구조 메타데이터만 평문으로 저장됩니다. 원문, 번역문, 언어, 모델, 처리 시간과 QA 경고는 AES-256-GCM 암호문 안에 들어갑니다.

자동으로 쌓이는 번역 기록과 사용자가 직접 승인한 번역 자산은 별개입니다. 기록을 삭제해도 승인 자산은 유지되며, 승인 자산을 수정하면 이전 내용을 덮어쓰지 않고 v2, v3처럼 새 암호화 버전을 추가합니다. 자산과 모든 버전 삭제 역시 화면에서 별도로 실행할 수 있습니다.

Windows에서는 무작위 256비트 기록 키를 현재 Windows 사용자 계정의 DPAPI로 보호합니다.

```text
%LOCALAPPDATA%\PrivateTranslator\history.db
%LOCALAPPDATA%\PrivateTranslator\vault.key
```

프로그램 폴더를 지워도 기록은 자동 삭제하지 않습니다. 기록 저장 스위치를 끈 요청은 DB에 넣지 않습니다.

## 개인정보 보호 경계

- 앱과 기본 모델은 `127.0.0.1`에만 바인딩됩니다.
- 외부 분석 도구, 원격 폰트, 텔레메트리를 사용하지 않습니다.
- 원문과 번역문을 콘솔 또는 모델 로그에 남기지 않습니다.
- 브라우저 저장소에는 대상 언어와 선택 모델 같은 비민감 설정만 저장합니다.
- 장치 모델은 loopback, 개인 서버 모델은 사설 IP, `.local`, 단일 호스트명 또는 Tailscale CGNAT 대역만 허용합니다.
- 인터넷 API는 설정 검증 단계에서 거부합니다.

## 개발과 검증

```powershell
$env:CARGO_HOME = Join-Path (Get-Location) '.cache\cargo'
$env:TRANSLATOR_BUNDLE_DIR = (Get-Location).Path
cargo fmt --all -- --check
cargo test --offline --all-targets
cargo clippy --offline --all-targets -- -D warnings
cargo build --offline --release
node --check .\web\app.js
```

모델 없이 UI와 기록 흐름만 시험할 때는 데모 프로필을 사용합니다.

```powershell
$env:TRANSLATOR_DATA_DIR = Join-Path (Get-Location) '.data\demo'
$env:TRANSLATOR_BUNDLE_DIR = (Get-Location).Path
cargo run -- --profile demo --no-open
```

## 현재 확인된 동작

- 실제 Hy-MT2 영→한 번역 성공
- 5,938자 사무 문서를 2개 구간으로 자동 분할해 누락 없이 번역
- 1.13GB 모델과 51개 llama.cpp EXE/DLL의 시작 전 SHA-256 검증 성공
- 실행 엔진의 `/v1/models` ID 확인 후에만 요청 수락
- 모델별 직렬 대기열과 브라우저 탭별 최신 요청 우선 처리
- 번역 기록→승인 자산→v2 수정→기록 삭제 후 자산 보존 흐름 검증
- 저장 작업을 비동기 웹 처리와 분리하고 기존 DB를 v2로 무손실 마이그레이션
- 이 개발 PC에서 짧은 사무 문장 약 1.5초, 생성 약 24 tokens/s 관측
- 저장 번역은 암호화 기록에서 다시 열림
- 기록 저장을 끈 요청은 기록 수가 늘지 않음
- DB 바이너리에서 시험 원문/번역문 평문이 검색되지 않음
- 앱을 강제 종료해도 자동 시작한 `llama-server`가 남지 않음

이 수치는 개발 PC 관측값이며 일반 사무용 노트북에서는 CPU와 메모리에 따라 달라집니다.

## 남은 배포 작업

- Windows 코드 서명과 설치 프로그램
- Quality 7B/12B 모델팩 실측 비교
- 암호화 백업/복원
- 개인 용어집과 승인 자산의 문맥 검색·자동 제안
- 모델 상태 표시와 사용 중 모델 교체

구성 요소 버전과 SHA-256은 `packaging/component-manifest.json`에 고정되어 있습니다. 라이선스 고지는 `THIRD_PARTY_NOTICES.md`를 참고하세요.

상세 설계는 `docs/architecture-v1.md`와 `docs/privacy-contract.md`에 있습니다.
