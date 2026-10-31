# M1-A Windows smoke test

Record Windows version, WebView2 version, commit SHA, build command or Actions run URL, and the result of each check. Leave unperformed checks marked **not performed**. This plan is not a claim that runtime testing has happened.

1. On Windows 11 x64, follow the README clean checkout/prerequisites and run every verification command. Confirm Rust reports toolchain 1.90.0 and dependency lockfiles remain unchanged.
2. Run `npm run tauri -- dev`. Confirm a native **ECHELON OS** window opens without elevation and renders a readable Graphite shell with muted teal focus. Verify minimum size, resizing, scrolling and text at 100%, 150% and 200% Windows scaling.
3. On Overview, confirm **Desktop connected**, `ECHELON OS`, version `0.1.0`, platform `windows`, and stage `M1-A`. This is the real frontend-to-Rust command smoke check. A missing status/error is a failure; browser preview does not satisfy it.
4. Use only Tab/Shift+Tab/Enter/Space. Verify visible focus, the **Skip to content** link, Overview navigation and NAVCOM navigation. Confirm the selected destination is announced. With a screen reader, check headings, navigation and desktop status announcements.
5. Open **View module details** and the NAVCOM navigation item. Both must show **Planned**, **Not yet implemented**, and unavailable route planning/EVE connectivity. There must be no route search, waypoint control or fake EVE data. Return to Overview.
6. Close and relaunch. It must start normally on Overview; no saved workspace/settings should be promised. Confirm there are no ongoing network calls, polling or unexpected child processes beyond the normal Tauri/WebView2 runtime. The dev server/HMR traffic is development-only.
7. Run `npm run build` and `npm run preview` separately in a browser. Confirm **Browser preview · Desktop connection unavailable**, never **Desktop connected**. The component suite covers loading, transport failure, malformed metadata and explicit retry; an actual native failure should show the same concise retry state without raw backend errors.
8. Build with `npm run tauri -- build --bundles nsis --no-sign -- --locked`, or download the exact SHA's unsigned Actions artifact. Launch the release executable and repeat checks 2–5, especially **Desktop connected**: this catches production CSP/IPC problems that development may hide.
9. Run the NSIS installer as the current user with WebView2 present. Confirm the per-user installation, shortcut, launch and uninstall work without requesting administrator access. Record any Windows unsigned-publisher/SmartScreen prompt. Do not disable machine security controls to make a test pass.
10. Record failures and logs on the draft PR. A successful CI build is build evidence only. Signing, distribution, recovery and full packaging signoff are outside M1-A.

## Result record

| Check                                 | Result / evidence |
| ------------------------------------- | ----------------- |
| Commit / Actions run                  | Not performed     |
| Windows / WebView2 / scale            | Not performed     |
| Clean checks and unsigned build       | Not performed     |
| Native development shell and IPC      | Not performed     |
| Keyboard / screen reader / scaling    | Not performed     |
| Planned NAVCOM / honest preview       | Not performed     |
| Release CSP and IPC                   | Not performed     |
| Per-user install / launch / uninstall | Not performed     |
