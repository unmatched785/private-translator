Private Translator Quality 0.1.2
===================================

1. 인터넷에 연결된 상태에서 Install-Model.exe를 더블클릭해 공용 Hy-MT2 1.8B 모델(약 1.13GB)을 설치·검증합니다.
2. PrivateTranslator-Quality.exe를 더블클릭합니다.
3. 브라우저에서 쓰는 동안 프로그램 창을 열어 둡니다.

선택형 Hy-MT2 7B 항목은 허용된 개인 주소의 호환 모델 서버가 필요합니다. loopback을 포함한 모든 개인 네트워크 엔드포인트는 Bearer 인증이 필수입니다. 7B를 활성화하려면 앱 실행 전에 PRIVATE_TRANSLATOR_QUALITY_API_KEY를 설정하고, 외부 서버도 `llama-server --api-key`에 같은 값을 전달해 실행해야 합니다. 값이 없거나 비어 있으면 7B만 사용 불가이고 관리형 1.8B는 계속 동작하며, 7B 직접 요청도 네트워크 접근 전에 거부됩니다. `127.0.0.1` 또는 `::1` loopback 서버만 HTTP를 쓸 수 있습니다. 비-loopback 서버는 호스트명이 아닌 IP 주소를 직접 적은 HTTPS URL만 허용하며, 인증서에는 해당 IP SAN이 있고 Windows가 신뢰하는 내부 CA 체인이어야 합니다. 클라우드 번역 API로 자동 전환하지 않으며, 첫 Quality 릴리즈는 7B 모델팩을 포함하거나 자동 설치하지 않습니다.

공용 모델 위치:
%LOCALAPPDATA%\PrivateTranslator\models

확인 명령:
PrivateTranslator-Quality.exe model status
PrivateTranslator-Quality.exe model verify
PrivateTranslator-Quality.exe setup --portable

평소 번역 통신은 PC 안에 머뭅니다. 인터넷을 쓰는 기본 동작은 사용자가 직접 시작한 고정 공식 모델 설치뿐입니다. 기록과 승인 번역 자산 버전은 이 Windows PC의 암호화 기록고에 저장합니다.

공개용 ZIP은 Lite·Quality·Install-Model 실행 파일의 유효한 Authenticode 서명과 RFC 3161 타임스탬프를 필수로 확인합니다. upstream llama-server.exe는 고정 크기와 SHA-256으로 검증합니다. UNSIGNED-DEVELOPMENT 묶음은 로컬 검증 전용이며 Windows가 알 수 없는 게시자 경고를 표시할 수 있습니다.
