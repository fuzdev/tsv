<!-- Leading block comment: the shorthand declines and the comment stays -->
<Comp a={/* c */ a} />

<!-- Trailing block comment -->
<Comp a={a /* c */} />

<!-- Trailing line comment: the `}` takes its own line -->
<Comp
	a={a // c
	}
/>

<!-- HTML element: one attribute declines, its uncommented sibling still collapses -->
<input a={/* c */ a} {b} />

<!-- An uncommented shorthand still collapses -->
<Comp {a} {b} />

<!-- Directives: `class:` / `bind:` / `let:` / `style:` decline the same way -->
<div class:a={/* c */ a}></div>
<div class:a={a /* c */}></div>
<input bind:value={/* c */ value} />
<input bind:value={value /* c */} />
<Comp let:a={/* c */ a} />
<div style:color={/* c */ color}></div>
<div style:color={color /* c */}></div>

<!-- A directive's trailing `//` gives the `}` its own line and wraps the attribute list -->
<div
	class:a={a // c
	}
></div>
<div
	style:color={color // c
	}
></div>

<!-- The `bind:` `{getter, setter}` sequence is never a shorthand and is unaffected -->
<input bind:value={a, /* c */ b} />
