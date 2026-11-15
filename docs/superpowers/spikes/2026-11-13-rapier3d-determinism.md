# Rapier3d determinism spike — read-out granularity

Canonical plan: `0c519df`. Measured on x86_64-unknown-linux-gnu with
rustc 1.99.0 (b940084d7 2026-09-28), `rapier3d-f64` 0.36.1, features
`dim3,f64,std`, defaults disabled. Two local runs produced byte-identical lines:

```text
heads face=[0.0000107973496077,0.9999999998623939,0.0000125948492390] cos=1.00000000 pos=[0.00038083,0.00081440,-0.00097106] seed=0x5eed5eed5eed5eed 0xf30ec2cbe371dd5e linux
tails face=[-0.0000107973496077,-0.9999999998623934,-0.0000125948492390] cos=-1.00000000 pos=[0.00038083,0.00081440,-0.00097106] seed=0x5eed5eed5eed5eed 0xd257199bf14e1293 linux
bouncing-high face=[0.0000053373388985,0.9999999997648317,0.0000210202172577] cos=1.00000000 pos=[0.00907362,0.00081452,0.00517402] seed=0x5eed5eed5eed5eed 0xd40bea57374d2a94 linux
boundary face=[-0.6667198905660233,0.7452804828934255,0.0064489799017150] cos=0.74528503 pos=[0.43045627,0.00850790,-0.00004073] seed=0x5eed5eed5eed5eed 0xe38a1b5139a3fad3 linux
```

## Verdict

**Determinism holds at the read-out granularity**: §7 stands as written (every peer
simulates; the keyframe is the fallback alone), and Tasks 5–8 proceed unchanged.

The reading of this sentence, honestly scoped: on this host two runs were byte-identical,
and the three named faces and the boundary probe all rest where their names say (the
boundary line lands a hair past cos(45°) — which face-up side of the boundary it is
on, at 8 decimal places, is what its line measures; that is §13.1's finest question,
and it was the plan's own fixture, not physics, that first disagreed with itself). The
same question asked of the *other* targets is what the lines exist for: the native CI
runners (aarch64-unknown-linux-gnu, aarch64-apple-darwin, x86_64-pc-windows-msvc)
and the wasm browser run print the same four cases, and a hash that disagrees names which
pose disagreed and at what granularity. Unequal raw hashes alone do not establish an
adjudication-granularity mismatch — the raw normals carry 16 places, positions 8, and
§12's adjudication reads coarser than both. The cross-target runs are what the CI matrix
(Task 3) runs; nothing here is claimed that has not been printed.

## Cross-target confirmation (Task 3's CI matrix runs these)

Native CI runners run the same named test with `--nocapture` and retain the four
comparison lines. The wasm side runs the wasm test target (Task 2's spike is what proves
the build closes for `wasm32-unknown-unknown` at all). Compare hashes first, then
inspect differences: differences that leave the adjudication unchanged are not a mismatch
under §12.
