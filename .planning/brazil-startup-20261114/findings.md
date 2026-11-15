# 확인 내용

- 로그: C:/Users/pAsta/Downloads/hq-launcher.log, 12,290줄, 2026-06-26 ~ 2026-11-13 16:04:04.861.
- 전체 누적 ERROR 19건, WARN 74건. 최신 11월 13일에는 ERROR/WARN 없음.
- 11월 13일 15:51:09.232: Update download finished, installing...
- 이후 시작 기록: 15:51:19, 15:52:21, 16:02:34, 16:03:03, 16:04:04.
- 이후 대부분 mod manifest와 events manifest 요청까지만 기록됨. 종료나 panic 기록 없음.
- 11월 12일에는 게임 overlay ready와 AutoSheet input complete가 확인됨.
- 과거 DepotDownloader 경로 공백 문제와 Google Sheets 범위 오류는 현재 시작 실패와 시점이 다름.
- 사용자: 브라질에서 13~14일 실행. 제공 이미지 확인 결과 Preparing launcher / Loading mod preferences...에서 정지.
- 현 로컬 소스: Tauri config 1.15.0, 820cf9d release commit 2026-11-12 +09:00.
- 지역별 시간은 로그의 오프셋이 없어 임의 변환하지 않는다.

## 시작 정지 경로
- src/pages/LauncherPage.jsx:1359~1360: Loading mod preferences... 표시 후 get_disabled_mods를 await.
- src-tauri/src/lib.rs:12628~12641: get_disabled_mods는 로컬 설정을 읽은 뒤 원격 manifest도 await.
- src-tauri/src/mod_config.rs:261~264: 같은 manifest 요청이 진행 중이면 async 함수 안에서 std::sync::Condvar::wait로 실행 스레드를 동기 차단.
- 1.15.0 변경: get_practice_mod_list가 하드코딩 목록 반환에서 같은 원격 manifest를 가져오는 async 호출로 바뀜. 초기화 중 get_manifest와 함께 호출됨.
- 가설: 여러 동기 대기가 Tokio 작업 스레드를 모두 점유하면 원래 HTTP 요청과 12초 timeout이 실행되지 않아 시작이 무기한 정지할 수 있음. 브라질 네트워크 지연은 발생 조건일 수 있으나 해당 사용자 환경에서 확인하지 못함.
- 위 가설은 원본 캐시 함수의 격리 재현에서 확인됨. 사용자 환경의 확정 원인은 아직 아님.

## 추가 로그와 임시 우회 검증
- 새 첨부 Pasted text.txt와 기존 로그의 시간 포함 레코드 12,108개가 모두 동일함. 최신 11월 13일 기록 53개도 동일. 11월 14일 타임스탬프 없음.
- Tauri 2.10.3의 기본 async runtime은 Tokio Runtime::new()를 사용.
- 프로젝트 Tokio 1.50.0은 TOKIO_WORKER_THREADS 환경변수를 읽음. 프로세스 실행 전에 16으로 설정하면 런처의 기본 작업 스레드 수를 바꿀 수 있음.
- cache-probe.rs는 원본 mod_config.rs를 include하고 실제 fetch_remote_manifest_cached를 호출. 네트워크 전송만 500ms 지연 응답으로 모의 처리.
- TOKIO_WORKER_THREADS=4, 동시 요청 5개: 13초 후 0/5 완료. 원본의 12초 timeout도 진행하지 못함. 예상 정지 검증 exit 0.
- TOKIO_WORKER_THREADS=16, 같은 동시 요청 5개: 5/5 성공. exit 0.
- 이 결과는 해당 원인에 대한 임시 우회의 근거이며, 브라질 사용자의 앱 복구 성공을 뜻하지는 않음.
- 임시 절차: 런처 완전 종료 → exe 폴더에서 PowerShell → 현재 세션 환경변수 16 설정 → exe 실행. 시스템 전체 환경변수나 게임 데이터는 변경하지 않음.
