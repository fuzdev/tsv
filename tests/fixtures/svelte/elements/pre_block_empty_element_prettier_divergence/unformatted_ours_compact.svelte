<!-- An empty block-body element inside <pre> that overflows. The wrapped attributes and close token indent off the element's nesting depth (one level per enclosing container, matching prettier). The close form follows the same rule as everywhere else: a component keeps an authored `/>`, a plain HTML element normalizes to its close tag (see ws_sensitive_self_closing_kinds), so the `<span>` case is explicit-empty. When the attributes fit within print width they stay on one line and only the close token drops to its own line; when they overflow they wrap one per line with `/>` on its own line, never hugging the last attribute. The divergence is print-width-as-hard-limit: output_prettier shows prettier full-breaking the self-closing elements' attributes even though they fit (the explicit-empty and overflowing cases now match prettier). -->
<pre>{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5}/>{/if}</pre>

<pre>{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5}></Comp>{/if}</pre>

<pre>{#if cond}<span data-a={value1} data-b={value2} data-c={value3} data-d={value4} data-e={value5}></span>{/if}</pre>

<!-- attrs too long to fit on one line: they wrap one per line, the close form is still preserved, and `/>` drops to its own line rather than hugging the last attr -->
<pre>{#if cond}<Comp prop1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" prop2={value2} prop3={value3}/>{/if}</pre>

<pre>{#if cond}<Comp prop1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" prop2={value2} prop3={value3}></Comp>{/if}</pre>
