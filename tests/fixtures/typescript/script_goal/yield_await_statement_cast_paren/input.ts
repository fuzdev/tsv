// `yield` and `await` are identifiers in a sloppy script; heading a cast statement they
// keep their pair, since bare they open a `yield` / `await` expression
(yield) as T;
(yield) satisfies T;
(await) as T;
(await) satisfies T;
