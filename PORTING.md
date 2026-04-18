# Clawd → Tauri 포팅 로드맵

macOS-only 타깃. 원본: https://github.com/rullerzhou-afk/clawd-on-desk

**진행 원칙**
- 위에서 아래로, 체크박스 기반. 블로커 만나면 바로 이 문서에 `⚠️`로 표시
- 각 마일스톤 끝에 `npm run tauri dev`로 수동 확인 → 커밋
- Rust 코드는 TDD (red → green → refactor). 테스트 가능한 순수 로직부터.
- 원본 파일 참조: `~/clawd-on-desk/src/*.js` (readonly, 참고용)

---

## M1. 투명 창에 SVG 렌더 — 리스크 체크 ✅
> WebKit이 픽셀 게를 Electron Chromium과 동일하게 그려주는지가 포팅 전체의 80% 리스크.

- [x] `tauri.conf.json` — 창 속성: `transparent: true`, `decorations: false`, `alwaysOnTop: true`, `resizable: false`, `skipTaskbar: true`, `width/height`: 200×200
- [x] `tauri.conf.json` + `Cargo.toml` — `macOSPrivateApi: true` + `tauri` 크레이트 `macos-private-api` feature (투명 창 필수)
- [x] `src/index.html` — 최소 구조, `<object type="image/svg+xml">`로 idle-follow SVG 로드
- [x] `src/m1-renderer.js` — 눈알 추적 (mouse → #eyes-js/#body-js/#shadow-js transform)
- [x] `npm run tauri dev` 실행 → 투명 창에 idle 게 렌더 확인
- [x] **자산 경로 문제 해결**: Tauri dev 서버는 symlink를 안 따라감 → `assets/`와 `themes/`를 `src/` 안으로 이동
- [ ] NSWindow native 조정 (`ignoresMouseEvents`, `LSUIElement`, `setActivationPolicy("accessory")`) — M4로 미룸

**학습 메모 (나중에 참조)**
- 투명 창: `macOSPrivateApi: true` **AND** `tauri` 크레이트에 `macos-private-api` feature 둘 다 필요. 둘 중 하나라도 빠지면 앱이 조용히 종료됨.
- 자산: Tauri v2 dev 서버(frontendDist 기반)는 symlink 안 따름. 해결책: 자산을 frontendDist 내부로 이동 or 프로덕션은 `bundle.resources` 사용.
- SVG 로드 실패 증상: WebKit가 `<object type="image/svg+xml">`에 HTML 404 페이지를 파싱하려다 "This page contains the following errors" 분홍 박스 표시.

---

## M2. HTTP 서버 (:23333) ✅
> 훅 스크립트가 POST 하는 엔드포인트. Rust axum.

- [x] `src-tauri/Cargo.toml` — `axum`, `tokio`, `tower`, `dirs` 추가
- [x] `src-tauri/src/server.rs` — `POST /state`, `GET /state` (health), `POST /permission`
- [x] 포트 자동 탐색 (23333–23337), `~/.clawd/runtime.json` 기록
- [x] `x-clawd-server: clawd-on-desk-tauri` response header로 신원 확인
- [x] 수신한 state event를 Tauri `emit`으로 renderer에 전달
- [x] `capabilities/default.json` — `pet` 창 + `core:event:default` 권한 추가 (emit/listen 허용)
- [x] 수동 확인: curl POST → renderer console에 `[m2] state-change` 수신 확인
- [ ] TDD 테스트 (state payload 파싱) — 나중에 채움
- [ ] 권한 응답 HTTP long-poll — M6에서 구현

**학습 메모**
- Tauri v2는 capability 기반 보안. 창 label이 capability의 `windows` 배열에 있어야 event listen 가능. `allow-listen` permission은 `core:event:default`에 포함.
- 리빌드 시 이전 프로세스 cleanup이 항상 호출되지 않음 → runtime.json에 stale port 남을 수 있음. 해결: 훅 쪽에서 23333-23337 전 범위 scan (M5).

---

## M3. 상태 머신 ✅
> 원본 `src/state.js` (1089줄) 중 핵심 로직만 포팅. 테마/SVG 매핑은 renderer 쪽이므로 제외.

- [x] `src-tauri/src/state.rs` — Session struct, StateMachine, Priority map, IncomingEvent, SharedState
- [x] `resolve_display_state()` — 우선순위 기반 최고 상태 선택
- [x] 최소 표시 시간 (priority 낮은 상태로 바뀔 때만 가드)
- [x] 자동 회귀 timer (tokio::spawn + clear_oneshot)
- [x] working 서브: 1→typing, 2→juggling, 3+→building
- [x] juggling 서브: subagent 1→juggling, 2+→conducting
- [x] 멀티 세션 추적 (agent_id:session_id 키)
- [x] 단위 테스트 8/8 통과
- [x] 실제 Claude Code 훅 live 이벤트로 검증 (기존 Electron Clawd 훅이 runtime.json 읽어서 자동 연결됨)

**학습 메모**
- Tauri 2의 `manage(T)` + `try_state::<T>()`로 상태 공유. Mutex는 기본 std (tokio Mutex는 필요 없음 — 락 구간이 짧음).
- auto-return timer는 `tauri::async_runtime::spawn` + `tokio::time::sleep` 조합. tokio feature `macros`가 없으면 `#[tokio::test]` 안 되지만 필요 없음.
- 원본이 가진 기능 중 M3에서 뺀 것: 테마 별 SVG 매핑(renderer 쪽), sleep 시퀀스(tick loop 필요, M7+), startup recovery, stale cleanup, session badge, i18n 이벤트 라벨 — 필요할 때마다 추가.

---

## M4. 시스템 트레이 + 메뉴 ✅
- [x] `tauri` 크레이트 `tray-icon` + `image-png` feature 활성화
- [x] `src-tauri/src/tray.rs` — TrayIconBuilder, 메뉴 구성
- [x] `include_image!` 매크로로 `src/assets/tray-iconTemplate.png` 임베드 (`Image::from_bytes`는 v2에 없음)
- [x] 메뉴: Sleep/Wake (DND), Show/Hide Pet, Hide Dock Icon, Quit
- [x] `app.set_activation_policy(ActivationPolicy::Accessory)` — 시작 시 자동 Dock 숨김
- [x] DND 토글: `SharedState.toggle_dnd()` → `handle_incoming`가 이벤트 silent drop → 펫 sleeping 유지
- [ ] 우클릭 메뉴 (pet 창) — M7로 미룸 (context menu는 click handling과 묶어서)
- [ ] Mini Mode 메뉴 항목 — M8에서 추가
- [ ] Settings 메뉴 항목 — M9에서 추가
- [ ] 언어 스위치 — i18n 포팅 때 (M9 이후)

**학습 메모**
- Tauri v2 `include_image!()` 매크로는 컴파일 타임 PNG 임베드. 런타임 `Image::from_path`도 있지만 include_image가 더 간결.
- `app.try_state::<T>()`는 `Manager` 트레이트 메서드 — `use tauri::Manager;` 필수.
- `set_activation_policy`는 `#[cfg(target_os = "macos")]` 게이트 필요.
- Template PNG (파일명 `*Template.png`)로 저장하면 macOS가 자동으로 다크/라이트 모드 대응.

---

## M5. 훅 자동 등록
- [ ] `~/.claude/settings.json`에 `hooks/clawd-hook.js` 경로 append-only 주입
- [ ] `registerHooks()` 앱 시작 시 자동 실행
- [ ] `/install:claude-hooks` CLI 커맨드 (원본과 호환)
- [ ] 다른 에이전트 (codex/cursor/gemini/opencode) 훅 등록 함수도 동일 패턴
- [ ] 커밋: `feat(m5): auto-register agent hooks`

---

## M6. 권한 버블 창
> 원본 `src/permission.js` + `src/bubble.html` 포팅.

- [ ] `POST /permission` → 새 Tauri Window 생성 (transparent, small, always-on-top)
- [ ] `bubble.html` 재사용 (이미 `src/`에 복사됨)
- [ ] Allow / Deny / Suggestion 버튼 → HTTP response
- [ ] 여러 개 스택 쌓기 (우하단에서 위로)
- [ ] 글로벌 단축키 `Ctrl+Shift+Y`/`Ctrl+Shift+N`
- [ ] 클라이언트 disconnect 감지 (자동 dismiss)
- [ ] 커밋: `feat(m6): permission bubble`

---

## M7. 드래그 / 클릭 / 이중 창
- [ ] Pointer Capture API로 드래그 (웹뷰에서 가능, 원본과 동일)
- [ ] 더블클릭 → poke 반응, 4연타 → flail 반응
- [ ] **이중 창 구조 필요한지 맥에서 테스트** — click-through만 `setIgnoresMouseEvents`로 토글 가능하면 단일 창으로 OK
- [ ] 단일 창 불가 시 hit window 별도 생성
- [ ] 커밋: `feat(m7): drag and click reactions`

---

## M8. 미니 모드
- [ ] 우측 엣지 스냅 (SNAP_TOLERANCE=30px)
- [ ] 크랩 워크 애니메이션 (이미 `mini-crabwalk.svg` 존재)
- [ ] 포물선 점프 (`animateWindowParabola`)
- [ ] Peek on hover
- [ ] 커밋: `feat(m8): mini mode`

---

## M9. 설정 창 + prefs
- [ ] 별도 Tauri Window로 `settings.html` 로드
- [ ] prefs.json 저장 (원본 `src/prefs.js` 포팅 — 버전 마이그레이션 포함)
- [ ] 주 언어 (en/ko/zh), 테마 선택, agent 토글
- [ ] 커밋: `feat(m9): settings window`

---

## M10. 자동 업데이트 + 패키징
- [ ] `tauri bundle` 설정: DMG (x64 + arm64)
- [ ] `tauri-plugin-updater` (GitHub Releases)
- [ ] 코드 사이닝 / notarization (이건 추후)
- [ ] GitHub Actions 빌드 워크플로
- [ ] 커밋: `feat(m10): DMG packaging and auto-updater`

---

## 메모리 검증 (포팅 완료 시)
- [ ] `ps -o pid,rss -p <pid>` 측정
- [ ] 목표: idle 60MB 이하 (원본 650MB 대비 10배 감소)

---

## 원본과의 차이 의도된 것들
- Windows 지원 버림 → `WS_EX_NOACTIVATE`, `koffi` FFI, PowerShell helper 전부 제거
- 이중 창 (`hitWin`)은 Windows 포커스 버그 우회였음. 맥에서 필요한지 M7에서 검증 후 결정.
- `electron-updater` → `tauri-plugin-updater`
- `electron-builder` → `tauri bundle`
- Git 모드 자동 업데이트 (원본) → 유지 가능 (Tauri와 무관)

---

## 참조 매핑 (원본 → Tauri)
| 원본 | Tauri 포지션 |
|---|---|
| `src/main.js` (2815줄) | `src-tauri/src/lib.rs` + 여러 모듈로 분리 |
| `src/server.js` | `src-tauri/src/server.rs` (axum) |
| `src/state.js` | `src-tauri/src/state.rs` |
| `src/permission.js` | `src-tauri/src/permission.rs` |
| `src/mini.js` | `src-tauri/src/mini.rs` |
| `src/focus.js` (PowerShell/koffi) | 삭제 (Windows 전용) |
| `src/mac-window.js` | `src-tauri/src/macos.rs` (objc2) |
| `src/login-item.js` | `src-tauri/src/autostart.rs` |
| `src/log-rotate.js` | `src-tauri/src/log.rs` |
| `src/prefs.js` | `src-tauri/src/prefs.rs` (serde_json) |
| `src/updater.js` | tauri-plugin-updater |
| `src/menu.js` | `src-tauri/src/menu.rs` (tauri tray/menu API) |
| `src/tick.js` (50ms 루프) | Rust tokio interval |
| `src/renderer.js`, HTML, CSS | **그대로** |
| `src/i18n.js`, `theme-loader.js`, `animation-cycle.js` | **그대로** |
| `hooks/*.js` | **그대로** (외부 스크립트) |

---

**현재 위치**: M1 시작 전. 다음 할 일 → `tauri.conf.json` 수정해서 투명 창 만들기.
