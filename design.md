# High-Level Design Ideas

*All names subject to intense bikeshedding.*

## Segments

A single `Timeline` is a collection of `Segments`.
Each `Segment` represents a portion of the fight, with an internal linear timeline.
Think a basic block. 

`Segments` serve two purposes:

1.  Allowing fight timeline code to be movable and reusable, rather than using absolute timestamps.
1.  Logically grouping portions of the fight for players, e.g. breaking the fight down mechanic-by-mechanic.

`Segments` can be, and often are, nested, and a single `Segment` can occur multiple times in an overall timeline.
When a single `Segment` occurs multiple times in the timeline, it might have a separate strat each time.
A `Segment` can be minor, hidden from players' so that it is only used internally by the fight.
Filler `Segments` in between mechanics are frequently hidden, and will likely end up being implicit where the `Timeline` has gaps.
The remaining, major `Segments` can be independently selected and planned for, and strats can be made and shared for a single `Segment` or an entire timeline.

Note that `Segments` do not solve all timeline grouping issues: code for a boss to execute a certain attack, for instance,
may need to be repeatable at different times in the fight as part of different mechanics.
`Segments` are a big hammer usable as a fallback if other methods fail.

There is no support for looping fight timelines, such as normal content or DSR,
but if needed multiple iterations of a loop can be plotted out with recurring segments.

Some `Segments` will require additional state, such as buffs or debuffs, that can be provided by an earlier `Segment`,
or through a specific initialization routine for when the `Segment` is mapped independently.

## Variations

A `Variation` is any point where the fight does something different based on randomness or particular conditions.
Conditional variations do not have special-cased support, they are just treated as random.

`Variations` can result in a number of differences, but these can be grouped into two different categories:

1.  A timeline branch, where the next `Segment` after the current one is determined by the `Variation`.
    The most important example is dog vs snake first in P8S.
    But stack/spread mechanic variations are all also examples.
1.  A minor `Variation`, where the fight plays out slightly, but not substantially, differently based on the random factor.

There will almost certainly need to be a DSL for defining `Variations` and how they relate to one another.
Random `Variation` state must be able to cross between `Segments`, because timeline branches often occur in pairs,
but it must also be able to operate correctly when a `Segment` is plotted out without the earlier variation code present.
This implies that `Variations` should probably exist as global variables,
and any `Variation` referred to by any `Segment` in the `Timeline` must exist.

Initialization code for unanchored `Segments` may also require `Variations`,
e.g. to assign debuffs to players that would otherwise be based on earlier strats.

## Keyframes

Various timestamps can be `Keyframes` for both the fight `Timeline` and animations/boss attacks, but also for the players' strats.
Individual attacks/animations will have their own `Keyframes` defined in their tweening logic.
`Keyframes` for the players' strats are called `Stratframes`, and are first-class entities.

All `Keyframe` timestamps are stored relative to a specific `Segment`.

A `Timeline` should contain default `Stratframes` for particular points in the mechanic, but these can be changed by the user.
Some frames in a `Timeline` can be indicated as `Snapshot` frames.
While players editing a `Strat` at a `Stratframe` will see the mechanics as they exist at that point,
the actual keyframe when animating the fight will instead be at the previous `Snapshot` frame, if one exists.
This will allow mechanics to resolve correctly.
Manual manipulation of `Snapshot` frames of particular `Stratframe` will likely be required.

`Snapshot` frames can have labels and be referred to by mechanics resolving later in the fight,
to target former player positions.

## Timeline Manager

The `TimelineManager` is responsible for taking a `Fight` and turning it into something usable.

All entities needed for the `Timeline` are kept in the `Source`, a separate `World`.
Each entity contains its own animation script for each `Segment`, including spawn and despawn times.
All timestamps are relative to `Segments`' internal timelines.

When the current timestamp changes, the `TimelineManager` will (likely in `PreUpdate`?):

*   Spawn any entities that need to be newly spawned.
*   Despawn any entities that need to be despawned.
*   Ensure the correct `Segment` and internal timestamp are loaded.
*   Load any relevant `Snapshot` data.
*   If necessary, when jumping, run a minimal replay of player movement and movement of
    any entity that depends on player movement in order to correctly determine the current
    position.