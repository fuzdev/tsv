<!-- An empty block-body element inside <pre> that overflows. tsv indents one level shallower than prettier and preserves the source close form for both a component and an HTML inline element: self-closing keeps `/>`, explicit-empty keeps its close tag. When the attrs fit they stay on one line (only the close moves to its own line); when they are too long they wrap one per line — the `/>` still drops to its own line, never hugging the last attr. output_prettier shows prettier's form. -->
<pre>{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5}/>{/if}</pre>

<pre>{#if cond}<Comp prop1={value1} prop2={value2} prop3={value3} prop4={value4} prop5={value5}></Comp>{/if}</pre>

<pre>{#if cond}<span data-a={value1} data-b={value2} data-c={value3} data-d={value4} data-e={value5}/>{/if}</pre>

<pre>{#if cond}<span data-a={value1} data-b={value2} data-c={value3} data-d={value4} data-e={value5}></span>{/if}</pre>

<!-- attrs too long to fit on one line: they wrap one per line, the close form is still preserved, and `/>` drops to its own line rather than hugging the last attr -->
<pre>{#if cond}<Comp prop1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" prop2={value2} prop3={value3}/>{/if}</pre>

<pre>{#if cond}<Comp prop1="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" prop2={value2} prop3={value3}></Comp>{/if}</pre>
