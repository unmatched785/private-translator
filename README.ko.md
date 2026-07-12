# Private Translator

[English](README.md) · [보안 정책](SECURITY.md) · [기여 안내](CONTRIBUTING.md)

Private Translator는 웹 번역기처럼 쓰되 원문, 번역문, 기록을 인터넷 사이트로 보내지 않는 Windows 우선 오프라인 번역기입니다. 화면은 평소 쓰는 브라우저에서 열리고, 작은 로컬 실행 파일이 모델과 암호화 기록고를 관리합니다.

> **v0.1.1 상태:** Windows x64에서 실제 모델과 기록 흐름을 검증했지만 아직 코드 서명은 없습니다. Windows가 알 수 없는 게시자 경고를 표시할 수 있습니다.

## 주요 기능

- Tencent Hy-MT2 1.8B Q4와 `llama.cpp`를 PC 안에서 자동 실행·종료
- 공식 Hy-MT2 표에 따른 38개 언어 항목
- 붙여넣기 자동 번역과 `Ctrl+Enter`
- 긴 문서 토큰 기반 분할 및 잘린 구간 재시도
- 검색 가능한 AES-256-GCM 암호화 번역 기록
- 좋은 번역만 승인하는 번역 자산과 덮어쓰지 않는 수정 버전
- 숫자와 URL 보존 점검
- 모델·런타임 SHA-256 및 실행 중 모델 ID 확인
- loopback 전용 서버, 클라우드 폴백·텔레메트리·분석 도구 없음
- 영어 기본 UI와 한국어 전환

## 다운로드와 실행

1. [최신 릴리즈](https://github.com/unmatched785/private-translator/releases/latest)에서 `PrivateTranslator-v0.1.1-Windows-x64.zip`을 받습니다.
2. ZIP을 일반 폴더에 풉니다.
3. 인터넷에 연결된 상태에서 `Install-Model.cmd`를 한 번 더블클릭합니다. 고정된 공식 Hy-MT2 리비전에서 약 1.13GB를 내려받고, 중단 시 이어받으며 정확한 크기와 SHA-256을 확인합니다.
4. 보통은 `PrivateTranslator-Lite.exe`를 더블클릭합니다.
5. 브라우저에서 쓰는 동안 프로그램 창을 열어 둡니다. 창을 닫으면 로컬 모델도 함께 종료됩니다.

작은 릴리즈 묶음에는 두 실행 파일과 고정된 `llama.cpp` 런타임만 들어가고 GGUF 모델은 넣지 않습니다. 두 판은 `%LOCALAPPDATA%\PrivateTranslator\models`에 한 번 설치한 검증 모델을 함께 씁니다.

| 실행 파일 | 용도 |
| --- | --- |
| `PrivateTranslator-Lite.exe` | 일반 사무용 노트북용 권장 기본판 |
| `PrivateTranslator-Quality.exe` | 같은 기본 모델에 선택형 개인 7B 서버 항목을 더한 판 |

Quality도 인터넷 번역 API로 자동 전환하지 않습니다. v0.1.1은 용량이 큰 7B 모델을 설치하지 않습니다. Windows 10/11 x64, 시스템 메모리 8GB 이상을 권장합니다. 모델 설치 뒤 평소 번역에는 인터넷이 필요 없습니다. 완전 오프라인 전달용으로 기존 [v0.1.0 전체 묶음](https://github.com/unmatched785/private-translator/releases/tag/v0.1.0)도 그대로 유지합니다.

모델 확인과 고급 설치 명령:

```powershell
.\PrivateTranslator-Lite.exe model status
.\PrivateTranslator-Lite.exe model verify
.\PrivateTranslator-Lite.exe setup --portable
.\PrivateTranslator-Lite.exe setup --from C:\경로\Hy-MT2-1.8B-Q4_K_M.gguf
```

## 지원 언어

한국어, 영어, 일본어, 중국어 간체·번체, 프랑스어, 독일어, 스페인어, 포르투갈어, 이탈리아어, 러시아어, 아랍어, 튀르키예어, 태국어, 베트남어, 인도네시아어, 말레이어, 필리핀어, 힌디어, 폴란드어, 체코어, 네덜란드어, 우크라이나어, 히브리어, 페르시아어, 벵골어, 타밀어, 텔루구어, 마라티어, 구자라트어, 우르두어, 크메르어, 미얀마어, 티베트어, 카자흐어, 몽골어, 위구르어, 광둥어를 표시합니다.

목록은 [공식 Hy-MT2 모델 카드](https://huggingface.co/tencent/Hy-MT2-1.8B-GGUF)를 따릅니다. 모든 언어 조합의 품질이 같다는 뜻은 아닙니다.

## 로컬 기록고와 번역 자산

원문, 번역문, 언어, 모델 정보, 처리 시간, QA 경고는 AES-256-GCM으로 암호화합니다. Windows에서는 무작위 기록 키를 현재 Windows 계정의 DPAPI로 보호합니다.

```text
%LOCALAPPDATA%\PrivateTranslator\history.db
%LOCALAPPDATA%\PrivateTranslator\vault.key
%LOCALAPPDATA%\PrivateTranslator\models\Hy-MT2-1.8B-Q4_K_M.gguf
```

기록 저장 스위치를 끈 요청은 DB에 넣지 않습니다. 일반 번역 기록과 사용자가 승인한 번역 자산은 별개입니다. 기록을 삭제해도 승인 자산은 유지되고, 자산을 수정하면 이전 내용을 덮지 않고 v2, v3처럼 새 암호화 버전을 추가합니다.

이 보호는 디스크 파일의 단순 열람과 다른 Windows 사용자를 대상으로 합니다. 로그인한 사용자 권한의 악성 프로그램, 키로거, 화면 캡처, 관리자, 메모리 덤프까지 막는 보안 제품은 아닙니다.

## 소스에서 빌드

Windows x64, Rust stable, PowerShell이 필요합니다. Node.js는 화면 코드 검사와 모의 서버에만 사용합니다.

```powershell
.\scripts\bootstrap.ps1
.\scripts\check.ps1
$env:CARGO_HOME = Join-Path (Get-Location) '.cache\cargo'
cargo build --locked --release
.\scripts\package.ps1
.\scripts\release.ps1
```

`bootstrap.ps1`은 개발·실동작 검증용 고정 모델과 `llama.cpp` 런타임을 내려받고 크기와 SHA-256을 확인합니다. `package.ps1`과 `release.ps1`이 만드는 배포 ZIP은 모든 GGUF를 제외하며 150MB 크기 제한을 적용합니다. 자세한 구조와 개인정보 경계는 [architecture-v1.md](docs/architecture-v1.md), [privacy-contract.md](docs/privacy-contract.md)를 참고하세요.

앱 코드는 [MIT License](LICENSE)입니다. Hy-MT2와 `llama.cpp`는 각 upstream 라이선스를 유지하며 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)에 출처를 적었습니다.
