<!-- Leading block comment: the shorthand declines and the comment stays -->
<Comp {a} />

<!-- Trailing block comment -->
<Comp {a} />

<!-- Trailing line comment: the `}` takes its own line -->
<Comp {a} />

<!-- HTML element: one attribute declines, its uncommented sibling still collapses -->
<input {a} {b} />

<!-- An uncommented shorthand still collapses -->
<Comp {a} {b} />

<!-- Directives: `class:` / `bind:` / `let:` / `style:` decline the same way -->
<div class:a></div>
<div class:a></div>
<input bind:value />
<input bind:value />
<Comp let:a />
<div style:color></div>
<div style:color></div>

<!-- A directive's trailing `//` gives the `}` its own line and wraps the attribute list -->
<div class:a></div>
<div style:color></div>

<!-- The `bind:` `{getter, setter}` sequence is never a shorthand and is unaffected -->
<input bind:value={a, /* c */ b} />
