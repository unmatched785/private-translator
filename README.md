# Private Translator

브라우저처럼 사용하지만 번역 엔진과 기록은 개인 장치 안에서만 처리하는 오프라인 번역기입니다.

현재 MVP에는 다음이 구현되어 있습니다.

- Hy-MT2 1.8B Q4 모델과 `llama.cpp` CPU 엔진 자동 시작/종료
- 붙여넣기 자동 번역과 `Ctrl+Enter` 번역
- Lite/Quality 실행 파일 이름에 따른 프로필 자동 선택
- 로컬 암호화 기록 검색, 즐겨찾기, 다시 열기, 삭제
- 요청별 기록 저장 끄기
- 숫자와 URL 보존 점검
- 외부 주소 차단과 loopback 전용 웹 서버

## 바로 실행

개발 빌드를 직접 실행하려면 다음 파일을 더블클릭합니다.

```text
target\release\private-translator.exe
```

`runtime/llama.cpp/llama-server.exe`와 `models/Hy-MT2-1.8B-Q4_K_M.gguf`가 있으면 모델 엔진을 자동으로 준비한 뒤 기본 브라우저를 엽니다. 사용을 마치면 프로그램 창을 닫습니다. 강제 종료되더라도 이 앱이 시작한 모델 엔진은 함께 종료됩니다.

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

SQLite에는 무작위 ID, 생성 시각, 즐겨찾기, nonce, 암호문만 평문으로 저장됩니다. 원문, 번역문, 언어, 모델, 처리 시간과 QA 경고는 하나의 AES-256-GCM 암호문 안에 들어갑니다.

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
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
cargo build --release
node --check .\web\app.js
```

모델 없이 UI와 기록 흐름만 시험할 때는 데모 프로필을 사용합니다.

```powershell
$env:TRANSLATOR_DATA_DIR = Join-Path (Get-Location) '.data\demo'
cargo run -- --profile demo --no-open
```

## 현재 확인된 동작

- 실제 Hy-MT2 영→한 번역 성공
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
- 개인 용어집과 승인 번역 메모리
- 긴 문서 분할 및 문맥 연결
- 모델 상태 표시와 사용 중 모델 교체

구성 요소 버전과 SHA-256은 `packaging/component-manifest.json`에 고정되어 있습니다. 라이선스 고지는 `THIRD_PARTY_NOTICES.md`를 참고하세요.

상세 설계는 `docs/architecture-v1.md`와 `docs/privacy-contract.md`에 있습니다.
