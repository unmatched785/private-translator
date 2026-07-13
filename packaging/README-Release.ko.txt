Private Translator 0.1.2 Windows x64
======================================

처음 한 번
1. ZIP 전체를 일반 폴더에 풉니다.
2. 인터넷에 연결된 상태에서 Install-Model.exe를 한 번 더블클릭합니다. 고정된 공식 Hy-MT2 리비전에서 약 1.13GB를 내려받고, 중단 시 이어받으며 크기와 SHA-256을 확인합니다.
3. 보통은 PrivateTranslator-Lite.exe를 실행합니다. 선택형 개인 7B 서버 항목이 필요할 때만 Quality를 사용합니다.

앱 업데이트를 작게 유지하려고 릴리즈 ZIP에는 GGUF 모델을 넣지 않았습니다. 두 실행 파일은 %LOCALAPPDATA%\PrivateTranslator\models의 검증된 모델을 함께 씁니다. 설치 뒤 평소 번역은 오프라인으로 동작합니다.

평소 번역 통신과 암호화 기록고는 PC 안에 남습니다. 기본 경로에서 인터넷을 쓰는 것은 사용자가 직접 시작한 모델 설치뿐이며, Quality도 클라우드로 자동 전환하지 않습니다.

loopback을 포함한 모든 Quality 개인 네트워크 엔드포인트는 Bearer 인증이 필수입니다. 7B를 활성화하려면 앱 실행 전에 PRIVATE_TRANSLATOR_QUALITY_API_KEY를 설정하고 외부 llama-server도 --api-key에 같은 값을 전달해 실행해야 합니다. 미설정 시 7B만 사용 불가이고 요청은 네트워크 접근 전에 차단됩니다. HTTP는 127.0.0.1과 ::1 loopback 주소에서만 허용합니다. 비-loopback 서버는 호스트명이 아닌 IP 주소를 직접 적은 HTTPS URL, 해당 IP SAN, Windows가 신뢰하는 내부 CA 체인이 추가로 필요합니다.

공개용 ZIP은 Lite·Quality·Install-Model 실행 파일의 유효한 Authenticode 서명과 RFC 3161 타임스탬프를 필수로 확인하며 실행용 래퍼 스크립트를 넣지 않습니다. upstream llama-server.exe는 고정 크기와 SHA-256으로 검증합니다. UNSIGNED-DEVELOPMENT 묶음은 로컬 검증 전용이며 공개하면 안 됩니다. 함께 제공되는 SHA-256 파일로 ZIP을 확인할 수 있고, supply-chain 폴더에는 CycloneDX SBOM, 고정 Rust 의존성 목록, crate 라이선스 원문이 들어 있습니다.
