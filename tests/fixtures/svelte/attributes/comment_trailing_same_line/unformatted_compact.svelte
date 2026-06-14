<!-- Trailing line comment on a regular attribute stays inline -->
<div
data-attr="value"// trailing
>
text
</div>

<!-- Trailing line comment on a shorthand attribute -->
<div
{value}// trailing
>
text
</div>

<!-- Trailing line comment on a self-closing component -->
<Comp
prop={expr} // trailing
/>

<!-- Trailing line comment on a spread attribute -->
<div
{...expr}// trailing
>
text
</div>

<!-- Trailing line comment on a directive -->
<div
bind:value// trailing
>
text
</div>

<!-- Trailing block comment collapses inline (block is self-delimiting) -->
<div data-attr="value"/* bar */>text</div>

<!-- Own-line and same-line comments coexist; each placement preserved -->
<div
// own
data-attr="value"// trailing
>
text
</div>
