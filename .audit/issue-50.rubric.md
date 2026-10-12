# Architect rubric

Score each 1-5.

1. One shared all-Element 16:9 placement and exact scaled glyph path. Read-only output excludes selection/drag/edit controls.
2. Existing document draft/history/reload/asset semantics remain single owned and transactional selection is reused.
3. 100-slide virtualized visible rendering and selection exposure do not fight manual scroll or create stale asynchronous results.
4. Keyboard and native AX selection/focus are explicit; real VoiceOver proof is acknowledged as required.
5. Smallest maintainable interface hides real invariants without extensive editor rewrite or duplicated selection ownership.

Candidates A/B/C are independent packages under /tmp/psycho-issue-50-design-{a,b,c}. The repo grounding is .audit/issue-50.grounding.md.
