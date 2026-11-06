```
                                      ┌─────────────────────────────┐
                                      │                             │
                                      │           SPEC.md           │
                                      │                             │
                                      └──────────────┬──────────────┘
                                                     │
                                                     │
                                      ╔══════════════▼══════════════╗                       ┏━━━━━━━━━━━━━━━━━━━━━━━━━┓
                                      ║                             ╟─── TOOLS / BLOCKED ───▶      Orchestrator:      ┃
                                      ║     Planner: Preflight      ║                       ┃     Wait for human:     ┃
                                      ║                             ◀── /factory continue ──┨ fix spec, install tools ┃
                                      ╚══════════════╤══════════════╝                       ┗━━━━━━━━━━━━━━━━━━━━━━━━━┛
                                                     │ READY
                                                     │
                                      ╔══════════════▼══════════════╗
                                      ║                             ║
                                      ║        Planner: Plan        ║
                                      ║                             ║
                                      ╚══════════════╤══════════════╝
                                                     │
                                                     │ human reviews the plan,
                                                     │ then /factory continue
                                                     │
                                      ╔══════════════▼══════════════╗
                                      ║                             ╟──── BLOCKED ──────────────────────────────┐
┌─────────────────────────────────────▶           Builder           ◀─────────────────────────────────────┐     │
│                                     ║                             ║                                     │     │
│                                     ╚══════════════╤══════════════╝                                     │     │
│                                                    │                                                    │     │
│                                                    │                                                    │     │
│                  ┌─────────────────────────────────┼─────────────────────────────────┐                  │     │
│                  │                                 │                                 │                  │     │
│                  │                                 │                                 │                  │     │
│   ╔══════════════▼══════════════╗   ╔══════════════▼══════════════╗   ╔══════════════▼══════════════╗   │     │
│   ║        Code Reviewer        ║   ║      Feature Reviewer       ║   ║    Specialist Reviewers     ║   │     │
│   ║        (every task)         ║   ║        (every task)         ║   ║    (those task.md lists)    ║   │     │
│   ╚══════════════╤══════════════╝   ╚══════════════╤══════════════╝   ╚══════════════╤══════════════╝   │     │
│                  │                                 │                                 │                  │     │
│                  │                                 │                                 │                  │     │
│                  └─────────────────────────────────┼─────────────────────────────────┘                  │     │
│                                                    │                                                    │     │
│                                                    │                                                    │     │
│                                     ┏━━━━━━━━━━━━━━▼━━━━━━━━━━━━━━┓   Round 1 and any review FAIL:      │     │
│                                     ┃        Orchestrator:        ┃   resume builder (fix round)        │     │
│                                     ┃    Wait for all reviews     ┠─────────────────────────────────────┤     │
│                                     ┗━━━━━━━━━━━━━━┯━━━━━━━━━━━━━━┛                                     │     │
│                                                    │                                                    │     │
│                              If all reviews PASS,  │                                                    │     │
│                              after the fix round,  │                                                    │     │
│                           or after a rework round  │                                                    │     │
│                                                    │                                                    │     │
│                                     ╔══════════════▼══════════════╗                                     │     │
│                                     ║                             ║                                     │     │
│                                     ║          Approver           ╟────── REWORK (max 2 per task) ──────┘     │
│                                     ║                             ║                                           │
│                                     ╚══════════════╤══════════════╝                                           │
│                                                    │                                                          │
│                                                    │                                                          │
│                               ┌APPROVE─────────────┴────────────ESCALATE┐                                     │
│                               │                                         │                   Builder blocked:  │
│                               │                                         │                     skip reviewers  │
│                               │                                         │                       and Approver  │
│  next task     ┏━━━━━━━━━━━━━━▼━━━━━━━━━━━━━━┓           ┏━━━━━━━━━━━━━━▼━━━━━━━━━━━━━━┓                      │
├────────────────┨        Orchestrator:        ┃           ┃        Orchestrator:        ◀──────────────────────┘
│                ┃        Merge to main        ┃           ┃   Wait for human decision   ┃
│                ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛           ┗━━━━━━━━━━━━━━┯━━━━━━━━━━━━━━┛
│                                                                         │
│                                                                         │
└──────────────────────────── /factory rework ────────────────────────────┘
```

Specialist Reviewers are the roster's `security`, `privacy`, `api` and `ui`
reviewers. Each task's `Reviews` section lists the ones that run on it.
