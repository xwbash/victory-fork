# `v_writelane_b32` and `v_readlane_b32` where their lane is not running

Status: fixed for the programs the wave can take together; the rest translate as before.

## What was wrong

On the console `v_writelane_b32 vN, sX, K` writes lane K whatever EXEC is, and
`v_readlane_b32 sY, vN, K` reads it the same way. The translator lowered the write as an
ordinary vector write, done by the lane itself: when lane K was outside EXEC, or did not
reach the block, the write was lost and a later read gave the old value (or, for a lane
that is not in the block at all, whatever the host gives).

`ir::unspill` (`crates/victory-shader/src/ir.rs`) turns such pairs into scalar moves, but
only for a VGPR whose every use is a pair with a constant lane. What it leaves in this
title's shader pack, translated for a 32-lane host:

* 40 compute programs keep lane writes, 5,423 of them. All 40 are 32-lane programs, and
  all are ray-tracing trace programs (`shadowmask:CS_TraceShadowRays_*`,
  `rtdirectionalshadows:*`, `sr_tracing:CS_Raytrace*`, the screen-probe tracers, ...).
* They keep a small array of scalars in the lanes of one VGPR: written by constant lane,
  read by a computed one (`v_readlane_b32 s22, v29, s22`), and the VGPR is an ordinary
  temporary elsewhere in the program. Hence no `unspill`.
* Every one of those writes stands in a block that not every lane runs, with no guard: the
  lane written is simply not there when it left through an early-out.
* No 64-lane program keeps a lane write. The 14 `CS_ShadowRenderTiled*` programs of
  issue #62 are not among the 40, so this is not what is wrong with them.

## What the lowering does now

`crates/victory-shader/src/emit/lower.rs`, the two arms and `Lower::lane_there`:

* A lane write, or a lane read of a VGPR that lane writes fill, in a block only some lanes
  run is refused (`LANE_ELSEWHERE`), naming the block.
* `emit` then has the wave take the branches around that block together (`widen`), so
  every lane is there. Where the host's waves are the console's, as for the 40 programs,
  that costs no barrier. A console wave run as two host waves goes through its forms
  without skips instead.
* Under a guard (lanes there with EXEC clear) the lane takes the scalar when some lane of
  the console wave is in EXEC, with or without its own bit. Two host waves of 32 exchange
  their answers.
* If the branches cannot be taken together, the program is translated as before
  (`with_lanes_as_they_stand`): no program stops translating.

Measured on the pack (`VICTORY_HOST_MAX_WAVE=32 victory translate --validate`), before and
after: 2,669 of 2,682 compute programs emitted and validated both times. Of the 40
programs, 36 translate either way and all 36 get a pipeline on an RTX 5080: 25 now with
their branches wave-wide (6 to 131 branches each), 11 as before. Those 11 stop wave-wide on
"a descriptor's address or format read as data" (10) and "a constant buffer's descriptor
read as data" (1): `VICTORY_WAVE_WHY=1` prints `LANE-ELSEWHERE ...` with the reason.

## Tests

`crates/victory-shader/src/emit/wave64_tests.rs`; both need a hardware device with shader
model 6.6 and print `not run` without one.

* `a_lane_write_reaches_its_lane_outside_exec`: a console wave of 64 on a 32-lane host.
* `a_lane_array_is_written_and_read_with_the_skip_wave_wide`: a 32-lane wave, the trace
  programs' pattern.

## Not known, not done

* "Expected" is the instruction's documented behaviour (it ignores EXEC). It has not been
  measured on a console.
* Not checked in the game: the 25 programs' cost with their branches wave-wide, and
  whether their results change. Ray tracing lights nothing yet (see
  `docs/fork-ray-tracing.md`).
* Not run on a 64-lane card.
* The 11 programs left as before. Keeping the lanes of such a VGPR as scalars in
  `ir::unspill` (a VGPR with other uses, computed lane reads) would settle those too, at
  no cost per frame.
* Under a guard, "the wave is here" is read as "some lane is in EXEC". A wave that walks
  on with EXEC empty and no skip writes too on the console.
