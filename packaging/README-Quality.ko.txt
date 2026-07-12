Private Translator Quality 0.1.1
===================================

1. 인터넷에 연결된 상태에서 Install-Model.cmd를 더블클릭해 공용 Hy-MT2 1.8B 모델(약 1.13GB)을 설치·검증합니다.
2. PrivateTranslator-Quality.exe를 더블클릭합니다.
3. 브라우저에서 쓰는 동안 프로그램 창을 열어 둡니다.

선택형 Hy-MT2 7B 항목은 허용된 개인 주소의 호환 모델 서버가 필요합니다. 클라우드 번역 API로 자동 전환하지 않으며, 첫 Quality 릴리즈는 7B 모델팩을 포함하거나 자동 설치하지 않습니다.

공용 모델 위치:
%LOCALAPPDATA%\PrivateTranslator\models

확인 명령:
PrivateTranslator-Quality.exe model status
PrivateTranslator-Quality.exe model verify
PrivateTranslator-Quality.exe setup --portable

평소 번역 통신은 PC 안에 머뭅니다. 인터넷을 쓰는 기본 동작은 사용자가 직접 시작한 고정 공식 모델 설치뿐입니다. 기록과 승인 번역 자산 버전은 이 Windows PC의 암호화 기록고에 저장합니다.

현재 릴리즈는 코드 서명이 없어 Windows가 알 수 없는 게시자 경고를 표시할 수 있습니다.
