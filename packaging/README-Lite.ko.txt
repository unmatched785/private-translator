Private Translator Lite 0.1.1
================================

처음 한 번
1. 인터넷에 연결된 상태에서 Install-Model.cmd를 더블클릭합니다.
2. 고정된 공식 Hy-MT2 모델(약 1.13GB)을 내려받고, 중단된 다운로드를 이어받아 크기와 SHA-256을 확인합니다.
3. PrivateTranslator-Lite.exe를 더블클릭합니다.
4. 브라우저에서 쓰는 동안 프로그램 창을 열어 둡니다.

작은 앱 묶음에는 모델이 들어 있지 않습니다. 검증된 모델은 기본적으로 이후 앱 버전도 함께 쓰는 다음 위치에 둡니다.
%LOCALAPPDATA%\PrivateTranslator\models

고급 명령:
PrivateTranslator-Lite.exe model status
PrivateTranslator-Lite.exe model verify
PrivateTranslator-Lite.exe setup --portable
PrivateTranslator-Lite.exe setup --from C:\경로\Hy-MT2-1.8B-Q4_K_M.gguf

- 평소 번역 통신은 127.0.0.1 안에 머물며 인터넷 번역 API로 보내지 않습니다.
- 기본 경로에서 인터넷을 쓰는 것은 사용자가 직접 시작한 모델 설치뿐입니다.
- 기록은 현재 Windows 계정으로 보호한 AES-256-GCM 암호문으로 저장합니다.
- 기본 기록 위치: %LOCALAPPDATA%\PrivateTranslator
- 프로그램 폴더를 지워도 기록과 공용 모델은 자동 삭제하지 않습니다.

현재 릴리즈는 코드 서명이 없어 Windows가 알 수 없는 게시자 경고를 표시할 수 있습니다.
